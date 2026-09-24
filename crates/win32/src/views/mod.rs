//! The Win32 frontend's views (#106).
//!
//! Each region of the main window is a view: a struct owning the controls it
//! draws into, a `sync` that updates them from the shell's state, and — for
//! the central area — a message type mapped from control notifications. The
//! first slice puts placeholders where the real views will go.

pub mod music;
pub mod navigator;
pub mod placeholder;
pub mod status_bar;
