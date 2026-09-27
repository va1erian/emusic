#![cfg(windows)]
#![forbid(unsafe_code)]

//! emusic's native Win32 frontend (#106), a UI built on `win32ui`.
//!
//! This crate is one of the two interchangeable frontends over the
//! toolkit-agnostic [`emusic_ui`] core: the Windows binary selects it, a
//! non-Windows binary selects `emusic-frontend-portable`. Both drive the shared
//! [`Shell`](emusic_ui::shell::Shell) and expose the same [`run`] entry point.
//!
//! [`app::Win32App`] owns the window's controls, [`menu`] builds the menu bar,
//! [`views`] holds the view widgets, and [`waker`] wakes the UI from workers.
//! On non-Windows targets the whole crate compiles to an empty library.

mod appearance;
mod d2d_text;

pub mod app;
pub mod backend;
pub mod dialogs;
pub mod icon;
pub mod menu;
pub mod theme;
pub mod views;
pub mod waker;
pub mod window;

mod run;

pub use run::run;
