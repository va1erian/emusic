//! Sending a request from a secondary launch to the primary instance.

use std::io::Write;
use std::thread;
use std::time::Duration;

use interprocess::os::windows::named_pipe::{DuplexPipeStream, pipe_mode};

use super::message::IpcMessage;
use super::pipe_path;
use crate::error::{Result, WinshellError};
use crate::sys;

type Stream = DuplexPipeStream<pipe_mode::Bytes>;

/// How many times to retry connecting when the primary hasn't finished
/// starting up yet (its pipe doesn't exist at all, as opposed to merely
/// being busy, which `interprocess` already retries internally).
const CONNECT_RETRIES: u32 = 10;
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(30);

/// Connects to the primary instance's pipe for `app_id`, hands it the right
/// to raise its own window to the foreground, then sends `message` and
/// disconnects.
///
/// Intended for a secondary process launch (e.g. a file opened while
/// emusic is already running): call this, then exit quickly.
pub fn send_to_primary(app_id: &str, message: &IpcMessage) -> Result<()> {
    let pipe_name = pipe_path(app_id);
    let mut stream = connect_with_retries(&pipe_name)?;

    // The secondary process is briefly in the foreground (it was just
    // launched by the user/Explorer); delegate that right to the primary
    // so its `SetForegroundWindow` call succeeds once it handles `message`.
    if let Ok(pid) = sys::named_pipe_server_process_id(&stream) {
        let _ = sys::allow_set_foreground_window(pid);
    }

    let mut payload = serde_json::to_vec(message)?;
    payload.push(b'\n');
    stream.write_all(&payload)?;
    stream.flush()?;
    Ok(())
}

fn connect_with_retries(pipe_name: &str) -> Result<Stream> {
    let mut last_err = None;
    for attempt in 0..CONNECT_RETRIES {
        match Stream::connect_by_path(pipe_name) {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                last_err = Some(e);
                if attempt + 1 < CONNECT_RETRIES {
                    thread::sleep(CONNECT_RETRY_DELAY);
                }
            }
        }
    }
    Err(WinshellError::PipeUnavailable(
        last_err.map(|e| e.to_string()).unwrap_or_default(),
    ))
}
