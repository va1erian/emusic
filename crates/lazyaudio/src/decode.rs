//! MP3 decoding: a seekable byte source in, interleaved `f32` frames out.
//!
//! This module names nothing from emusic (it is half of the seam a shared
//! media library would lift out, with [`crate::output`]). It wraps symphonia's
//! MPEG audio reader and decoder: the reader probes past ID3 tags and junk,
//! reads the Xing/LAME header for the length and the encoder delay and
//! padding (which the decoder trims), and seeks; [`Mp3Decoder::seek`] then
//! decodes from the frame before the target and discards up to it, so a seek
//! lands on the exact sample.

use std::io::{Read, Seek, SeekFrom};
use std::time::Duration;

use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::Timestamp;

/// Frames decoded and dropped before a seek target: four MPEG-1 frames, so
/// the overlap and the bit reservoir are primed when the target plays.
const PREROLL_FRAMES: u64 = 4 * 1152;

/// Why a stream could not be opened, decoded or sought.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The bytes are not an MPEG audio stream this decoder plays.
    #[error("not a playable MP3 stream: {0}")]
    Unsupported(String),
    /// Reading the source failed.
    #[error("read failed: {0}")]
    Io(#[from] std::io::Error),
    /// The stream is damaged beyond what the decoder skips.
    #[error("decode failed: {0}")]
    Decode(String),
}

/// The decoder's result type.
pub type Result<T> = std::result::Result<T, Error>;

/// How precisely [`Mp3Decoder::seek`] lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekPrecision {
    /// On the requested sample.
    Exact,
    /// Near it: the stream's length is unknown, so a position past the end
    /// lands on the last frame read.
    Approximate,
}

/// What a stream is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    /// Frames per second.
    pub rate: u32,
    /// Samples per frame (1 or 2).
    pub channels: u16,
    /// Playable frames (encoder delay and padding excluded), when the header
    /// says.
    pub frames: Option<u64>,
    /// [`frames`](Self::frames) as a duration.
    pub duration: Option<Duration>,
    /// How precisely a seek lands.
    pub seek: SeekPrecision,
}

/// A byte source symphonia can read: any `Read + Seek` that can cross
/// threads, with its length (which lets the reader find an ID3v1 tag at the
/// end instead of decoding it as audio).
struct Source<R> {
    inner: R,
    len: Option<u64>,
}

impl<R: Seek> Source<R> {
    fn new(mut inner: R) -> std::io::Result<Source<R>> {
        let start = inner.stream_position()?;
        let end = inner.seek(SeekFrom::End(0))?;
        inner.seek(SeekFrom::Start(start))?;
        Ok(Source {
            inner,
            len: Some(end),
        })
    }
}

impl<R: Read> Read for Source<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<R: Seek> Seek for Source<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl<R: Read + Seek + Send + Sync> MediaSource for Source<R> {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.len
    }
}

/// An open MP3 stream.
pub struct Mp3Decoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track: u32,
    info: StreamInfo,
    /// Decoded samples not yet handed out, from `pending_at` on.
    pending: Vec<f32>,
    pending_at: usize,
    /// Frames still to drop after a seek, to land on the requested one.
    skip: u64,
}

