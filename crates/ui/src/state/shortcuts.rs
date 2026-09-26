//! The central keyboard-shortcut table (#28).
//!
//! Toolkit-agnostic data only: the actions a shortcut can trigger, the keys
//! they bind to and the exact text the Help window lists. The Win32 frontend
//! translates [`Shortcut`]s into its own accelerator type and maps each
//! [`ShortcutAction`] onto a [`Command`](crate::state::Command); nothing here
//! depends on a windowing toolkit.

use std::time::Duration;

use super::projectm::PresetRequest;
use super::{Command, VizCommand};

/// How far [`ShortcutAction::SeekBackward`]/[`ShortcutAction::SeekForward`]
/// move the playhead per press, in seconds.
pub const SEEK_STEP_SECS: f32 = 5.0;

/// How much [`ShortcutAction::VolumeUp`]/[`ShortcutAction::VolumeDown`] change
/// the volume per press.
pub const VOLUME_STEP: f32 = 0.05;

/// A command a keyboard shortcut can trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutAction {
    /// Play or pause the current track.
    PlayPause,
    /// Skip to the next queue entry.
    NextTrack,
    /// Go back to the previous queue entry.
    PreviousTrack,
    /// Seek forward five seconds.
    SeekForward,
    /// Seek backward five seconds.
    SeekBackward,
    /// Raise the volume by one step.
    VolumeUp,
    /// Lower the volume by one step.
    VolumeDown,
    /// Focus the top bar's search box.
    Search,
    /// Rescan every enabled library folder.
    Rescan,
    /// Switch the visualization to the next projectM preset.
    PresetNext,
    /// Switch the visualization to the previous projectM preset.
    PresetPrevious,
    /// Switch the visualization to a random projectM preset.
    PresetRandom,
    /// Lock or unlock the visualization's current projectM preset.
    PresetLock,
}

/// A physical key a shortcut binds to, in the toolkit-agnostic vocabulary.
///
/// Kept deliberately small: only the keys the bindings use, so the frontend's
/// key mapping is exhaustive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutKey {
    /// The space bar.
    Space,
    /// The left arrow key.
    Left,
    /// The right arrow key.
    Right,
    /// The up arrow key.
    Up,
    /// The down arrow key.
    Down,
    /// The `F5` function key.
    F5,
    /// The `F` letter key.
    F,
    /// The `R` letter key.
    R,
    /// The `L` letter key.
    L,
}

impl ShortcutKey {
    /// The key's name as [`Shortcut::display`] renders it.
    pub const fn name(self) -> &'static str {
        match self {
            ShortcutKey::Space => "Space",
            ShortcutKey::Left => "Left",
            ShortcutKey::Right => "Right",
            ShortcutKey::Up => "Up",
            ShortcutKey::Down => "Down",
            ShortcutKey::F5 => "F5",
            ShortcutKey::F => "F",
            ShortcutKey::R => "R",
            ShortcutKey::L => "L",
        }
    }
}

/// One keyboard binding: an action, the key and modifiers that trigger it, and
/// the text the Help window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    /// What the binding does.
    pub action: ShortcutAction,
    /// The key that triggers it.
    pub key: ShortcutKey,
    /// Whether Ctrl is held with the key.
    pub ctrl: bool,
    /// Whether Shift is held with the key.
    pub shift: bool,
    /// Whether Alt is held with the key.
    pub alt: bool,
    /// A one-line human description of the action.
    pub description: &'static str,
}

impl Shortcut {
    /// The binding's display text, e.g. `"Ctrl+Right"` or `"Space"`.
    ///
    /// Modifiers render in a fixed order (Ctrl, Shift, Alt) followed by the
    /// key name.
    pub fn display(&self) -> String {
        let mut text = String::new();
        if self.ctrl {
            text.push_str("Ctrl+");
        }
        if self.shift {
            text.push_str("Shift+");
        }
        if self.alt {
            text.push_str("Alt+");
        }
        text.push_str(self.key.name());
        text
    }
}

