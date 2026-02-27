//! MP3 audio decoder using minimp3 with chunk-based processing

use std::io::Read;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

use crate::error::{LufsError, Result};
use crate::decoders::AudioDecoder;

/// Progress reporter for MP3 decoding
pub type ProgressReporter = Arc<AtomicU64>;

/// MP3 decoder that streams audio data
///
/// Processes the file in large chunks (1MB) to balance memory usage and performance.
pub struct Mp3Decoder<R: Read> {
    reader: R,
    chunk_data: Vec<u8>,
    sample_queue: Vec<i16>,
    sample_rate: u32,
    channels: u32,
    chunk_index: usize,
    total_bytes_read: u64,
    #[allow(dead_code)]
    file_size: Option<u64>,
    progress: Option<ProgressReporter>,
    eof: bool,
}

impl<R: Read> Mp3Decoder<R> {
    const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB chunks

    /// Create a new MP3 decoder from a reader
    pub fn new(reader: R) -> Result<Self> {
        Self::new_with_progress(reader, None, None)
    }

    /// Create a new MP3 decoder with progress tracking
    pub fn new_with_progress(mut reader: R, file_size: Option<u64>, progress: Option<ProgressReporter>) -> Result<Self> {
        // Read first chunk to get stream info
        let (chunk_data, sample_rate, channels, initial_samples) =
            Self::read_first_chunk(&mut reader)?;

        let bytes_read = chunk_data.len() as u64;

        Ok(Mp3Decoder {
            reader,
            chunk_data,
            sample_queue: initial_samples,
            sample_rate,
            channels,
            chunk_index: 0,
            total_bytes_read: bytes_read,
            file_size,
            progress,
            eof: false,
        })
    }

    /// Read first chunk and extract stream info
    fn read_first_chunk(reader: &mut R) -> Result<(Vec<u8>, u32, u32, Vec<i16>)> {
        let mut chunk_data = vec![0u8; Self::CHUNK_SIZE];
        let n = reader.read(&mut chunk_data)?;
        chunk_data.truncate(n);

        if chunk_data.is_empty() {
            return Err(LufsError::InvalidData("Empty MP3 file".to_string()));
        }

        let mut decoder = minimp3::Decoder::new(&chunk_data[..]);

        match decoder.next_frame() {
            Ok(frame) => {
                let sample_rate = frame.sample_rate as u32;
                let channels = 2; // MP3 is typically stereo
                Ok((chunk_data, sample_rate, channels, frame.data))
            }
            Err(minimp3::Error::Eof) => {
                Err(LufsError::InvalidData("No valid MP3 frames found".to_string()))
            }
            Err(e) => Err(LufsError::DecodeError(format!(
                "MP3 header parse error: {:?}",
                e
            ))),
        }
    }

    /// Read and decode the next chunk
    fn read_next_chunk(&mut self) -> Result<bool> {
        if self.eof {
            return Ok(false);
        }

        self.chunk_index += 1;

        // For subsequent chunks, we need to account for overlap
        // because MP3 frames can span chunk boundaries
        let overlap = 4096; // Keep last 4KB for frame boundary handling
        let chunk_size = Self::CHUNK_SIZE;

        let mut new_chunk = vec![0u8; chunk_size];
        let n = self.reader.read(&mut new_chunk)?;

        if n == 0 {
            self.eof = true;

            // Try to decode any remaining data in the buffer
            if !self.chunk_data.is_empty() {
                self.decode_chunk_data()?;
            }

            return Ok(!self.sample_queue.is_empty());
        }

        new_chunk.truncate(n);
        self.total_bytes_read += n as u64;

        // Update progress if we have a reporter
        if let Some(ref progress) = self.progress {
            progress.store(self.total_bytes_read, Ordering::Relaxed);
        }

        // Keep some overlap data
        let keep_len = std::cmp::min(overlap, self.chunk_data.len());
        let overlap_data: Vec<u8> = self.chunk_data
            .drain(self.chunk_data.len() - keep_len..)
            .collect();

        self.chunk_data = overlap_data;
        self.chunk_data.extend_from_slice(&new_chunk);

        self.decode_chunk_data()?;

        Ok(true)
    }

    /// Decode all frames from current chunk_data
    fn decode_chunk_data(&mut self) -> Result<()> {
        let mut decoder = minimp3::Decoder::new(&self.chunk_data[..]);

        while let Ok(frame) = decoder.next_frame() {
            self.sample_queue.extend_from_slice(&frame.data);
        }

        Ok(())
    }
}

impl<R: Read> AudioDecoder for Mp3Decoder<R> {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u32 {
        self.channels
    }

    fn decode_chunk(&mut self) -> Result<Option<Vec<i16>>> {
        let chunk_size = 8192 * 2; // Target ~8192 stereo frames

        // Refill sample queue if needed
        while self.sample_queue.len() < chunk_size {
            let had_more = self.read_next_chunk()?;
            if !had_more {
                break; // EOF reached
            }
        }

        if self.sample_queue.is_empty() {
            return Ok(None); // EOF
        }

        // Take samples from queue
        let take = std::cmp::min(chunk_size, self.sample_queue.len());
        let result: Vec<i16> = self.sample_queue.drain(..take).collect();

        Ok(Some(result))
    }
}
