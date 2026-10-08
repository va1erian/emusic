//! The thread behind one channel: it owns the decoder and the output stream,
//! keeps the stream's queue full, and answers the channel's commands.
//!
//! The clock is what played: the stream's played count plus the frame the
//! stream started at. Pausing closes the stream (the sound system has no pause
//! that keeps its queue) and keeps the frames it had not played yet, which
//! the next stream plays first, so a pause loses nothing. A seek drops them
//! and starts a fresh stream at the new frame.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use emusic_player::error::PlayerError;

use crate::channel::Shared;
use crate::decode::{Mp3Decoder, duration_to_frames};
use crate::output::{Output, OutputFactory};

/// Samples decoded at a time.
const DECODE_SAMPLES: usize = 4096;

/// How long the loop waits for a command between refills while playing.
const REFILL_INTERVAL: Duration = Duration::from_millis(10);

/// What the channel asks of its worker.
pub(crate) enum Command {
    Play { restart: bool },
    Pause,
    Stop,
    Seek(Duration),
    Volume(f32),
}

/// A command and where to send its outcome.
pub(crate) struct Request {
    pub(crate) command: Command,
    pub(crate) reply: Sender<Result<(), PlayerError>>,
}

/// The current stream and what it holds.
struct Stream {
    output: Box<dyn Output>,
    /// Frames written to it, padding included.
    written: u64,
    /// Frames it has played.
    played: u64,
    /// Samples written but not played yet (padding excluded): what a pause
    /// gives back.
    unplayed: VecDeque<i16>,
    /// Frames of the track written before the padding, once the track's end
    /// is queued.
    end: Option<u64>,
    /// Frames of silence still to write after the end, to fill its period.
    pad: u64,
    /// Whether this worker started the stream (it may also have started on
    /// its own when its queue filled).
    started: bool,
}

pub(crate) struct Worker {
    decoder: Mp3Decoder,
    factory: Arc<dyn OutputFactory>,
    shared: Arc<Shared>,
    rate: u32,
    channels: usize,
    stream: Option<Stream>,
    /// The track frame the current stream started at.
    base: u64,
    /// Samples to queue before decoding more.
    carry: VecDeque<i16>,
    /// The decoder has nothing more.
    drained: bool,
    gain: f32,
}

impl Worker {
    pub(crate) fn new(
        decoder: Mp3Decoder,
        factory: Arc<dyn OutputFactory>,
        shared: Arc<Shared>,
    ) -> Worker {
        let info = *decoder.info();
        Worker {
            decoder,
            factory,
            shared,
            rate: info.rate,
            channels: usize::from(info.channels),
            stream: None,
            base: 0,
            carry: VecDeque::new(),
            drained: false,
            gain: 1.0,
        }
    }

