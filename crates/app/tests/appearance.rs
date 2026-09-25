//! The appearance settings (#309) must reach the renderer live: changing the
//! font size, density or zebra flag updates the metrics the views read on the
//! next frame, without a restart.
//!
//! Rendered headlessly via `egui_kittest`'s accesskit tree, like the other
//! non-snapshot UI tests: no GPU is needed because no frame is rendered.

use eframe::egui;
use egui_kittest::Harness;

use emusic::app::EguiApp;
use emusic::appearance;
use emusic::config::Config;
use emusic::mock::{MockLibrary, MockPlayer};
use emusic::state::{Appearance, Density, FontSize, Theme, View};

fn harness_with(appearance: Appearance) -> Harness<'static, EguiApp> {
    harness_for(Config {
        appearance,
        ..Config::default()
    })
}

fn harness_for(config: Config) -> Harness<'static, EguiApp> {
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1280.0, 800.0))
        .build_eframe(move |cc| {
            EguiApp::with_config(
                cc,
                Box::new(MockLibrary::new()),
                Box::new(MockPlayer::default()),
                config,
            )
        });
    harness.state_mut().set_view(View::Music);
    harness.run_steps(1);
    harness
}

/// Regression: `egui_kittest` applies its own (dark) builder theme right after
/// `EguiApp::build`, so the frontend must re-apply the config's theme on the
/// first rendered frame instead of trusting the build-time application
/// (#309).
#[test]
fn light_theme_reaches_the_style_on_the_first_frame() {
    let harness = harness_for(Config {
        theme: Theme::Light,
        ..Config::default()
    });

    assert_eq!(harness.ctx.theme(), egui::Theme::Light, "light is active");
    let visuals = &harness.ctx.global_style().visuals;
    let dark = egui::Visuals::dark();
    assert_ne!(
        visuals.panel_fill, dark.panel_fill,
        "panel fill must come from the light palette, not egui's dark default"
    );
    assert!(
        visuals.panel_fill.r() > 128
            && visuals.panel_fill.g() > 128
            && visuals.panel_fill.b() > 128,
        "light panel fill is bright, got {:?}",
        visuals.panel_fill
    );
    assert_ne!(visuals.window_fill, dark.window_fill);
}

#[test]
fn installed_metrics_match_the_smallest_comfortable_setting() {
    let harness = harness_with(Appearance {
        font_size: FontSize::Small,
        density: Density::Comfortable,
        zebra: true,
    });
    let metrics = appearance::metrics();
    assert!(metrics.row_height < 20.0, "small font shrinks rows");
    assert!(appearance::zebra());
    drop(harness);
}

#[test]
fn changing_density_updates_the_installed_metrics_live() {
    let mut harness = harness_with(Appearance::default());
    let before = appearance::metrics();

    harness.state_mut().shell().state.appearance.density = Density::Spacious;
    harness.run_steps(1);

    assert!(
        appearance::metrics().row_height > before.row_height,
        "spacious rows must be taller without a restart"
    );
}

#[test]
fn changing_font_size_updates_the_installed_metrics_live() {
    let mut harness = harness_with(Appearance::default());
    let before = appearance::metrics();

    harness.state_mut().shell().state.appearance.font_size = FontSize::Larger;
    harness.run_steps(1);

    let after = appearance::metrics();
    assert!(after.body > before.body, "text must grow");
    assert!(after.row_height > before.row_height, "rows must grow");
}

#[test]
fn turning_zebra_off_reaches_the_renderer() {
    let mut harness = harness_with(Appearance::default());

    harness.state_mut().shell().state.appearance.zebra = false;
    harness.run_steps(1);

    assert!(!appearance::zebra());
}
