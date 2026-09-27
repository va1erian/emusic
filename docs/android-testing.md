# Android testing, screenshots and UI automation

How to run, look at and drive the emusic Android app from a headless machine.
The Android counterpart to [`emusic-shot`](win32-uia.md) and the Win32 UI
Automation helper: an agent can boot the app, take real screenshots and
manipulate it without a display.

## Prerequisites

- Android SDK with `platform-tools`, `emulator`, `cmdline-tools` and a system
  image (`system-images;android-36;google_apis;x86_64`). `ANDROID_HOME` must
  point at the SDK root.
- Hardware acceleration (WHPX on Windows, KVM on Linux). Check with
  `emulator -accel-check`.
- The debug APK. Build it once with:

  ```powershell
  cd android
  .\gradlew :app:assembleDebug
  ```

  The build cross-compiles the Rust core and generates the Kotlin bindings; see
  `crates/mobile` and `android/app/build.gradle.kts`.

## The driver: `scripts\android\android.ps1`

One script boots the emulator, installs/launches the app, captures screenshots
and drives the UI. Run `-Command` as the first positional argument.

```powershell
$s = "scripts\android\android.ps1"

& $s avd                        # install the system image and create the AVD (once)
& $s start                      # boot the AVD headless and wait for boot
& $s install                    # install the debug APK
& $s launch                     # start MainActivity
& $s shot -Out scratch\main.png -Width 480
& $s tap  -Text Add             # tap a node by its visible text ...
& $s tap  -Id pairing_code      # ... or by a resource-id substring
& $s type -Text "123456"        # type into the focused field
& $s key  -Key BACK             # send key events (HOME, BACK, ENTER, DEL, ...)
& $s dump                       # print every text node with its bounds
& $s status                     # device serial and current focus
& $s stop                       # shut the emulator down
```

`-Sdk <path>` overrides `ANDROID_HOME`. `tap` finds the node with
`uiautomator`, computes its centre from the bounds, and issues a real
`input tap` — the same path a finger takes, so focus and click handling are
exercised.

## Rules that matter

- **Pull screenshots as files, never pipe them.** `adb exec-out screencap -p >`
  through PowerShell corrupts the PNG (PowerShell rewrites the byte stream);
  the script uses `screencap` to `/sdcard` plus `adb pull`.
- **Downscale before looking.** A Pixel is 1080×2400; `-Width 480` keeps the
  image readable. Always look at the PNG before trusting a change.
- **One emulator at a time.** `start` refuses to boot a second one; `status`
  tells you whether one is running.
- **Headless by default.** `-WithWindow` shows the emulator, which steals focus
  on a shared desktop; leave it off for agent runs.

This mirrors the desktop workflow in [AGENTS.md](../AGENTS.md): inspect the
rendered result, do not guess.

## Instrumentation tests

Real device/emulator tests live in `android/app/src/androidTest` and use
AndroidX Test + UI Automator. Run them against a booted emulator:

```powershell
cd android
.\gradlew :app:connectedDebugAndroidTest
```

CI runs the same on a hardware-accelerated emulator; see
`.github/workflows/android.yml`.

## Limitations

- The app is server-only, so screenshots of the library/playback need a reachable
  `emusic-server` (the emulator host is `10.0.2.2`).
- UI Automator sees the accessibility tree; Compose nodes without text need a
  `Modifier.testTag`/`contentDescription` to be found reliably.
