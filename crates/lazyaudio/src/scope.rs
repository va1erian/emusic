//! The visualizer's view of a channel: the last frames that played, as mono
//! samples and as an FFT spectrum, matching what BASS's `ChannelGetData`
//! gives the BASS backend (1024 samples, 512 magnitude bins).

use std::collections::VecDeque;
use std::f32::consts::PI;

/// Samples (and FFT points) per read.
pub const WINDOW: usize = 1024;

/// The most decoded frames kept: the window plus everything queued ahead of
/// the speaker, with room to spare.
const KEEP: usize = 1 << 16;

/// Recently decoded frames, downmixed to mono, and how many of the newest
/// have not played yet.
#[derive(Default)]
pub struct Scope {
    history: VecDeque<f32>,
    unplayed: usize,
}

impl Scope {
    /// Appends decoded interleaved `samples` of `channels`-sample frames; they
    /// have not played yet.
    pub fn decoded(&mut self, samples: &[f32], channels: usize) {
        let channels = channels.max(1);
        for frame in samples.chunks_exact(channels) {
            self.history
                .push_back(frame.iter().sum::<f32>() / channels as f32);
        }
        self.unplayed += samples.len() / channels;
        let excess = self.history.len().saturating_sub(KEEP);
        self.history.drain(..excess);
        self.unplayed = self.unplayed.min(self.history.len());
    }

    /// Records that `frames` more of the decoded frames have played.
    pub fn played(&mut self, frames: u64) {
        self.unplayed = self.unplayed.saturating_sub(frames as usize);
    }

    /// Forgets everything, for a seek.
    pub fn clear(&mut self) {
        self.history.clear();
        self.unplayed = 0;
    }

    /// The last [`WINDOW`] frames that played, oldest first, or `None` before
    /// any did.
    pub fn samples(&self) -> Option<Vec<f32>> {
        let end = self.history.len() - self.unplayed;
        if end == 0 {
            return None;
        }
        let start = end.saturating_sub(WINDOW);
        let mut window: Vec<f32> = self.history.range(start..end).copied().collect();
        // Left-pad a short history so every read has the same length.
        if window.len() < WINDOW {
            let mut padded = vec![0.0; WINDOW - window.len()];
            padded.append(&mut window);
            window = padded;
        }
        Some(window)
    }

    /// The magnitude spectrum of [`samples`](Self::samples) (Hann-windowed),
    /// `WINDOW / 2` bins from 0 Hz up, each in `0.0..=1.0` for a full-scale
    /// sine.
    pub fn fft(&self) -> Option<Vec<f32>> {
        let samples = self.samples()?;
        Some(spectrum(&samples))
    }
}

/// The magnitude spectrum of `samples` (a power of two long).
pub fn spectrum(samples: &[f32]) -> Vec<f32> {
    let n = samples.len();
    let mut re: Vec<f32> = samples
        .iter()
        .enumerate()
        .map(|(i, &s)| s * 0.5 * (1.0 - (2.0 * PI * i as f32 / n as f32).cos()))
        .collect();
    let mut im = vec![0.0; n];
    fft_in_place(&mut re, &mut im);
    // A Hann window halves a sine's amplitude; a real sine splits its energy
    // between two bins: 4/n brings a full-scale sine back to 1.
    (0..n / 2)
        .map(|k| (re[k].hypot(im[k]) * 4.0 / n as f32).min(1.0))
        .collect()
}

/// An iterative radix-2 FFT over `re`/`im` (a power of two long).
fn fft_in_place(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * PI / len as f32;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (sin, cos) = (angle * k as f32).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * cos - im[b] * sin;
                let ti = re[b] * sin + im[b] * cos;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sine_peaks_in_its_bin() {
        // Bin 64 of 1024 points at 44.1 kHz is 2756 Hz; any bin-centred sine
        // works.
        let samples: Vec<f32> = (0..WINDOW)
            .map(|i| (2.0 * PI * 64.0 * i as f32 / WINDOW as f32).sin())
            .collect();
        let bins = spectrum(&samples);
        let peak = (0..bins.len())
            .max_by(|&a, &b| bins[a].total_cmp(&bins[b]))
            .unwrap();
        assert_eq!(peak, 64);
        assert!((bins[64] - 1.0).abs() < 0.05, "{}", bins[64]);
    }

    #[test]
    fn only_played_frames_are_visible() {
        let mut scope = Scope::default();
        scope.decoded(&[0.5, 0.5, -0.5, -0.5], 2);
        assert_eq!(scope.samples(), None);
        scope.played(1);
        let window = scope.samples().unwrap();
        assert_eq!(window.len(), WINDOW);
        assert_eq!(window[WINDOW - 1], 0.5);
        scope.played(1);
        assert_eq!(scope.samples().unwrap()[WINDOW - 1], -0.5);
    }
}
