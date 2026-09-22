#![forbid(unsafe_code)]

//! Headless screenshot tool (#32): renders the app (always with mock data)
//! off-screen via `egui_kittest`'s wgpu snapshot rendering (falls back to a
//! software adapter such as WARP where no GPU is available) and writes a
//! PNG. Kept as a separate binary, gated behind the `shot` feature, so
//! `egui_kittest`/wgpu never end up in the real `emusic.exe` dependency
//! tree.
//!
//! ```text
//! cargo run -p emusic --features shot --bin emusic-shot -- \
//!     --view music --size 1280x800 --theme dark --out target/shots/music.png
//! cargo run -p emusic --features shot --bin emusic-shot -- --all
//! ```

use std::path::{Path, PathBuf};

use clap::Parser;
use eframe::egui;
use egui_kittest::Harness;

use emusic::app::App;
use emusic::config::Config;
use emusic::mock::{MockLibrary, MockPlayer};
use emusic::state::View;

#[derive(Parser, Debug)]
#[command(name = "emusic-shot")]
struct Cli {
    /// View to render, e.g. `music`, `albums`, `now-playing`. Ignored if
    /// `--all` is set.
    #[arg(long)]
    view: Option<String>,

    /// Render every view into `--out`'s directory, one PNG per view.
    #[arg(long)]
    all: bool,

    /// `<width>x<height>`, e.g. `1280x800`.
    #[arg(long, default_value = "1280x800")]
    size: String,

    #[arg(long, value_enum, default_value = "dark")]
    theme: ThemeArg,

    /// Output PNG path (single view) or directory (`--all`).
    #[arg(long, default_value = "target/shots/shot.png")]
    out: PathBuf,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ThemeArg {
    Dark,
    Light,
}

fn main() {
    let cli = Cli::parse();
    let (width, height) = parse_size(&cli.size);

    if cli.all {
        let dir = if cli.out.extension().is_some() {
            cli.out
                .parent()
                .unwrap_or(Path::new("target/shots"))
                .to_path_buf()
        } else {
            cli.out.clone()
        };
        std::fs::create_dir_all(&dir).expect("create output directory");
        for view in View::ALL {
            let out = dir.join(format!("{}.png", view.slug()));
            render_one(view, width, height, cli.theme, &out);
        }
        return;
    }

    let view = cli
        .view
        .as_deref()
        .and_then(View::from_slug)
        .unwrap_or(View::Music);
    if let Some(parent) = cli.out.parent() {
        std::fs::create_dir_all(parent).expect("create output directory");
    }
    render_one(view, width, height, cli.theme, &cli.out);
}

fn render_one(view: View, width: f32, height: f32, theme: ThemeArg, out: &Path) {
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(width, height))
        .build_eframe(|cc| {
            let library = Box::new(MockLibrary::new());
            let player = Box::new(MockPlayer::playing_demo());
            App::with_config(cc, library, player, Config::default())
        });

    if matches!(theme, ThemeArg::Light) {
        harness.ctx.set_theme(egui::Theme::Light);
    }
    harness.state_mut().set_view(view);
    // A single step is enough for a static screenshot; `Harness::run` would
    // wait for the UI to go idle, which it never does here because the
    // shell's repaint policy (#6) keeps requesting frames while "playing".
    harness.run_steps(1);

    let image = harness.render().expect("headless render failed");
    image.save(out).expect("write screenshot PNG");
    eprintln!("wrote {}", out.display());
}

fn parse_size(spec: &str) -> (f32, f32) {
    let (w, h) = spec.split_once('x').unwrap_or(("1280", "800"));
    (w.parse().unwrap_or(1280.0), h.parse().unwrap_or(800.0))
}
