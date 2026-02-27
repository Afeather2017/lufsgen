//! Audio decoder trait and format detection

use std::fs::File;
use std::io::Read;
use std::path::Path;

pub use self::mp3::Mp3Decoder;
pub use self::ogg::OggDecoder;
pub use self::wav::WavDecoder;

pub mod mp3;
mod ogg;
mod wav;

/// Supported audio formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Mp3,
    Ogg,
    Wav,
}

impl AudioFormat {
    /// Parse format from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "mp3" => Some(AudioFormat::Mp3),
            "ogg" => Some(AudioFormat::Ogg),
            "wav" => Some(AudioFormat::Wav),
            _ => None,
        }
    }

    /// Detect format from file path
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(AudioFormat::from_extension)
    }
}

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

/// Create a decoder for a given format from a reader
///
/// Note: The reader type must have 'static lifetime for boxing.
/// For OGG format, the reader must also implement Seek.
pub fn create_decoder<R: Read + 'static>(
    reader: R,
    format: AudioFormat,
) -> crate::error::Result<Box<dyn AudioDecoder>> {
    match format {
        AudioFormat::Mp3 => Ok(Box::new(Mp3Decoder::new(reader)?)),
        AudioFormat::Ogg => {
            // OGG requires Seek - we can't use generic R here
            // Users should use create_decoder_from_path for OGG files
            Err(crate::error::LufsError::UnsupportedFormat(
                "OGG requires seekable reader - use calculate_from_file() instead".to_string(),
            ))
        }
        AudioFormat::Wav => Ok(Box::new(WavDecoder::new(reader)?)),
    }
}

/// Create a decoder for a file path
///
/// This is the recommended way to create decoders as it handles
/// the Seek requirement for OGG files automatically.
pub fn create_decoder_from_path(path: &Path) -> crate::error::Result<Box<dyn AudioDecoder>> {
    let format = AudioFormat::from_path(path)
        .ok_or_else(|| crate::error::LufsError::UnsupportedFormat(
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown")
                .to_string(),
        ))?;

    let file = File::open(path)?;

    // For OGG, we need a seekable reader - File supports this
    if format == AudioFormat::Ogg {
        return Ok(Box::new(ogg::OggDecoder::new(file)?));
    }

    create_decoder(file, format)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_extension() {
        assert_eq!(AudioFormat::from_extension("mp3"), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::from_extension("MP3"), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::from_extension("Mp3"), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::from_extension("ogg"), Some(AudioFormat::Ogg));
        assert_eq!(AudioFormat::from_extension("wav"), Some(AudioFormat::Wav));
        assert_eq!(AudioFormat::from_extension("flac"), None);
        assert_eq!(AudioFormat::from_extension("txt"), None);
        assert_eq!(AudioFormat::from_extension(""), None);
    }

    #[test]
    fn test_format_from_path() {
        assert_eq!(AudioFormat::from_path(Path::new("test.mp3")), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::from_path(Path::new("test.ogg")), Some(AudioFormat::Ogg));
        assert_eq!(AudioFormat::from_path(Path::new("test.wav")), Some(AudioFormat::Wav));
        assert_eq!(AudioFormat::from_path(Path::new("test.flac")), None);
        assert_eq!(AudioFormat::from_path(Path::new("test")), None);
    }

    #[test]
    fn test_format_partial_eq() {
        assert_eq!(AudioFormat::Mp3, AudioFormat::Mp3);
        assert_ne!(AudioFormat::Mp3, AudioFormat::Wav);
    }
}
