#![forbid(unsafe_code)]

//! emusic's audio backend for hosts whose sound system takes a stream of
//! samples (LazyOS's `audiod`), with no BASS: MP3 decoding with symphonia and
//! a player channel over an [`Output`](output::Output) the host provides.
//!
//! [`decode`] and [`output`] know nothing of emusic: they are the part a
//! shared media library would take over. [`LazyBackend`] and its channel are
//! the adapter to `emusic-player`.

mod backend;
mod channel;
pub mod decode;
pub mod output;
mod scope;
mod worker;

pub use backend::LazyBackend;
pub use channel::LazyChannel;
