# macOS `.app` bundle

Assembles a `emusic.app` around the release binary. Nothing here is committed
as a built bundle; the BASS dylibs are never committed either (they are staged
at assembly time under the existing licence arrangement).

## Layout

```
emusic.app/
  Contents/
    Info.plist                       # this folder's Info.plist
    MacOS/
      emusic                         # target/release/emusic
      bass/
        libbass.dylib                # from EMUSIC_BASS_DIR, never committed
        libbassflac.dylib
        ...
    Resources/
      emusic.icns                    # assets/icns/emusic-app.icns
```

The loader finds BASS in a `bass/` folder next to the executable
(`crates/bass/src/ffi/loader.rs` accepts the macOS names `libbass*.dylib`), or
in `$EMUSIC_BASS_DIR`.

## Assemble

```sh
cargo build --release -p emusic

APP=emusic.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS/bass" "$APP/Contents/Resources"

cp installer/macos/Info.plist "$APP/Contents/"
cp target/release/emusic "$APP/Contents/MacOS/"
cp assets/icns/emusic-app.icns "$APP/Contents/Resources/emusic.icns"

# BASS dylibs (not in git); copy the core library and the add-ons you ship.
cp "$EMUSIC_BASS_DIR"/libbass*.dylib "$APP/Contents/MacOS/bass/"

# Ad-hoc sign so Gatekeeper will run a locally built bundle.
codesign --force --deep --sign - "$APP"

open "$APP"
```

The portable frontend always runs on `xui`'s software `canvas` backend, on
every target, so there is nothing to select here.

## Icon

`assets/icns/emusic-app.icns` is the same artwork as the Windows
`assets/ico/emusic-app.ico`, committed as a multi-size (16/32/64/128/256 px)
PNG-based `.icns` and referenced by `CFBundleIconFile`. To regenerate it from
the Windows icon, run `python scripts/make-icns.py` (needs Pillow).

## Follow-ups

- `CFBundleDocumentTypes` declares an alternate handler; opening files from
  Finder also needs the single-instance/IPC follow-up (P3.2) so a second
  launch forwards paths to the running instance (there is no `open-file` event
  wiring yet).
