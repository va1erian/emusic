#![forbid(unsafe_code)]

//! Online metadata lookup for auto-tagging.
//!
//! [`Provider`] abstracts a music-metadata database; [`MusicBrainzProvider`] is
//! the first implementation. A caller builds a [`TrackQuery`] from a track's
//! local tags and/or filename and gets back ranked [`Candidate`]s whose fields
//! map onto the editable tag set (title, artist, album, album artist, year,
//! track/disc number).
//!
//! Everything in this crate is blocking and must run off the UI thread — the
//! `emusic-ui` backend drives it from a dedicated worker (see issue #208).

mod candidate;
mod error;
mod provider;
mod query;
mod score;

pub mod musicbrainz;

pub use candidate::Candidate;
pub use error::{MetadataError, Result};
pub use musicbrainz::MusicBrainzProvider;
pub use provider::Provider;
pub use query::TrackQuery;
pub use score::score;
