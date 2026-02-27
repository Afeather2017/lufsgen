//! Unified audio decoder using Symphonia
//!
//! Supports MP3, OGG, FLAC, AAC, M4A, MP4, WAV, and more.
//! Format is detected from stream content (magic bytes), not file extension.

use std::io::{Cursor, Read, Seek, SeekFrom};

use crate::error::{LufsError, Result};
use crate::decoders::AudioDecoder;

// Re-export Symphonia types for convenience
use symphonia::core::audio::AudioBuffer;
use symphonia::core::codecs::{DecoderOptions};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::{Hint};

/// In-memory media source for streaming
///
/// Wraps a Cursor to provide seekable access to buffered audio data.
struct InMemorySource {
    cursor: Cursor<Vec<u8>>,
}

impl InMemorySource {
    fn new(data: Vec<u8>) -> Self {
        Self { cursor: Cursor::new(data) }
    }
}

impl Read for InMemorySource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.cursor.read(buf)
    }
}

impl Seek for InMemorySource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(pos)
    }
}

impl MediaSource for InMemorySource {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.cursor.get_ref().len() as u64)
    }
}

/// Unified audio decoder using Symphonia
///
/// Automatically detects format from stream content and supports:
/// - MP3 (mp3)
/// - OGG/Vorbis (ogg, oga)
/// - FLAC (flac)
/// - AAC (aac)
/// - M4A/MP4 (m4a, mp4)
/// - WAV (wav)
pub struct SymphoniaDecoder {
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    sample_rate: u32,
    channels: u32,
    format: Box<dyn symphonia::core::formats::FormatReader>,
    ended: bool,
}

impl SymphoniaDecoder {
    /// Create a new decoder from a reader with automatic format detection
    ///
    /// This is the recommended way to create a decoder as it detects
    /// the format from the stream content (magic bytes) rather than
    /// relying on file extensions.
    pub fn new<R: Read>(mut reader: R) -> Result<Self> {
        // Read all data into memory for seekable access
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;

        if buffer.is_empty() {
            return Err(LufsError::InvalidData("Empty audio file".to_string()));
        }

        // Create a seekable media source from the buffered data
        let source = Box::new(InMemorySource::new(buffer));

        let mss = MediaSourceStream::new(
            source,
            MediaSourceStreamOptions::default(),
        );

        // Probe the format - Symphonia detects from magic bytes
        let hint = Hint::new();

        // Use the default probe to detect format
        let probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| {
            LufsError::DecodeError(format!(
                "Failed to detect audio format: {}. The file may be corrupt or use an unsupported format.",
                e
            ))
        })?;

        // Get the format reader
        let format = probed.format;

        // Get the default track
        let track = format
            .default_track()
            .ok_or_else(|| LufsError::InvalidData("No audio track found".to_string()))?;

        // Get codec parameters
        let codec_params = &track.codec_params;

        let sample_rate = codec_params
            .sample_rate
            .ok_or_else(|| LufsError::InvalidData("Missing sample rate".to_string()))?;

        let channels = codec_params
            .channels
            .map(|c| c.count() as u32)
            .unwrap_or(2); // Default to stereo

        // Create the decoder - pass reference to codec_params
        let decoder = symphonia::default::get_codecs()
            .make(codec_params, &DecoderOptions::default())
            .map_err(|e| {
                LufsError::DecodeError(format!(
                    "Failed to create decoder for codec: {}. The audio format may not be supported.",
                    e
                ))
            })?;