/// The [`Command`] `action` dispatches given the player's current context.
///
/// This is the one place the shortcut→command mapping lives, so the frontend
/// only has to decode the key and hand over the player snapshot. Seek targets
/// clamp to `[0, duration]` when a duration is known (and to `>= 0` otherwise);
/// volume clamps to `[0, 1]`.
///
/// [`ShortcutAction::Search`] returns `None`: focusing the top-bar search box
/// is frontend state, not a shell command. Every other action (transport,
/// library and projectM preset) maps onto a [`Command`].
pub fn shortcut_command(
    action: ShortcutAction,
    position: Duration,
    duration: Option<Duration>,
    volume: f32,
) -> Option<Command> {
    match action {
        ShortcutAction::PlayPause => Some(Command::PlayerPlayPause),
        ShortcutAction::NextTrack => Some(Command::PlayerNext),
        ShortcutAction::PreviousTrack => Some(Command::PlayerPrevious),
        ShortcutAction::SeekForward => Some(Command::PlayerSeek(seek_target(
            position,
            duration,
            SEEK_STEP_SECS,
        ))),
        ShortcutAction::SeekBackward => Some(Command::PlayerSeek(seek_target(
            position,
            duration,
            -SEEK_STEP_SECS,
        ))),
        ShortcutAction::VolumeUp => Some(Command::PlayerSetVolume(
            (volume + VOLUME_STEP).clamp(0.0, 1.0),
        )),
        ShortcutAction::VolumeDown => Some(Command::PlayerSetVolume(
            (volume - VOLUME_STEP).clamp(0.0, 1.0),
        )),
        ShortcutAction::Rescan => Some(Command::LibraryRescan),
        ShortcutAction::PresetNext => Some(Command::Viz(VizCommand::Preset(PresetRequest::Next))),
        ShortcutAction::PresetPrevious => {
            Some(Command::Viz(VizCommand::Preset(PresetRequest::Previous)))
        }
        ShortcutAction::PresetRandom => {
            Some(Command::Viz(VizCommand::Preset(PresetRequest::Random)))
        }
        ShortcutAction::PresetLock => Some(Command::Viz(VizCommand::TogglePresetLock)),
        ShortcutAction::Search => None,
    }
}

/// `position + delta_secs`, clamped to `[0, duration]` when `duration` is
/// known and to `>= 0` otherwise.
fn seek_target(position: Duration, duration: Option<Duration>, delta_secs: f32) -> Duration {
    let target = (position.as_secs_f32() + delta_secs).max(0.0);
    let target = match duration {
        Some(duration) => target.min(duration.as_secs_f32()),
        None => target,
    };
    Duration::from_secs_f32(target)
}

