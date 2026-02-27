//! OGG/Vorbis audio decoder using lewton
//!
//! Note: OGG decoding requires Seek capability due to how lewton's OggStreamReader works.

use std::io::{BufReader, Read, Seek};

use crate::error::{LufsError, Result};
use crate::decoders::AudioDecoder;

/// OGG/Vorbis decoder that streams audio data
///
/// Note: Requires Seek trait in addition to Read.
pub struct OggDecoder<R: Read + Seek> {
    // We need to own the BufReader to manage lifetimes
    reader: BufReader<R>,
    sample_rate: u32,
    channels: u32,
}

impl<R: Read + Seek> OggDecoder<R> {
    /// Create a new OGG decoder from a reader
    pub fn new(reader: R) -> Result<Self> {
        let mut buf_reader = BufReader::new(reader);

        // Create the OGG stream reader to get header info
        let ogg_reader = lewton::inside_ogg::OggStreamReader::new(&mut buf_reader)
            .map_err(|e| LufsError::DecodeError(format!("OGG header error: {}", e)))?;

        let ident_hdr = ogg_reader.ident_hdr;
        let sample_rate = ident_hdr.audio_sample_rate as u32;
        let channels = ident_hdr.audio_channels as u32;

        Ok(OggDecoder {
            reader: buf_reader,
            sample_rate,
            channels,
        })
    }
}

impl<R: Read + Seek> AudioDecoder for OggDecoder<R> {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u32 {
        self.channels
    }

    fn decode_chunk(&mut self) -> Result<Option<Vec<i16>>> {
        // Create a new decoder each time since we can't store it with lifetime issues
        // This is less efficient but works with the Read trait constraint
        let mut decoder = lewton::inside_ogg::OggStreamReader::new(&mut self.reader)
            .map_err(|e| LufsError::DecodeError(format!("OGG read error: {}", e)))?;

        // Read one packet at a time
        match decoder.read_dec_packet_itl() {
            Ok(Some(packet)) => Ok(Some(packet)),
            Ok(None) => Ok(None), // EOF
            Err(e) => Err(LufsError::DecodeError(format!("OGG decode error: {}", e))),
        }
    }
}
