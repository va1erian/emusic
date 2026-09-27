#![forbid(unsafe_code)]

//! FLAC encoding with the pure-Rust [`flacenc`] crate.
//!
//! `flacenc` has no C dependency and no build script, so the server image
//! redistributes only Rust code (Apache-2.0). Only signed 16-bit PCM is
//! encoded; that is the shape every renderer in this crate produces.

use std::io::Write;

use flacenc::component::BitRepr;
use flacenc::error::Verify;

use crate::error::RenderError;
use crate::pcm::PcmBuffer;

/// Bits per encoded sample. [`PcmBuffer`] is always `i16`.
const BITS_PER_SAMPLE: usize = 16;

/// Encodes `pcm` to a complete FLAC stream in memory.
pub fn encode(pcm: &PcmBuffer) -> Result<Vec<u8>, RenderError> {
    let samples = to_i32(pcm);
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, error)| RenderError::Flac(format!("invalid encoder config: {error:?}")))?;
    let source = flacenc::source::MemSource::from_samples(
        &samples,
        usize::from(pcm.channels()),
        BITS_PER_SAMPLE,
        pcm.sample_rate() as usize,
    );
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|error| RenderError::Flac(format!("encoding failed: {error}")))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|error| RenderError::Flac(format!("bitstream failed: {error}")))?;
    Ok(sink.as_slice().to_vec())
}

/// Encodes `pcm` to a complete FLAC stream and writes it to `writer`.
pub fn encode_into(pcm: &PcmBuffer, writer: &mut impl Write) -> Result<(), RenderError> {
    let bytes = encode(pcm)?;
    writer
        .write_all(&bytes)
        .map_err(|error| RenderError::Flac(format!("write failed: {error}")))
}

/// Widens the interleaved `i16` samples to the `i32` `flacenc` expects.
fn to_i32(pcm: &PcmBuffer) -> Vec<i32> {
    pcm.samples()
        .iter()
        .map(|&sample| i32::from(sample))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 0.1 s, 440 Hz stereo tone so the FLAC frames carry real signal.
    fn tone(sample_rate: u32, channels: u16, frames: usize) -> PcmBuffer {
        let mut samples = Vec::with_capacity(frames * usize::from(channels));
        for frame in 0..frames {
            let phase = 2.0 * std::f32::consts::PI * 440.0 * frame as f32 / sample_rate as f32;
            let value = (phase.sin() * 12_000.0) as i16;
            for _ in 0..channels {
                samples.push(value);
            }
        }
        PcmBuffer::new(sample_rate, channels, samples).expect("valid PCM")
    }

    #[test]
    fn encode_decode_round_trip_is_lossless() {
        let pcm = tone(44_100, 2, 4_410);
        let bytes = encode(&pcm).expect("encode");

        // The FLAC signature marks a real stream, not a bare header.
        assert_eq!(&bytes[..4], b"fLaC");

        let mut reader = claxon::FlacReader::new(&bytes[..]).expect("decode");
        let info = reader.streaminfo();
        assert_eq!(info.sample_rate, 44_100);
        assert_eq!(info.channels, 2);
        assert_eq!(info.bits_per_sample, 16);

        let decoded: Vec<i16> = reader
            .samples()
            .map(|sample| sample.expect("sample") as i16)
            .collect();
        assert_eq!(decoded, pcm.samples());
    }

    #[test]
    fn encode_into_writes_the_same_bytes_as_encode() {
        let pcm = tone(44_100, 1, 1_000);
        let expected = encode(&pcm).expect("encode");
        let mut written = Vec::new();
        encode_into(&pcm, &mut written).expect("encode_into");
        assert_eq!(written, expected);
    }
}
