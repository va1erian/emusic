#![forbid(unsafe_code)]

//! emusic's portable frontend, a UI built on the `xui_core` widget layer.
//!
//! This crate is the cross-platform renderer over the toolkit-agnostic
//! [`emusic_ui`] core. It is selected by the binary on non-Windows targets and
//! on Windows behind the `emusic/portable` feature; the native Windows renderer
//! is `emusic-frontend-win32`. Both drive the shared
//! [`Shell`](emusic_ui::shell::Shell) and expose the same [`run`] entry point.
//!
//! The shell layout and every view live here; OS shell services go through
//! [`emusic_platform`], so this crate names no backend type outside [`run`] and
//! [`window`].

pub mod app;
pub mod backend;
pub mod dialogs;
mod make;
pub mod menu;
pub mod theme;
pub mod views;
pub mod waker;
pub mod window;

mod run;

pub use run::run;
