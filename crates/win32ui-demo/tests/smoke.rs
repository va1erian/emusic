//! Headless smoke test: builds a real widget-layer window with a toolbar, a
//! tree and a virtual list through the external `win32ui` crate, then checks
//! the safe wrapper's bookkeeping. If the CI session cannot create windows at
//! all, the test skips rather than fails.

#![cfg(windows)]

use std::cell::Cell;
use std::rc::Rc;

use win32ui::prelude::*;

/// Milliseconds after which the watchdog gives up on the app.
const WATCHDOG_MS: u32 = 5000;

struct TestTree;

impl TreeModel for TestTree {
    type Key = i64;

    fn children(&self, parent: Option<&i64>) -> Vec<Node<i64>> {
        match parent {
            None => vec![Node::branch(1, "Music"), Node::leaf(2, "Playlists")],
            Some(&1) => vec![Node::leaf(11, "Rock"), Node::leaf(12, "Jazz")],
            _ => Vec::new(),
        }
    }
}

enum Msg {
    Start,
}

/// What the app observed, shared back to the test.
#[derive(Default)]
struct Observed {
    created: Cell<bool>,
    timed_out: Cell<bool>,
    node_count: Cell<Option<i32>>,
    selected: Cell<Option<Option<usize>>>,
    toolbar_height: Cell<Option<i32>>,
}

struct SmokeApp {
    tree: Option<TreeView<i64, Msg>>,
    list: Option<ListView<String, Msg>>,
    status: Option<StatusBar<Msg>>,
    toolbar: Option<Toolbar<Msg>>,
    observed: Rc<Observed>,
}

impl App for SmokeApp {
    type Msg = Msg;

    fn update(&mut self, _msg: Msg, ui: &mut Ui<Msg>) {
        let (Some(tree), Some(list), Some(status), Some(toolbar)) =
            (&self.tree, &self.list, &self.status, &self.toolbar)
        else {
            return;
        };

        self.observed.node_count.set(Some(tree.node_count()));
        list.set_selection(&[2]);
        self.observed.selected.set(Some(list.selected()));
        list.set_sort_indicator(1, SortDirection::Ascending);
        status.set_text(0, "Ready");
        self.observed.toolbar_height.set(Some(toolbar.height()));
        ui.quit();
    }
}

/// Runs the smoke app under a watchdog. `None` means the session cannot create
/// windows (skip rather than fail).
fn run_smoke() -> Option<Rc<Observed>> {
    win32ui::init();
    let observed = Rc::new(Observed::default());
    let observed_for_make = Rc::clone(&observed);

    let result = win32ui::run_app(
        WindowSpec::new("win32ui.smoke").theme(Theme::light()),
        move |ui| {
            let observed = Rc::clone(&observed_for_make);
            // A watchdog timer: quit the loop if the app never does.
            if let Ok(watchdog) = ui.set_timer(WATCHDOG_MS) {
                let observed = Rc::clone(&observed);
                ui.on_timer(move |fired| {
                    if fired == watchdog {
                        observed.timed_out.set(true);
                        win32ui::quit(1);
                    }
                    None
                });
            }

            let toolbar = Toolbar::new(ui, vec![ToolbarItem::new("One")]).ok();
            let tree = TreeView::new(ui, TestTree).ok();
            let list = ListView::new(ui)
                .ok()
                .map(|list| list.column("Title", Fill, |row: &String| row.as_str()));
            if let Some(list) = &list {
                list.set_model(vec!["zero".to_string(), "one".to_string()]);
            }
            let status = StatusBar::new(ui).ok();

            let created = toolbar.is_some() && tree.is_some() && list.is_some() && status.is_some();
            observed.created.set(created);
            if created {
                ui.emit(Msg::Start);
            } else {
                ui.quit();
            }

            SmokeApp {
                tree,
                list,
                status,
                toolbar,
                observed,
            }
        },
    );

    result.ok()?;
    Some(observed)
}

#[test]
fn window_with_controls_round_trips() {
    let Some(observed) = run_smoke() else {
        eprintln!("skipping: this session cannot create windows");
        return;
    };

    if !observed.created.get() {
        eprintln!("skipping: the controls could not be created");
        return;
    }
    assert!(
        !observed.timed_out.get(),
        "the watchdog fired before the app quit"
    );
    assert_eq!(
        observed.node_count.get(),
        Some(2),
        "the tree did not report its roots"
    );
    assert_eq!(
        observed.selected.get(),
        Some(Some(2)),
        "the list did not report row 2 as selected"
    );
    assert!(
        observed.toolbar_height.get().is_some_and(|h| h > 0),
        "the toolbar reported no height"
    );
}
