//! Deterministic PCM signal generation for tests (no external deps).

/// `seconds` of `freq` Hz mono `f32` sine at `rate` Hz, amplitude `0.5`.
pub fn f32_sine(rate: f64, freq: f64, seconds: f64) -> Vec<f32> {
    let count = (rate * seconds).round() as usize;
    (0..count)
        .map(|index| {
            let t = index as f64 / rate;
            ((t * freq * std::f64::consts::TAU).sin() * 0.5) as f32
        })
        .collect()
}

/// The little-endian `f32` bytes of `samples`, as BASS expects for a
/// `FLOAT` stream.
pub fn f32_bytes(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(std::mem::size_of_val(samples));
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}
