#![forbid(unsafe_code)]

//! The native Win32 frontend for emusic (#106), built on `win32ui`.
//!
//! The binary (`src/main.rs`) is a thin `main`; the reusable pieces live here:
//! [`app::Win32App`] owns the shared [`Shell`](emusic_ui::shell::Shell) and
//! the window's controls, [`menu`] builds the menu bar, [`views`] holds the
//! (placeholder) view widgets, and [`waker`] wakes the UI from workers.

pub mod app;
pub mod menu;
pub mod views;
pub mod waker;
pub mod window;
