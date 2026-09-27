#!/usr/bin/env python3
"""Build assets/icns/emusic-app.icns from the committed Windows icon.

The macOS bundle (installer/macos/) uses the same artwork as the Windows side
(assets/ico/emusic-app.ico). Pillow can read the ICO's native frames; this
script packs 16/32/64/128/256 px PNG frames into a standard `.icns` container.
Run it after changing the icon:

    python scripts/make-icns.py

Requires Pillow (`pip install Pillow`).
"""

import io
import os
import struct
import sys

from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "assets", "ico", "emusic-app.ico")
OUT = os.path.join(ROOT, "assets", "icns", "emusic-app.icns")

# ICNS PNG chunk type per pixel size (standard ic<NN>/icp<N> codes).
CHUNK_TYPES = {
    16: b"icp4",
    32: b"icp5",
    64: b"icp6",
    128: b"ic07",
    256: b"ic08",
}


def main() -> int:
    icon = Image.open(SRC)
    if icon.format != "ICO":
        print(f"{SRC} is not an ICO", file=sys.stderr)
        return 1
    native = sorted(icon.ico.sizes(), key=lambda size: size[0])
    largest = icon.convert("RGBA")

    chunks = []
    for size, chunk_type in CHUNK_TYPES.items():
        if (size, size) in native:
            icon.size = (size, size)
            frame = icon.convert("RGBA")
        else:
            frame = largest.resize((size, size), Image.LANCZOS)
        buffer = io.BytesIO()
        frame.save(buffer, format="PNG")
        data = buffer.getvalue()
        chunks.append(chunk_type + struct.pack(">I", len(data) + 8) + data)

    body = b"".join(chunks)
    icns = b"icns" + struct.pack(">I", len(body) + 8) + body
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "wb") as f:
        f.write(icns)
    print(f"wrote {OUT} ({len(icns)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