    /// Runs until the channel goes away.
    pub(crate) fn run(mut self, requests: Receiver<Request>) {
        loop {
            let wait = if self.stream.is_some() {
                REFILL_INTERVAL
            } else {
                Duration::from_secs(3600)
            };
            match requests.recv_timeout(wait) {
                Ok(request) => {
                    let outcome = self.handle(request.command);
                    let _ = request.reply.send(outcome);
                    continue;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            if let Err(error) = self.pump() {
                tracing::warn!(%error, "audio output failed; stopping the track");
                self.close_stream();
                self.shared.set_active(false);
            }
        }
    }

    fn handle(&mut self, command: Command) -> Result<(), PlayerError> {
        match command {
            Command::Play { restart } => {
                if restart || self.at_end() {
                    self.seek(Duration::ZERO)?;
                }
                self.shared.set_active(true);
                if self.stream.is_none() {
                    self.open_stream()?;
                }
                self.pump()
            }
            Command::Pause => {
                self.close_stream();
                Ok(())
            }
            Command::Stop => {
                self.close_stream();
                self.shared.set_active(false);
                Ok(())
            }
            Command::Seek(position) => self.seek(position),
            Command::Volume(gain) => {
                self.gain = gain;
                match &mut self.stream {
                    Some(stream) => stream.output.set_volume(gain).map_err(backend),
                    None => Ok(()),
                }
            }
        }
    }

    /// Whether the whole track has played.
    fn at_end(&self) -> bool {
        self.drained && self.carry.is_empty() && self.stream.is_none()
    }

    fn seek(&mut self, position: Duration) -> Result<(), PlayerError> {
        let playing = self.stream.is_some();
        self.stream = None;
        let landed = self.decoder.seek(position).map_err(backend)?;
        self.base = duration_to_frames(landed, self.rate);
        self.carry.clear();
        self.drained = false;
        self.shared.scope().clear();
        self.shared.set_position(self.base);
        if playing {
            self.open_stream()?;
        }
        Ok(())
    }

    fn open_stream(&mut self) -> Result<(), PlayerError> {
        let mut output = self
            .factory
            .open(self.rate, self.channels as u16)
            .map_err(backend)?;
        if output.rate() != self.rate || usize::from(output.channels()) != self.channels {
            return Err(PlayerError::Backend(format!(
                "the sound system cannot play {} Hz x{}",
                self.rate, self.channels
            )));
        }
        output.set_volume(self.gain).map_err(backend)?;
        self.stream = Some(Stream {
            output,
            written: 0,
            played: 0,
            unplayed: VecDeque::new(),
            end: None,
            pad: 0,
            started: false,
        });
        Ok(())
    }

    /// Closes the stream, keeping what it had not played for the next one.
    fn close_stream(&mut self) {
        let Some(mut stream) = self.stream.take() else {
            return;
        };
        if let Ok(played) = stream.output.played() {
            self.note_played(&mut stream, played);
        }
        let played = stream
            .end
            .map_or(stream.played, |end| stream.played.min(end));
        self.base += played;
        self.shared.set_position(self.base);
        for sample in stream.unplayed.into_iter().rev() {
            self.carry.push_front(sample);
        }
    }

    /// Advances the clock to the stream's `played` count.
    fn note_played(&self, stream: &mut Stream, played: u64) {
        if played <= stream.played {
            return;
        }
        let frames = played - stream.played;
        stream.played = played;
        let samples = (frames as usize * self.channels).min(stream.unplayed.len());
        stream.unplayed.drain(..samples);
        self.shared.scope().played(frames);
        let position = stream.end.map_or(played, |end| played.min(end));
        self.shared.set_position(self.base + position);
    }

    /// Keeps the stream's queue full and ends the track once its last frame
    /// has played.
    fn pump(&mut self) -> Result<(), PlayerError> {
        let Some(mut stream) = self.stream.take() else {
            return Ok(());
        };
        let outcome = self.refill(&mut stream);
        let finished = stream.end.is_some_and(|end| stream.played >= end);
        if finished {
            self.base += stream.end.unwrap_or(stream.played);
            self.shared.set_position(self.base);
            self.shared.set_active(false);
            self.shared.fire_end();
        } else {
            self.stream = Some(stream);
        }
        outcome
    }

    fn refill(&mut self, stream: &mut Stream) -> Result<(), PlayerError> {
        let played = stream.output.played().map_err(backend)?;
        self.note_played(stream, played);
        while stream.end.is_none() {
            if self.carry.is_empty() && !self.decode()? {
                // The sound system plays only whole periods, and one that
                // resamples holds the last frames back until more follow: pad
                // the last period with silence, and one more after it.
                let period = stream.output.period_frames().max(1);
                stream.end = Some(stream.written);
                stream.pad = (period - stream.written % period) % period + period;
                break;
            }
            let frames = stream
                .output
                .try_write(self.carry.make_contiguous())
                .map_err(backend)?;
            if frames == 0 {
                return Ok(());
            }
            let samples = frames * self.channels;
            stream.unplayed.extend(self.carry.drain(..samples));
            stream.written += frames as u64;
        }
        if stream.pad > 0 {
            let silence = vec![0i16; stream.pad as usize * self.channels];
            let frames = stream.output.try_write(&silence).map_err(backend)?;
            stream.written += frames as u64;
            stream.pad -= frames as u64;
            if stream.pad > 0 {
                return Ok(());
            }
        }
        // A track shorter than the queue never fills it: start it now.
        if !stream.started {
            stream.output.start().map_err(backend)?;
            stream.started = true;
        }
        Ok(())
    }

    /// Decodes the next block into `carry`; false at the end of the track.
    fn decode(&mut self) -> Result<bool, PlayerError> {
        if self.drained {
            return Ok(false);
        }
        let mut block = vec![0.0f32; DECODE_SAMPLES / self.channels * self.channels];
        let count = self.decoder.read(&mut block).map_err(backend)?;
        if count == 0 {
            self.drained = true;
            return Ok(false);
        }
        self.shared.scope().decoded(&block[..count], self.channels);
        self.carry.extend(block[..count].iter().map(|&s| to_i16(s)));
        Ok(true)
    }
}

/// `sample` (nominally `-1.0..=1.0`) as signed 16-bit, clipped.
fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
}

/// An output or decoder error as the player reports it.
fn backend(error: impl std::fmt::Display) -> PlayerError {
    PlayerError::Backend(error.to_string())
}
