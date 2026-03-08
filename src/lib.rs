//! LUFS Generator Library for Android
//!
//! A memory-efficient streaming LUFS (Loudness Units Full Scale) calculator
//! that processes audio chunk-by-chunk without loading entire files into memory.
//!
//! # Features
//!
//! - Streaming audio decoders for MP3, OGG, WAV, FLAC, AAC, M4A, and MP4 formats
//! - Automatic format detection from stream content (magic bytes)
//! - Memory-efficient chunk-based processing
//! - EBU R128 compliant loudness measurement
//!
//! # Example
//!
//! ```rust,no_run
//! use lufsgen::LufsCalculator;
//! use std::path::Path;
//!
//! let calc = LufsCalculator::default();
//!
//! // From file path - format is auto-detected
//! let lufs = calc.calculate_from_file(Path::new("song.mp3"))?;
//!
//! // Supports many formats: mp3, ogg, wav, flac, aac, m4a, mp4
//! let lufs_flac = calc.calculate_from_file(Path::new("song.flac"))?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod error;
pub mod decoders;
pub mod lufs;

// Public API re-exports
pub use error::{LufsError, Result};
pub use decoders::{AudioDecoder, SymphoniaDecoder, create_decoder, create_decoder_from_path};
pub use lufs::{LufsCalculator, LufsResult};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Supported audio file extensions
///
/// Includes all formats supported by Symphonia:
/// - WAV: wav
/// - MP3: mp3
/// - OGG/Vorbis: ogg, oga
/// - FLAC: flac
/// - AAC: aac
/// - MP4/M4A: m4a, mp4 (audio-only)
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "wav", "mp3", "ogg", "oga", "flac", "aac", "m4a", "mp4",
];

/// Check if a file has a supported audio extension
pub fn is_audio_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}
