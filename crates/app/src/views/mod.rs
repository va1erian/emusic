//! The app's views.
//!
//! #370 ports the **Music** view and its shared **track table** to the portable
//! `xui_core` widget layer as the reference; every other region is a documented
//! [`placeholder`](placeholder) owned by a later issue. Each view owns its own
//! widgets and exposes a small, stable interface (`set_bounds`, `set_visible`,
//! `sync`, and its own `Msg` hooks), so one can be ported without touching the
//! shell's wiring.

pub mod music;
pub mod navigator;
pub mod placeholder;
pub mod track_table;
