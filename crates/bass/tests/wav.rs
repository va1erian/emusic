//! Minimal 16-bit PCM WAV generation for tests (no external deps).

/// One second of 44100 Hz mono sine-edge samples and the raw bytes of a
/// valid 16-bit PCM WAV file containing them. Returns `(file_bytes,
/// sample_rate, channels)`.
pub fn mono_file_1s() -> (Vec<u8>, u32, u32) {
    const RATE: u32 = 44_100;
    let samples: Vec<i16> = (0..RATE as usize).map(sine).collect();
    (file(RATE, 1, &samples), RATE, 1)
}

fn sine(index: usize) -> i16 {
    const FREQ: f64 = 30.0;
    let t = index as f64 / 44_100.0;
    let value = (t * FREQ * std::f64::consts::TAU).sin() * (i16::MAX as f64 / 2.0);
    value as i16
}

/// Assembles `samples` into a standard 44-byte-header 16-bit WAV file.
fn file(rate: u32, channels: u32, samples: &[i16]) -> Vec<u8> {
    let bits = 16u16;
    let data_len = std::mem::size_of_val(samples) as u32;
    let byte_rate = rate * u32::from(bits) / 8 * channels;
    let block_align = u16::try_from(u32::from(bits) / 8 * channels).expect("block_align fits u16");

    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend(b"RIFF");
    out.extend((36 + data_len).to_le_bytes());
    out.extend(b"WAVE");
    out.extend(b"fmt ");
    out.extend(16u32.to_le_bytes()); // fmt chunk size
    out.extend(1u16.to_le_bytes()); // PCM
    out.extend(
        u16::try_from(channels)
            .expect("channels fits u16")
            .to_le_bytes(),
    );
    out.extend(rate.to_le_bytes());
    out.extend(byte_rate.to_le_bytes());
    out.extend(block_align.to_le_bytes());
    out.extend(bits.to_le_bytes());
    out.extend(b"data");
    out.extend(data_len.to_le_bytes());
    for sample in samples {
        out.extend(sample.to_le_bytes());
    }
    out
}
