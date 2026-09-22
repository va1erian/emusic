//! Writing temporary files for tests.

use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Writes `bytes` to a uniquely named file in the OS temp directory,
/// returning its path. Files are left in place for later cleanup by the
/// OS; they're small and the names never collide across test runs.
pub fn write_file(base: &str, ext: &str, bytes: &[u8]) -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("{base}_{pid}_{n}.{ext}"));
    std::fs::write(&path, bytes).expect("write temp test file");
    path
}
