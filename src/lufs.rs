//! LUFS calculator with streaming audio processing

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use crate::error::{LufsError, Result};
use crate::decoders::{AudioDecoder, create_decoder};

/// Progress callback type - receives (bytes_read, total_bytes)
pub type ProgressCallback = Arc<AtomicU64>;

/// Reader wrapper that reports read progress in bytes.
///
/// Progress is reported as the furthest byte offset reached, which keeps
/// progress monotonic even if the decoder performs internal seeks.
struct ProgressReader<R: Read + Seek> {
    inner: R,
    progress: ProgressCallback,
    position: u64,
    max_position: u64,
}

impl<R: Read + Seek> ProgressReader<R> {
    fn new(mut inner: R, progress: ProgressCallback) -> Result<Self> {
        let position = inner.stream_position().map_err(LufsError::Io)?;
        progress.store(position, std::sync::atomic::Ordering::Relaxed);
        Ok(Self {
            inner,
            progress,
            position,
            max_position: position,
        })
    }

    fn report_progress(&mut self) {
        if self.position > self.max_position {
            self.max_position = self.position;
            self.progress
                .store(self.max_position, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

impl<R: Read + Seek> Read for ProgressReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let bytes = self.inner.read(buf)?;
        self.position = self.position.saturating_add(bytes as u64);
        self.report_progress();
        Ok(bytes)
    }
}

impl<R: Read + Seek> Seek for ProgressReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = self.inner.seek(pos)?;
        self.position = new_pos;
        Ok(new_pos)
    }
}

/// LUFS calculation result
#[derive(Debug, Clone)]
pub struct LufsResult {
    /// Filename
    pub filename: String,
    /// Full path
    pub path: String,
    /// Calculated LUFS value (None if calculation failed)
    pub lufs: Option<f64>,
}

/// LUFS calculator with configurable chunk size
///
/// Processes audio data chunk-by-chunk for memory efficiency.
/// Default chunk size is 8192 samples (~93ms at 44.1kHz stereo).
#[derive(Debug, Clone)]
pub struct LufsCalculator {
    #[allow(dead_code)]
    chunk_size: usize,
}

impl LufsCalculator {
    /// Create a new calculator with custom chunk size
    ///
    /// # Arguments
    ///
    /// * `chunk_size` - Number of samples to process per chunk (default: 8192)
    ///
    /// Smaller chunks use less memory but may be slightly slower due to overhead.
    pub fn new(chunk_size: usize) -> Self {
        LufsCalculator { chunk_size }
    }

    /// Calculate LUFS from a file path
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the audio file
    ///
    /// # Returns
    ///
    /// * `Ok(Some(lufs))` - LUFS value in dB
    /// * `Ok(None)` - File format not supported
    /// * `Err(...)` - Error occurred
    pub fn calculate_from_file(&self, path: &Path) -> Result<Option<f64>> {
        self.calculate_from_file_with_progress(path, None)
    }

    /// Calculate LUFS from a file path with progress reporting
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the audio file
    /// * `progress` - Optional AtomicU64 that will be updated with bytes read
    ///
    /// # Returns
    ///
    /// * `Ok(Some(lufs))` - LUFS value in dB
    /// * `Ok(None)` - File format not supported
    /// * `Err(...)` - Error occurred
    pub fn calculate_from_file_with_progress(
        &self,
        path: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<Option<f64>> {
        // Check if file exists
        if !path.exists() {
            return Err(LufsError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File does not exist: {}", path.display()),
            )));
        }

