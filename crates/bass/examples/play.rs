//! Plays a single file to the end, then exits.
//!
//! ```text
//! cargo run -p bass --example play -- song.flac
//! cargo run -p bass --example play -- tune.xm
//! ```
//!
//! Requires `bass.dll` (and, for tracker formats, nothing extra — MOD/XM/S3M
//! etc are natively supported) in `EMUSIC_BASS_DIR` or `<exe dir>/bass/`.

use std::error::Error;
use std::path::Path;
use std::thread;
use std::time::Duration;

use bass::{Bass, Channel, MusicFlags, StreamFlags};

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: play <file>")
        .map_err(|e| -> Box<dyn Error> { e.into() })?;

    let bass = Bass::init(-1, 44100)?;
    println!("BASS version: 0x{:08x}", bass.version());

    play_file(&bass, Path::new(&path))
}

/// Tries to open `path` as a plain audio stream first (the common case),
/// falling back to a tracker module load if that fails — this lets the
/// example handle both `song.flac` and `tune.xm` without the caller having
/// to say which kind it is.
fn play_file(bass: &Bass, path: &Path) -> Result<(), Box<dyn Error>> {
    match bass.open_stream(path, StreamFlags::empty()) {
        Ok(stream) => {
            println!("Opened as a stream: {:?}", stream.info()?);
            stream.play(true)?;
            wait_until_done(&stream);
        }
        Err(stream_err) => {
            let music = bass
                .open_music(path, MusicFlags::PRESCAN, 0)
                .map_err(|music_err| {
                    format!("couldn't open as stream ({stream_err}) or music ({music_err})")
                })?;
            println!("Opened as music: {:?}", music.info()?);
            println!("Tags: {:?}", music.tags());
            music.play(true)?;
            wait_until_done(&music);
        }
    }
    Ok(())
}

fn wait_until_done(channel: &impl Channel) {
    while channel.is_active() {
        thread::sleep(Duration::from_millis(200));
    }
}
