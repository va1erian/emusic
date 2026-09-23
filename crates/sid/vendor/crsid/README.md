# Vendored cRSID engine

This directory contains the C sources of **cRSID** by Hermit (Mihály Horváth),
used by the `emusic-sid` crate to play Commodore 64 `.sid` / `.psid` / `.rsid`
tunes. The files here are an **unmodified** copy of the upstream sources; see
"Building" below for how they are compiled without patching them.

## Upstream

- Project: cRSID by Hermit (a.k.a. *crsid-by-hermit*)
- Repository: <https://github.com/r-moeritz/crsid-by-hermit>
- Commit: `641f09a380ca05374102354a9934986950e625c3` (2022-03-30)
- Original author's site: <http://hermit.sidrip.com>

The predecessor engines **cSID** / **cSID-light** (<https://github.com/mlund/csid>,
upstream <http://hermit.uw.hu>) use the same licence; cRSID is a later,
integer-only rewrite with cycle-exact CPU/ADSR emulation and RealSID
(CIA/VIC/IRQ/NMI) support, so only cRSID is vendored here.

## Licence and attribution

cRSID is distributed under a permissive, WTFPL-style licence that requests
attribution. The upstream `README.txt` (kept verbatim next to this file) states:

> License is still WTF: Do what the fuck you want with this code, but
>                       it would be nice mentioning me as the original author.

The same notice appears at the top of `libcRSID.c`:

> License: WTF - do what the fuck you want with the code, but please mention me
> as the original author

This is a permissive licence: it permits use, modification and redistribution,
including in MIT-licensed and closed-source projects, with attribution
requested (not required as a condition). **Attribution is given here and in the
`emusic-sid` crate documentation.** No copyleft obligations are inherited.

## Files

- `libcRSID.c`, `libcRSID.h` — engine entry points and the public structs.
- `C64/` — the 6502 CPU, memory, CIA, VIC and SID emulation.
- `host/audio.c` — `cRSID_generateSample`, the per-sample generator used here.
- `host/file.c` — `cRSID_processSIDfile`, which loads tune bytes into C64 RAM.
- `README.txt`, `ChangeLog.md` — upstream documentation, kept verbatim.

The standalone `cRSID.c` player and its SDL host (`cRSID_initSound`,
`cRSID_generateSound`) are **not** vendored: they pull in SDL and are not needed
for embedding. Everything is compiled by `build.rs` via `cc`; see
`src/shim.c` for the thin C ABI the Rust side calls.

## Building

These files are used verbatim; the only added source is `src/shim.c`, which
`#include`s `libcRSID.c` and adds small non-`static` wrappers, because cRSID's
`cRSID_generateSample` is declared `static inline` and therefore not linkable
from Rust directly.

cRSID uses **GCC nested functions** (e.g. the `loadReg`/`rd`/`addrMode*` helpers
inside `cRSID_emulateCPU`), a GCC extension that MSVC and clang do not
implement. The C side is therefore built with a MinGW-w64 GCC via the `cc`
crate, even though the Rust side targets MSVC; the resulting objects link into
the MSVC binary with no MinGW runtime DLLs. `build.rs` locates a compiler via
`EMUSIC_SID_CC`, then the usual install locations (`C:\mingw64`, `C:\msys64`,
`D:\msys2`, ...), then `PATH`. GitHub's Windows runners ship MinGW-w64 at
`C:\mingw64\bin`, so CI works out of the box.