/// Every keyboard binding, in the order the Help window lists them.
///
/// This is the one source of truth: the app registers accelerators from it and
/// the Help window renders it.
pub const SHORTCUTS: &[Shortcut] = &[
    Shortcut {
        action: ShortcutAction::PlayPause,
        key: ShortcutKey::Space,
        ctrl: false,
        shift: false,
        alt: false,
        description: "Play or pause",
    },
    Shortcut {
        action: ShortcutAction::NextTrack,
        key: ShortcutKey::Right,
        ctrl: true,
        shift: false,
        alt: false,
        description: "Next track",
    },
    Shortcut {
        action: ShortcutAction::PreviousTrack,
        key: ShortcutKey::Left,
        ctrl: true,
        shift: false,
        alt: false,
        description: "Previous track",
    },
    Shortcut {
        action: ShortcutAction::SeekForward,
        key: ShortcutKey::Right,
        ctrl: false,
        shift: false,
        alt: false,
        description: "Seek forward 5 seconds",
    },
    Shortcut {
        action: ShortcutAction::SeekBackward,
        key: ShortcutKey::Left,
        ctrl: false,
        shift: false,
        alt: false,
        description: "Seek backward 5 seconds",
    },
    Shortcut {
        action: ShortcutAction::VolumeUp,
        key: ShortcutKey::Up,
        ctrl: true,
        shift: false,
        alt: false,
        description: "Increase volume",
    },
    Shortcut {
        action: ShortcutAction::VolumeDown,
        key: ShortcutKey::Down,
        ctrl: true,
        shift: false,
        alt: false,
        description: "Decrease volume",
    },
    Shortcut {
        action: ShortcutAction::Search,
        key: ShortcutKey::F,
        ctrl: true,
        shift: false,
        alt: false,
        description: "Focus the search box",
    },
    Shortcut {
        action: ShortcutAction::Rescan,
        key: ShortcutKey::F5,
        ctrl: false,
        shift: false,
        alt: false,
        description: "Rescan the library",
    },
    Shortcut {
        action: ShortcutAction::PresetNext,
        key: ShortcutKey::Right,
        ctrl: true,
        shift: false,
        alt: true,
        description: "Next preset",
    },
    Shortcut {
        action: ShortcutAction::PresetPrevious,
        key: ShortcutKey::Left,
        ctrl: true,
        shift: false,
        alt: true,
        description: "Previous preset",
    },
    Shortcut {
        action: ShortcutAction::PresetRandom,
        key: ShortcutKey::R,
        ctrl: true,
        shift: false,
        alt: true,
        description: "Random preset",
    },
    Shortcut {
        action: ShortcutAction::PresetLock,
        key: ShortcutKey::L,
        ctrl: true,
        shift: false,
        alt: true,
        description: "Lock the current preset",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_exactly_one_binding() {
        for action in [
            ShortcutAction::PlayPause,
            ShortcutAction::NextTrack,
            ShortcutAction::PreviousTrack,
            ShortcutAction::SeekForward,
            ShortcutAction::SeekBackward,
            ShortcutAction::VolumeUp,
            ShortcutAction::VolumeDown,
            ShortcutAction::Search,
            ShortcutAction::Rescan,
            ShortcutAction::PresetNext,
            ShortcutAction::PresetPrevious,
            ShortcutAction::PresetRandom,
            ShortcutAction::PresetLock,
        ] {
            let count = SHORTCUTS
                .iter()
                .filter(|shortcut| shortcut.action == action)
                .count();
            assert_eq!(count, 1, "{action:?} should have exactly one binding");
        }
    }

    #[test]
    fn transport_actions_map_to_their_commands() {
        let none = |action| shortcut_command(action, Duration::ZERO, None, 0.5);
        assert_eq!(
            none(ShortcutAction::PlayPause),
            Some(Command::PlayerPlayPause)
        );
        assert_eq!(none(ShortcutAction::NextTrack), Some(Command::PlayerNext));
        assert_eq!(
            none(ShortcutAction::PreviousTrack),
            Some(Command::PlayerPrevious)
        );
        assert_eq!(none(ShortcutAction::Rescan), Some(Command::LibraryRescan));
        assert_eq!(
            none(ShortcutAction::Search),
            None,
            "Search focuses the frontend's search box, not a command"
        );
    }

    #[test]
    fn preset_actions_map_to_their_commands() {
        let none = |action| shortcut_command(action, Duration::ZERO, None, 0.5);
        assert_eq!(
            none(ShortcutAction::PresetNext),
            Some(Command::Viz(VizCommand::Preset(PresetRequest::Next)))
        );
        assert_eq!(
            none(ShortcutAction::PresetPrevious),
            Some(Command::Viz(VizCommand::Preset(PresetRequest::Previous)))
        );
        assert_eq!(
            none(ShortcutAction::PresetRandom),
            Some(Command::Viz(VizCommand::Preset(PresetRequest::Random)))
        );
        assert_eq!(
            none(ShortcutAction::PresetLock),
            Some(Command::Viz(VizCommand::TogglePresetLock))
        );
    }

    #[test]
    fn seek_clamps_to_the_duration_and_to_zero() {
        let duration = Some(Duration::from_secs(100));
        let seek = |position, duration| {
            shortcut_command(ShortcutAction::SeekForward, position, duration, 0.5)
        };
        assert_eq!(
            seek(Duration::from_secs(30), duration),
            Some(Command::PlayerSeek(Duration::from_secs(35)))
        );
        assert_eq!(
            seek(Duration::from_secs(98), duration),
            Some(Command::PlayerSeek(Duration::from_secs(100))),
            "forward seek clamps to the duration"
        );
        let back = shortcut_command(
            ShortcutAction::SeekBackward,
            Duration::from_secs(2),
            duration,
            0.5,
        );
        assert_eq!(back, Some(Command::PlayerSeek(Duration::ZERO)));
        let back_unknown = shortcut_command(
            ShortcutAction::SeekBackward,
            Duration::from_secs(2),
            None,
            0.5,
        );
        assert_eq!(
            back_unknown,
            Some(Command::PlayerSeek(Duration::ZERO)),
            "no duration still clamps to zero"
        );
    }

    #[test]
    fn volume_clamps_to_the_unit_interval() {
        let up = |volume| shortcut_command(ShortcutAction::VolumeUp, Duration::ZERO, None, volume);
        let down =
            |volume| shortcut_command(ShortcutAction::VolumeDown, Duration::ZERO, None, volume);
        assert_eq!(up(0.5), Some(Command::PlayerSetVolume(0.55)));
        assert_eq!(up(0.99), Some(Command::PlayerSetVolume(1.0)));
        assert_eq!(down(0.5), Some(Command::PlayerSetVolume(0.45)));
        assert_eq!(down(0.01), Some(Command::PlayerSetVolume(0.0)));
    }

    #[test]
    fn display_formats_modifiers_then_key() {
        let find = |action: ShortcutAction| {
            SHORTCUTS
                .iter()
                .find(|shortcut| shortcut.action == action)
                .expect("binding exists")
        };
        assert_eq!(find(ShortcutAction::PlayPause).display(), "Space");
        assert_eq!(find(ShortcutAction::NextTrack).display(), "Ctrl+Right");
        assert_eq!(find(ShortcutAction::VolumeUp).display(), "Ctrl+Up");
        assert_eq!(find(ShortcutAction::Search).display(), "Ctrl+F");
        assert_eq!(find(ShortcutAction::Rescan).display(), "F5");
        assert_eq!(find(ShortcutAction::PresetNext).display(), "Ctrl+Alt+Right");
        assert_eq!(
            find(ShortcutAction::PresetPrevious).display(),
            "Ctrl+Alt+Left"
        );
        assert_eq!(find(ShortcutAction::PresetRandom).display(), "Ctrl+Alt+R");
        assert_eq!(find(ShortcutAction::PresetLock).display(), "Ctrl+Alt+L");
    }
}
