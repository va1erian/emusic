#!/usr/bin/env python3
"""Regenerate the MP3 fixtures of emusic-lazyaudio's tests with ffmpeg (libmp3lame).

Every file is a pure sine, so a test can tell what it decoded by its
frequency: 440 Hz unless noted, 1 s long. Run from anywhere; ffmpeg must be on
PATH. The outputs are small and checked in, so the tests need no encoder.
"""

import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent


def tone(freq, seconds, rate=44100):
    return ["-f", "lavfi", "-i", f"sine=frequency={freq}:sample_rate={rate}:duration={seconds}"]


FIXTURES = {
    # 128 kbit/s CBR stereo at 44.1 kHz (LAME writes an Info header).
    "cbr.mp3": tone(440, 1) + ["-ac", "2", "-b:a", "128k"],
    # VBR (quality 4) with a Xing/LAME header, which carries the frame count.
    "vbr.mp3": tone(440, 1) + ["-ac", "2", "-q:a", "4"],
    # CBR with an ID3v2.3 tag in front and an ID3v1 tag behind.
    "tags.mp3": tone(440, 1) + ["-ac", "2", "-b:a", "128k", "-id3v2_version", "3",
                                "-write_id3v1", "1", "-metadata", "title=Fixture",
                                "-metadata", "artist=emusic"],
    # Mono.
    "mono.mp3": tone(440, 1) + ["-ac", "1", "-b:a", "64k"],
    # MPEG-2 Layer III at 22.05 kHz.
    "mpeg2.mp3": tone(440, 1, 22050) + ["-ac", "2", "-b:a", "64k"],
    # 440 Hz for 1 s then 660 Hz for 1 s, for seeking.
    "two_tones.mp3": ["-f", "lavfi", "-i",
                      "sine=frequency=440:sample_rate=44100:duration=1",
                      "-f", "lavfi", "-i",
                      "sine=frequency=660:sample_rate=44100:duration=1",
                      "-filter_complex", "[0:a][1:a]concat=n=2:v=0:a=1",
                      "-ac", "2", "-b:a", "128k"],
}

for name, args in FIXTURES.items():
    subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", *args,
                    "-c:a", "libmp3lame", str(HERE / name)], check=True)
    print(name, (HERE / name).stat().st_size)