impl Mp3Decoder {
    /// Opens `source` as an MP3 stream.
    pub fn open<R: Read + Seek + Send + Sync + 'static>(source: R) -> Result<Mp3Decoder> {
        let stream = MediaSourceStream::new(Box::new(Source::new(source)?), Default::default());
        let mut hint = Hint::new();
        hint.with_extension("mp3");
        let format = symphonia::default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|error| Error::Unsupported(error.to_string()))?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| Error::Unsupported("no audio track".into()))?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .ok_or_else(|| Error::Unsupported("no audio parameters".into()))?;
        let rate = params
            .sample_rate
            .filter(|&rate| rate > 0)
            .ok_or_else(|| Error::Unsupported("no sample rate".into()))?;
        let channels = params
            .channels
            .as_ref()
            .map(|channels| channels.count())
            .filter(|&count| count == 1 || count == 2)
            .ok_or_else(|| Error::Unsupported("not mono or stereo".into()))?;
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|error| Error::Unsupported(error.to_string()))?;
        let frames = track.num_frames;
        let info = StreamInfo {
            rate,
            channels: channels as u16,
            frames,
            duration: frames.map(|frames| frames_to_duration(frames, rate)),
            seek: if frames.is_some() {
                SeekPrecision::Exact
            } else {
                SeekPrecision::Approximate
            },
        };
        let track = track.id;
        Ok(Mp3Decoder {
            format,
            decoder,
            track,
            info,
            pending: Vec::new(),
            pending_at: 0,
            skip: 0,
        })
    }

    /// What the stream is.
    pub fn info(&self) -> &StreamInfo {
        &self.info
    }

    /// Fills `out` with interleaved samples, whole frames only; returns how
    /// many samples it wrote, 0 at the end of the stream. Damaged frames are
    /// skipped; a truncated stream ends where its data does.
    pub fn read(&mut self, out: &mut [f32]) -> Result<usize> {
        let channels = usize::from(self.info.channels);
        let wanted = out.len() / channels * channels;
        let mut written = 0;
        while written < wanted {
            if self.pending_at == self.pending.len() && !self.decode_next()? {
                break;
            }
            let available = &self.pending[self.pending_at..];
            let count = available.len().min(wanted - written);
            out[written..written + count].copy_from_slice(&available[..count]);
            self.pending_at += count;
            written += count;
        }
        Ok(written)
    }

    /// Moves to `position` (clamped to the stream); returns where it landed.
    pub fn seek(&mut self, position: Duration) -> Result<Duration> {
        let mut target = duration_to_frames(position, self.info.rate);
        if let Some(frames) = self.info.frames {
            target = target.min(frames);
        }
        // Start decoding a little early: an MP3 frame overlaps the one before
        // it, so the first frames after a reset are not yet the stream's.
        let from = target.saturating_sub(PREROLL_FRAMES);
        let ts = Timestamp::try_from(from).unwrap_or(Timestamp::new(i64::MAX));
        let seeked = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Timestamp {
                ts,
                track_id: self.track,
            },
        );
        self.decoder.reset();
        self.pending.clear();
        self.pending_at = 0;
        match seeked {
            Ok(seeked) => {
                let actual = u64::try_from(seeked.actual_ts.get()).unwrap_or(0);
                self.skip = target.saturating_sub(actual);
                Ok(frames_to_duration(target, self.info.rate))
            }
            // Past the end of a stream of unknown length: nothing is left.
            Err(SymphoniaError::SeekError(_)) => {
                self.skip = u64::MAX;
                Ok(position)
            }
            Err(error) => Err(Error::Decode(error.to_string())),
        }
    }

    /// Decodes the next packet of the track into `pending`, dropping any
    /// frames a seek left to skip; false at the end of the stream.
    fn decode_next(&mut self) -> Result<bool> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => return Ok(false),
                // A truncated stream ends where its data does.
                Err(SymphoniaError::IoError(_)) => return Ok(false),
                Err(error) => return Err(Error::Decode(error.to_string())),
            };
            if packet.track_id != self.track {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buffer) => {
                    let channels = usize::from(self.info.channels);
                    copy_interleaved(&buffer, channels, &mut self.pending);
                    self.pending_at = 0;
                    self.drop_skipped(channels);
                    if self.pending_at < self.pending.len() {
                        return Ok(true);
                    }
                }
                Err(SymphoniaError::DecodeError(reason)) => {
                    tracing::debug!(reason, "skipping a damaged MP3 frame");
                }
                Err(SymphoniaError::IoError(_)) => return Ok(false),
                Err(error) => return Err(Error::Decode(error.to_string())),
            }
        }
    }

    /// Drops the frames still to skip after a seek from the front of
    /// `pending`.
    fn drop_skipped(&mut self, channels: usize) {
        let frames = (self.pending.len() / channels) as u64;
        let dropped = self.skip.min(frames);
        self.skip -= dropped;
        self.pending_at = dropped as usize * channels;
    }
}

/// Copies `buffer` into `out` as interleaved `f32` with `channels` samples
/// per frame, mixing down or duplicating when a frame's layout differs from
/// the stream's.
fn copy_interleaved(buffer: &GenericAudioBufferRef<'_>, channels: usize, out: &mut Vec<f32>) {
    let source = buffer.spec().channels().count().max(1);
    if source == channels {
        buffer.copy_to_vec_interleaved(out);
        return;
    }
    let mut native = Vec::new();
    buffer.copy_to_vec_interleaved(&mut native);
    out.clear();
    for frame in native.chunks_exact(source) {
        let mono = frame.iter().sum::<f32>() / source as f32;
        out.extend(std::iter::repeat_n(mono, channels));
    }
}

/// `frames` at `rate` as a duration.
pub fn frames_to_duration(frames: u64, rate: u32) -> Duration {
    let rate = u64::from(rate.max(1));
    Duration::from_secs(frames / rate)
        + Duration::from_nanos((frames % rate) * 1_000_000_000 / rate)
}

/// `duration` as whole frames at `rate`.
pub fn duration_to_frames(duration: Duration, rate: u32) -> u64 {
    let nanos = duration.as_nanos() * u128::from(rate) / 1_000_000_000;
    u64::try_from(nanos).unwrap_or(u64::MAX)
}
