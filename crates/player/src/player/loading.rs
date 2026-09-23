//! Off-UI-thread track opening and end-of-track/failure handling.
//!
//! Every [`crate::AudioBackend::open`] call is dispatched to a short-lived
//! worker thread here — see the [`super::Player`] type docs for why (network
//! shares must not block the UI thread) — and [`Player::process_pending_open`]
//! (driven by [`Player::tick`]) picks up the result without blocking.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{TryRecvError, bounded};

use crate::error::PlayerError;
use crate::events::{PlaybackState, PlayerEvent};
use crate::listen::ListenAccounting;
use crate::queue::RepeatMode;
use crate::volume::perceptual_to_gain;

use super::{CurrentTrack, OpenMessage, Player};

impl Player {
    /// Drops the current track (if any), stopping it and emitting the
    /// [`PlayerEvent::PlayFinished`] accounting summary for it.
    pub(super) fn drop_current_and_account(&mut self) {
        if let Some(current) = self.current.take() {
            let _ = current.channel.stop();
            let completed = self.listen.completed(current.duration);
            self.emit(PlayerEvent::PlayFinished {
                path: current.path,
                listened: self.listen.listened(),
                completed,
            });
        }
        self.listen = ListenAccounting::new();
        self.last_tick = None;
        // A resume applies only to the open it was set for; dropping the
        // current track cancels any still-pending one.
        self.pending_resume = None;
    }

    /// Spawns a worker thread to open `path` off the calling (UI) thread —
    /// see the module docs for why. [`Player::tick`] picks up the result via
    /// [`Player::process_pending_open`].
    pub(super) fn start_open(&mut self, path: PathBuf, queue_index: usize) {
        let (tx, rx) = bounded(1);
        let backend = Arc::clone(&self.backend);
        let end_tx = self.end_tx.clone();
        let spawned = thread::Builder::new()
            .name("emusic-player-open".to_string())
            .spawn(move || {
                let failed_path = path.clone();
                let outcome = backend.open(&path).and_then(|channel| {
                    let guard = channel.on_end(Box::new(move || {
                        let _ = end_tx.send(());
                    }))?;
                    Ok((path, channel, guard))
                });
                let message = match outcome {
                    Ok((path, channel, guard)) => OpenMessage::Ready {
                        path,
                        queue_index,
                        channel,
                        guard,
                    },
                    Err(error) => OpenMessage::Failed {
                        path: failed_path,
                        error,
                    },
                };
                let _ = tx.send(message);
            });
        match spawned {
            Ok(_join_handle) => self.pending_open = Some(rx),
            Err(io_error) => {
                self.emit(PlayerEvent::Error(PlayerError::SpawnFailed(
                    io_error.to_string(),
                )));
            }
        }
    }

    pub(super) fn process_pending_open(&mut self) {
        let Some(rx) = &self.pending_open else {
            return;
        };
        match rx.try_recv() {
            Ok(OpenMessage::Ready {
                path,
                queue_index,
                channel,
                guard,
            }) => {
                self.pending_open = None;
                let resume = self.pending_resume.take();
                let (position, play) =
                    resume.map_or((Duration::ZERO, true), |r| (r.position, r.play));
                if let Err(error) = channel.set_volume(perceptual_to_gain(self.volume)) {
                    self.emit(PlayerEvent::Error(error));
                }
                if position > Duration::ZERO
                    && let Err(error) = channel.seek(position)
                {
                    self.emit(PlayerEvent::Error(error));
                }
                if let Err(error) = channel.play(false) {
                    self.emit(PlayerEvent::Error(error));
                    return;
                }
                // A restored track may have been paused when the app closed;
                // load it (so its position/duration read back) but hold it
                // there instead of playing.
                if !play && let Err(error) = channel.pause() {
                    self.emit(PlayerEvent::Error(error));
                }
                let duration = channel.duration().ok();
                self.current = Some(CurrentTrack {
                    channel,
                    _end_guard: guard,
                    path: path.clone(),
                    duration,
                });
                self.last_tick = play.then(Instant::now);
                self.set_state(if play {
                    PlaybackState::Playing
                } else {
                    PlaybackState::Paused
                });
                self.emit(PlayerEvent::TrackStarted { path, queue_index });
            }
            Ok(OpenMessage::Failed { path, error }) => {
                self.pending_open = None;
                self.pending_resume = None;
                if self.queue.is_shuffle() {
                    // A scoped shuffle must survive an unreadable file (an
                    // offline NAS, a deleted track): skip it and try the next
                    // one instead of stopping. `advance_skip` never
                    // reshuffles, so an entirely offline scope still ends.
                    self.emit(PlayerEvent::TrackSkipped { path });
                    match self.queue.advance_skip() {
                        Some(next) => {
                            self.emit(PlayerEvent::QueueChanged);
                            self.open_current_or_stop(Some(next));
                        }
                        None => self.set_state(PlaybackState::Stopped),
                    }
                } else {
                    self.emit(PlayerEvent::Error(error));
                    self.set_state(PlaybackState::Stopped);
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.pending_open = None;
                self.pending_resume = None;
            }
        }
    }

    pub(super) fn process_end_signals(&mut self) {
        let mut ended = false;
        while self.end_rx.try_recv().is_ok() {
            ended = true;
        }
        if ended {
            self.handle_track_ended();
        }
    }

    fn handle_track_ended(&mut self) {
        let Some(path) = self.current.as_ref().map(|c| c.path.clone()) else {
            return;
        };
        self.accrue_listened_time();
        self.emit(PlayerEvent::TrackEnded { path: path.clone() });

        if self.queue.repeat_mode() == RepeatMode::One {
            let queue_index = self.queue.current_item_index().unwrap_or(0);
            self.drop_current_and_account();
            self.start_open(path, queue_index);
            return;
        }

        self.drop_current_and_account();
        let next = self.queue.advance();
        self.open_current_or_stop(next);
    }
}
