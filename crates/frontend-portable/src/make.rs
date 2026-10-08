//! Creating an xui widget outside a layout.
//!
//! xui creates widgets only through the `arrange` builders mounted in a
//! layout. emusic's views still place most of their widgets themselves, on
//! resize and on scroll (`app/layout.rs`, `views/settings/form.rs`), so
//! [`make`] realises one builder through a throwaway mount and hands the
//! widget back. The mount is dropped at once: nothing re-flows the widget
//! afterwards, and the view owns its bounds as before.

use std::rc::Rc;

use xui::xui_core::app::Ui;
use xui::xui_core::arrange::{Build, Handle, Layout, LayoutExt, ScrollBuild, TabsBuild, absolute};
use xui::xui_core::backend::Result;
use xui::xui_core::widget::{Placeable, ScrollView, Tabs};

/// A builder [`make`] can realise: it binds the widget to a handle and becomes
/// a layout entry.
pub trait Realize<W: 'static, M: 'static> {
    /// Places the bound builder in a one-entry layout.
    fn bound(self, handle: &Handle<W>) -> Layout<M>;
}

impl<W: Placeable<M> + 'static, M: 'static> Realize<W, M> for Build<W, M> {
    fn bound(self, handle: &Handle<W>) -> Layout<M> {
        absolute().child(self.bind(handle).at(0, 0, 0, 0))
    }
}

impl<M: 'static> Realize<Tabs<M>, M> for TabsBuild<M> {
    fn bound(self, handle: &Handle<Tabs<M>>) -> Layout<M> {
        absolute().child(self.bind(handle).at(0, 0, 0, 0))
    }
}

impl<M: 'static> Realize<ScrollView<M>, M> for ScrollBuild<M> {
    fn bound(self, handle: &Handle<ScrollView<M>>) -> Layout<M> {
        absolute().child(self.bind(handle).at(0, 0, 0, 0))
    }
}

/// Creates the widget `builder` describes as a child of `ui`'s parent.
pub fn make<W: 'static, M: 'static>(ui: &Ui<M>, builder: impl Realize<W, M>) -> Result<W> {
    let handle = Handle::new();
    let mounted = ui.mount(builder.bound(&handle))?;
    drop(mounted);
    let widget = handle
        .try_get()
        .expect("a mounted builder fills the handle bound to it");
    drop(handle);
    Ok(Rc::try_unwrap(widget)
        .ok()
        .expect("the dropped mount and handle held the only other references"))
}
