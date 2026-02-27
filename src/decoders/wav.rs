//! WAV audio decoder using hound

use std::io::Read;

use crate::error::{LufsError, Result};
use crate::decoders::AudioDecoder;

/// WAV decoder that streams audio data
pub struct WavDecoder<R: Read> {
    reader: hound::WavIntoSamples<R, i16>,
    sample_rate: u32,
    channels: u32,
}

impl<R: Read> WavDecoder<R> {
    /// Create a new WAV decoder from a reader
    pub fn new(reader: R) -> Result<Self> {
        // First create a WavReader to get the spec
        let wav_reader = hound::WavReader::new(reader)
            .map_err(|e| LufsError::DecodeError(format!("WAV header error: {}", e)))?;

        let spec = wav_reader.spec();
        let sample_rate = spec.sample_rate as u32;
        let channels = spec.channels as u32;

        // Only support i16 format for now
        if spec.sample_format != hound::SampleFormat::Int {
            return Err(LufsError::UnsupportedFormat(
                "Only 16-bit PCM WAV is supported".to_string(),
            ));
        }

        // Validate bits per sample
        if spec.bits_per_sample != 16 {
            return Err(LufsError::DecodeError(format!(
                "Unsupported bits per sample: {} (only 16-bit supported)",
                spec.bits_per_sample
            )));
        }

        // Convert to streaming samples reader
        let reader = wav_reader.into_samples();

        Ok(WavDecoder {
            reader,
            sample_rate,
            channels,
        })
    }
}

impl<R: Read> AudioDecoder for WavDecoder<R> {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u32 {
        self.channels
    }

    fn decode_chunk(&mut self) -> Result<Option<Vec<i16>>> {
        // Read a reasonable chunk size (8192 samples)
        let chunk_size = 8192;
        let mut samples = Vec::with_capacity(chunk_size);

        for _ in 0..chunk_size {
            match self.reader.next() {
                Some(Ok(sample)) => samples.push(sample),
                Some(Err(e)) => {
                    return Err(LufsError::DecodeError(format!("WAV read error: {}", e)))
                }
                None => break,
            }
        }

        if samples.is_empty() {
            Ok(None)
        } else {
            Ok(Some(samples))
        }
    }
}
