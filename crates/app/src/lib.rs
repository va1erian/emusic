#![forbid(unsafe_code)]

//! The emusic application, a portable UI built on `xui_core`.
//!
//! The binary (`src/main.rs`) is a thin `main`: it handles the CLI, file
//! associations and the single-instance handshake, picks a backend, and runs
//! [`app::Win32App`] on the portable runtime. The shell layout and every view
//! live here; OS shell services go through [`emusic_platform`], so this crate
//! names no backend type outside `main` and `window`.

pub mod app;
pub mod theme;
pub mod views;
pub mod waker;
pub mod window;
