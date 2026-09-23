emusic icon set
===============
ico/   multi-resolution .ico (16,20,24,32,40,48,64,256). Sizes <=32 use the simplified
       "-small" drawings (bigger label, fewer gear teeth, no sleeves / no text) so they stay crisp.
png/   every icon at 16..256 px.
svg/   sources. "<name>.svg" = large drawing (>=48 px), "<name>-small.svg" = <=32 px drawing.
       All text is outlined to paths, no font dependency.

Rust: embed the app icon into the exe (build.rs, with the `winres` or `embed-resource` crate)
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/icons/ico/emusic-app.ico");
    res.compile().unwrap();
egui window icon: load png/256/emusic-app.png into egui::IconData and pass it to
ViewportBuilder::with_icon.

Installer (Inno Setup):  SetupIconFile=ico\emusic-setup.ico   UninstallDisplayIcon={app}\emusic-uninstall.ico
NSIS: !define MUI_ICON "ico\emusic-setup.ico"  !define MUI_UNICON "ico\emusic-uninstall.ico"
File associations: point HKCR\emusic.<ext>\DefaultIcon at the matching file-<ext>.ico
(file-audio.ico is the fallback for anything else BASS plays).
