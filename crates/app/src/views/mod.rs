//! The app's views.
//!
//! #370 ports the **Music** view and its shared **track table** to the portable
//! `xui_core` widget layer as the reference; #373 ports the **navigator** and
//! the **Folders** view onto the model `TreeView`. Every other region is a
//! documented [`placeholder`](placeholder) owned by a later issue. Each view
//! owns its own widgets and exposes a small, stable interface (`set_bounds`,
//! `set_visible`, `sync`, and its own `Msg` hooks), so one can be ported
//! without touching the shell's wiring.

pub mod folders;
pub mod music;
pub mod navigator;
pub mod placeholder;
pub mod status_bar;
pub mod top_bar;
pub mod track_table;