        // Check if file has supported extension (fast path)
        let ext = path.extension().and_then(|e| e.to_str());
        if let Some(ext_str) = ext {
            let ext_lower = ext_str.to_lowercase();
            if !crate::SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str()) {
                return Ok(None);
            }
        }

        // Get file size for progress tracking
        let file_size = std::fs::metadata(path).ok().map(|m| m.len());

        // Open file
        let file = File::open(path)?;
        self.calculate_from_reader_with_progress(file, file_size, progress)
    }

    /// Calculate LUFS from a generic reader with progress reporting
    ///
    /// # Arguments
    ///
    /// * `reader` - Any type implementing `Read + Seek` (File, Cursor, etc.)
    /// * `file_size` - Optional file size for progress tracking
    /// * `progress` - Optional AtomicU64 that will be updated with bytes read
    ///
    /// # Returns
    ///
    /// * `Ok(Some(lufs))` - LUFS value in dB
    /// * `Ok(None)` - Format not supported
    /// * `Err(...)` - Error occurred
    ///
    /// Note: Format is automatically detected from stream content.
    pub fn calculate_from_reader_with_progress<R: Read + Seek + Send + Sync + 'static>(
        &self,
        reader: R,
        file_size: Option<u64>,
        progress: Option<ProgressCallback>,
    ) -> Result<Option<f64>> {
        // Create decoder with automatic format detection
        let mut decoder = if let Some(progress_ref) = progress.clone() {
            let progress_reader = ProgressReader::new(reader, progress_ref)?;
            create_decoder(progress_reader)?
        } else {
            create_decoder(reader)?
        };

        // Initialize EBU R128 loudness meter
        let mut ebur = ebur128::EbuR128::new(
            decoder.channels(),
            decoder.sample_rate(),
            ebur128::Mode::I,
        )
        .map_err(|e| LufsError::EbuR128Error(format!("Failed to create EBU R128: {:?}", e)))?;

        // Process audio in chunks
        self.calculate_with_decoder(&mut decoder, &mut ebur)?;

        // Update progress to completion if tracking
        if let (Some(size), Some(prog)) = (file_size, progress) {
            prog.store(size, std::sync::atomic::Ordering::Relaxed);
        }

        // Get the loudness value
        let loudness = ebur
            .loudness_global()
            .map_err(|e| LufsError::EbuR128Error(format!("Failed to get loudness: {:?}", e)))?;

        Ok(Some(loudness))
    }

    /// Calculate LUFS from a generic reader
    ///
    /// # Arguments
    ///
    /// * `reader` - Any type implementing `Read + Seek` (File, Cursor, etc.)
    ///
    /// # Returns
    ///
    /// * `Ok(Some(lufs))` - LUFS value in dB
    /// * `Ok(None)` - Format not supported
    /// * `Err(...)` - Error occurred
    ///
    /// Note: Format is automatically detected from stream content.
    pub fn calculate_from_reader<R: Read + Seek + Send + Sync + 'static>(
        &self,
        reader: R,
    ) -> Result<Option<f64>> {
        self.calculate_from_reader_with_progress(reader, None, None)
    }

    /// Internal method: Calculate LUFS using a decoder
    fn calculate_with_decoder(
        &self,
        decoder: &mut Box<dyn AudioDecoder>,
        ebur: &mut ebur128::EbuR128,
    ) -> Result<()> {
        loop {
            match decoder.decode_chunk()? {
                Some(samples_i16) => {
                    // Convert i16 to f32 in range [-1.0, 1.0]
                    let samples_f32: Vec<f32> = samples_i16
                        .iter()
                        .map(|&s| s as f32 / 32768.0)
                        .collect();

                    // Feed to EBU R128
                    ebur.add_frames_f32(&samples_f32).map_err(|e| {
                        LufsError::EbuR128Error(format!("EBU R128 processing error: {:?}", e))
                    })?;
                }
                None => break, // EOF
            }
        }

        Ok(())
    }
}

impl Default for LufsCalculator {
    fn default() -> Self {
        Self::new(8192)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculator_default() {
        let calc = LufsCalculator::default();
        assert_eq!(calc.chunk_size, 8192);
    }

    #[test]
    fn test_calculator_custom() {
        let calc = LufsCalculator::new(4096);
        assert_eq!(calc.chunk_size, 4096);
    }

    #[test]
    fn test_calculator_nonexistent_file() {
        let calc = LufsCalculator::default();
        let result = calc.calculate_from_file(Path::new("/nonexistent/file.mp3"));
        assert!(result.is_err());
    }

    #[test]
    fn test_calculator_unsupported_format() {
        let calc = LufsCalculator::default();
        // Create a temporary file with unsupported extension
        let temp_file = std::env::temp_dir().join("test.xyz123");
        std::fs::write(&temp_file, b"dummy data").unwrap();

        let result = calc.calculate_from_file(&temp_file);
        assert!(matches!(result, Ok(None)));

        std::fs::remove_file(&temp_file).unwrap();
    }

    #[test]
    fn test_lufs_result_creation() {
        let result = LufsResult {
            filename: "test.mp3".to_string(),
            path: "/path/to/test.mp3".to_string(),
            lufs: Some(-12.5),
        };
        assert_eq!(result.filename, "test.mp3");
        assert_eq!(result.path, "/path/to/test.mp3");
        assert_eq!(result.lufs, Some(-12.5));
    }
}
