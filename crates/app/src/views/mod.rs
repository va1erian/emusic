//! The app's views.
//!
//! #370 ports the **Music** view and its shared **track table** to the portable
//! `xui_core` widget layer as the reference; #373 ports the **navigator** and
//! the **Folders** view onto the model `TreeView`; #372 ports the remaining
//! list views — **Artists**, **Genres**, **Starred**, **Most Played**,
//! **History** and the Music view's **column browser** — onto the model
//! `ListView`. **Settings** (#376) ports onto the portable pickers/flow text,
//! and the **Albums** grid (#374) onto the model `GridView`. **Now Playing**
//! (#371) ports its owner-drawn summary onto the portable `Canvas`, the
//! **Visualization** view (#371) is the portable preset browser, and the
//! **visualizer** (#371) is the painted status-bar strip. Every central view is
//! now ported: each owns its own widgets and exposes a small, stable interface
//! (`set_bounds`, `set_visible`, `sync`, and its own `Msg` hooks).

pub mod album_grid;
pub mod artists;
pub mod column_browser;
pub mod folders;
pub mod genres;
pub mod history;
pub mod most_played;
pub mod music;
pub mod name_counts;
pub mod navigator;
pub mod now_playing;
pub mod settings;
pub mod starred;
pub mod status_bar;
pub mod top_bar;
pub mod track_table;
pub mod visualization;
pub mod visualizer_strip;
