#![forbid(unsafe_code)]

//! The emusic application (#106), a native Win32 UI built on `win32ui`.
//!
//! The binary (`src/main.rs`) is a thin `main`; the reusable pieces live here:
//! [`app::Win32App`] owns the shared [`Shell`](emusic_ui::shell::Shell) and
//! the window's controls, [`menu`] builds the menu bar, [`views`] holds the
//! view widgets, and [`waker`] wakes the UI from workers.

mod appearance;
mod d2d_text;

pub mod app;
pub mod dialogs;
pub mod menu;
pub mod theme;
pub mod views;
pub mod waker;
pub mod window;
