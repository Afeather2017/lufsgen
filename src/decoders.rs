//! Audio decoder trait and unified format detection

use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

pub use self::symphonia_decoder::SymphoniaDecoder;

pub mod symphonia_decoder;

/// Trait for streaming audio decoders
pub trait AudioDecoder {
    /// Get sample rate in Hz
    fn sample_rate(&self) -> u32;

    /// Get number of channels
    fn channels(&self) -> u32;

    /// Decode next chunk of audio samples
    /// Returns `Ok(Some(samples))` with decoded i16 samples, `Ok(None)` on EOF, or error
    fn decode_chunk(&mut self) -> crate::error::Result<Option<Vec<i16>>>;
}

/// Create a decoder from a reader with automatic format detection
///
/// This function uses Symphonia's format detection which analyzes the stream
/// content (magic bytes) to determine the format. It supports:
/// - MP3 (mp3)
/// - OGG/Vorbis (ogg, oga)
/// - FLAC (flac)
/// - AAC (aac)
/// - M4A/MP4 (m4a, mp4)
/// - WAV (wav)
///
/// Note: Reader input must be seekable because many formats require seeking.
pub fn create_decoder<R: Read + Seek + Send + Sync + 'static>(
    reader: R,
) -> crate::error::Result<Box<dyn AudioDecoder>> {
    Ok(Box::new(SymphoniaDecoder::new(reader)?))
}

/// Create a decoder for a file path
///
/// This is the recommended way to create decoders for files.
/// It automatically detects the format from the file content.
pub fn create_decoder_from_path(path: &Path) -> crate::error::Result<Box<dyn AudioDecoder>> {
    let file = File::open(path)?;
    create_decoder(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_decoder_requires_valid_reader() {
        // Empty data should fail
        let empty: &[u8] = &[];
        let reader = std::io::Cursor::new(empty);
        let result = create_decoder(reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_decoder_from_nonexistent_path() {
        let result = create_decoder_from_path(Path::new("/nonexistent/file.mp3"));
        assert!(result.is_err());
    }
}