        Ok(SymphoniaDecoder {
            decoder,
            sample_rate,
            channels,
            format,
            ended: false,
        })
    }

    /// Decode the next audio packet and return samples
    ///
    /// Returns decoded samples as i16 PCM. Handles various sample formats
    /// and automatically converts to i16.
    fn decode_next_packet(&mut self) -> Result<Option<Vec<i16>>> {
        if self.ended {
            return Ok(None);
        }

        // Get the next packet from the format reader
        let packet = match self.format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::ResetRequired) => {
                // Format reset required - this can happen with some formats
                return Ok(None);
            }
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                // End of file
                self.ended = true;
                return Ok(None);
            }
            Err(e) => {
                return Err(LufsError::DecodeError(format!("Packet read error: {}", e)));
            }
        };

        // Decode the packet
        let decoded_buf = match self.decoder.decode(&packet) {
            Ok(buf) => buf,
            Err(symphonia::core::errors::Error::IoError(_))
            | Err(symphonia::core::errors::Error::DecodeError(_)) => {
                // Decode errors are not fatal - return empty samples
                return Ok(Some(Vec::new()));
            }
            Err(e) => {
                return Err(LufsError::DecodeError(format!("Decode error: {}", e)));
            }
        };

        // Convert audio buffer to i16 samples
        let samples = Self::audio_buffer_to_i16(&decoded_buf);
        Ok(Some(samples))
    }

    /// Convert Symphonia audio buffer to i16 samples
    ///
    /// Symphonia returns audio as AudioBufferRef which can contain different sample types.
    /// We convert all types to i16.
    fn audio_buffer_to_i16(buf: &symphonia::core::audio::AudioBufferRef<'_>) -> Vec<i16> {
        match buf {
            symphonia::core::audio::AudioBufferRef::F32(buf_f32) => {
                Self::convert_f32_to_i16(buf_f32.as_ref())
            }
            symphonia::core::audio::AudioBufferRef::S32(buf_s32) => {
                Self::convert_s32_to_i16(buf_s32.as_ref())
            }
            symphonia::core::audio::AudioBufferRef::S16(buf_s16) => {
                Self::convert_s16_to_i16(buf_s16.as_ref())
            }
            symphonia::core::audio::AudioBufferRef::S24(buf_s24) => {
                Self::convert_i24_to_i16(buf_s24.as_ref())
            }
            symphonia::core::audio::AudioBufferRef::U8(buf_u8) => {
                Self::convert_u8_to_i16(buf_u8.as_ref())
            }
            symphonia::core::audio::AudioBufferRef::U16(buf_u16) => {
                Self::convert_u16_to_i16(buf_u16.as_ref())
            }
            symphonia::core::audio::AudioBufferRef::U24(buf_u24) => {
                Self::convert_u24_to_i16(buf_u24.as_ref())
            }
            symphonia::core::audio::AudioBufferRef::U32(buf_u32) => {
                Self::convert_u32_to_i16(buf_u32.as_ref())
            }
            symphonia::core::audio::AudioBufferRef::S8(buf_s8) => {
                Self::convert_s8_to_i16(buf_s8.as_ref())
            }
            symphonia::core::audio::AudioBufferRef::F64(buf_f64) => {
                Self::convert_f64_to_i16(buf_f64.as_ref())
            }
        }
    }

    fn convert_f32_to_i16(buf: &AudioBuffer<f32>) -> Vec<i16> {
        let spec = *buf.spec();
        let channels = spec.channels.count();
        let planes = buf.planes();

        let mut result = Vec::new();

        // Process each plane (channel)
        for plane_idx in 0..channels {
            let plane = &planes.planes()[plane_idx];
            for &sample in plane.iter() {
                let i16_sample: i16 = (sample.clamp(-1.0_f32, 1.0_f32) * 32767.0) as i16;
                result.push(i16_sample);
            }
        }

        result
    }

    fn convert_f64_to_i16(buf: &AudioBuffer<f64>) -> Vec<i16> {
        let spec = *buf.spec();
        let channels = spec.channels.count();
        let planes = buf.planes();

        let mut result = Vec::new();

        for plane_idx in 0..channels {
            let plane = &planes.planes()[plane_idx];
            for &sample in plane.iter() {
                let i16_sample: i16 = (sample.clamp(-1.0_f64, 1.0_f64) * 32767.0) as i16;
                result.push(i16_sample);
            }
        }

        result
    }

    fn convert_s32_to_i16(buf: &AudioBuffer<i32>) -> Vec<i16> {
        let spec = *buf.spec();
        let channels = spec.channels.count();
        let planes = buf.planes();

        let mut result = Vec::new();

        let max_val = i32::MAX as f64;
        for plane_idx in 0..channels {
            let plane = &planes.planes()[plane_idx];
            for &sample in plane.iter() {
                let sample_f64 = sample as f64 / max_val;
                let i16_sample = (sample_f64.clamp(-1.0, 1.0) * 32767.0) as i16;
                result.push(i16_sample);
            }
        }

        result
    }

    fn convert_s16_to_i16(buf: &AudioBuffer<i16>) -> Vec<i16> {
        let spec = *buf.spec();
        let channels = spec.channels.count();
        let planes = buf.planes();

        let mut result = Vec::new();

        for plane_idx in 0..channels {
            let plane = &planes.planes()[plane_idx];
            for &sample in plane.iter() {
                result.push(sample);
            }
        }

        result
    }

    fn convert_s8_to_i16(buf: &AudioBuffer<i8>) -> Vec<i16> {
        let spec = *buf.spec();
        let channels = spec.channels.count();
        let planes = buf.planes();

        let mut result = Vec::new();

        for plane_idx in 0..channels {
            let plane = &planes.planes()[plane_idx];
            for &sample in plane.iter() {
                // i8 ranges from -128 to 127
                let sample_f32 = sample as f32 / 128.0;
                let i16_sample = (sample_f32.clamp(-1.0, 1.0) * 32767.0) as i16;
                result.push(i16_sample);
            }
        }

        result
    }

    fn convert_i24_to_i16(buf: &AudioBuffer<symphonia::core::sample::i24>) -> Vec<i16> {
        let spec = *buf.spec();
        let channels = spec.channels.count();
        let planes = buf.planes();

        let mut result = Vec::new();

        for plane_idx in 0..channels {
            let plane = &planes.planes()[plane_idx];
            for &sample in plane.iter() {
                // i24 stores samples as i32 with values scaled by 256
                let inner_val = sample.0;
                let sample_f64 = inner_val as f64 / (i32::MAX as f64 / 256.0);
                let i16_sample = (sample_f64.clamp(-1.0, 1.0) * 32767.0) as i16;
                result.push(i16_sample);
            }
        }

        result
    }

    fn convert_u8_to_i16(buf: &AudioBuffer<u8>) -> Vec<i16> {
        let spec = *buf.spec();
        let channels = spec.channels.count();
        let planes = buf.planes();

        let mut result = Vec::new();

        for plane_idx in 0..channels {
            let plane = &planes.planes()[plane_idx];
            for &sample in plane.iter() {
                // U8 ranges from 0-255, center at 128
                let sample_f32 = (sample as f32 - 128.0) / 128.0;
                let i16_sample = (sample_f32.clamp(-1.0, 1.0) * 32767.0) as i16;
                result.push(i16_sample);
            }
        }

        result
    }

    fn convert_u16_to_i16(buf: &AudioBuffer<u16>) -> Vec<i16> {
        let spec = *buf.spec();
        let channels = spec.channels.count();
        let planes = buf.planes();

        let mut result = Vec::new();

        for plane_idx in 0..channels {
            let plane = &planes.planes()[plane_idx];
            for &sample in plane.iter() {
                // U16 ranges from 0-65535, center at 32768
                let sample_f32 = (sample as f32 - 32768.0) / 32768.0;
                let i16_sample = (sample_f32.clamp(-1.0, 1.0) * 32767.0) as i16;
                result.push(i16_sample);
            }
        }

        result
    }

    fn convert_u24_to_i16(buf: &AudioBuffer<symphonia::core::sample::u24>) -> Vec<i16> {
        let spec = *buf.spec();
        let channels = spec.channels.count();
        let planes = buf.planes();

        let mut result = Vec::new();

        for plane_idx in 0..channels {
            let plane = &planes.planes()[plane_idx];
            for &sample in plane.iter() {
                // u24 stores samples as u32 with values scaled by 256
                let inner_val = sample.0;
                let max_val = (u32::MAX >> 8) as f64;
                let sample_f64 = (inner_val as f64 - max_val / 2.0) / (max_val / 2.0);
                let i16_sample = (sample_f64.clamp(-1.0, 1.0) * 32767.0) as i16;
                result.push(i16_sample);
            }
        }

        result
    }

    fn convert_u32_to_i16(buf: &AudioBuffer<u32>) -> Vec<i16> {
        let spec = *buf.spec();
        let channels = spec.channels.count();
        let planes = buf.planes();

        let mut result = Vec::new();

        for plane_idx in 0..channels {
            let plane = &planes.planes()[plane_idx];
            for &sample in plane.iter() {
                let max_val = u32::MAX as f64;
                let sample_f64 = (sample as f64 - max_val / 2.0) / (max_val / 2.0);
                let i16_sample = (sample_f64.clamp(-1.0, 1.0) * 32767.0) as i16;
                result.push(i16_sample);
            }
        }

        result
    }
}

impl AudioDecoder for SymphoniaDecoder {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u32 {
        self.channels
    }

    fn decode_chunk(&mut self) -> Result<Option<Vec<i16>>> {
        let mut all_samples = Vec::new();
        let target_samples = 8192 * self.channels as usize; // ~8192 frames

        // Decode packets until we have enough samples or hit EOF
        while all_samples.len() < target_samples {
            match self.decode_next_packet()? {
                Some(mut samples) => {
                    if samples.is_empty() {
                        // Decoder signaled skip but not EOF
                        continue;
                    }
                    all_samples.append(&mut samples);
                }
                None => {
                    if all_samples.is_empty() {
                        return Ok(None); // EOF
                    }
                    break;
                }
            }
        }

        Ok(Some(all_samples))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_source_is_seekable() {
        let data = vec![0u8; 1024];
        let source = InMemorySource::new(data);
        assert!(source.is_seekable());
        assert_eq!(source.byte_len(), Some(1024));
    }

    #[test]
    fn test_empty_data_error() {
        let empty: &[u8] = &[];
        let reader = std::io::Cursor::new(empty);
        let result = SymphoniaDecoder::new(reader);
        assert!(matches!(result, Err(LufsError::InvalidData(_))));
    }
}
