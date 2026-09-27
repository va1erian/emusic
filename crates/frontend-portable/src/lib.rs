#![forbid(unsafe_code)]

//! emusic's portable frontend, a UI built on the `xui_core` widget layer.
//!
//! This crate is one of the two interchangeable frontends over the
//! toolkit-agnostic [`emusic_ui`] core: a non-Windows binary selects it, the
//! Windows binary selects `emusic-frontend-win32`. Both drive the shared
//! [`Shell`](emusic_ui::shell::Shell) and expose the same [`run`] entry point.
//!
//! The shell layout and the views live here; OS shell services go through
//! [`emusic_platform`], so this crate names no backend type outside [`run`] and
//! [`window`].

pub mod app;
pub mod dialogs;
pub mod menu;
pub mod theme;
pub mod views;
pub mod waker;
pub mod window;

mod run;

pub use run::run;
