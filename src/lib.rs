//! LUFS Generator Library for Android
//!
//! A memory-efficient streaming LUFS (Loudness Units Full Scale) calculator
//! that processes audio chunk-by-chunk without loading entire files into memory.
//!
//! # Features
//!
//! - Streaming audio decoders for MP3, OGG, and WAV formats
//! - Memory-efficient chunk-based processing
//! - EBU R128 compliant loudness measurement
//!
//! # Example
//!
//! ```rust,no_run
//! use lufsgen_android::{LufsCalculator, AudioFormat};
//! use std::fs::File;
//! use std::path::Path;
//!
//! // From file path
//! let calc = LufsCalculator::default();
//! let lufs = calc.calculate_from_file(Path::new("song.mp3"))?;
//!
//! // From custom reader with explicit format
//! let file = File::open("song.wav")?;
//! let lufs = calc.calculate_from_reader(file, AudioFormat::Wav)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod error;
pub mod decoders;
pub mod lufs;

// Public API re-exports
pub use error::{LufsError, Result};
pub use decoders::{AudioDecoder, AudioFormat, create_decoder, create_decoder_from_path};
pub use lufs::{LufsCalculator, LufsResult};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Supported audio file extensions
pub const SUPPORTED_EXTENSIONS: &[&str] = &["wav", "mp3", "ogg"];

/// Check if a file has a supported audio extension
pub fn is_audio_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}
