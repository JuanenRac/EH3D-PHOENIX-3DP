#!/usr/bin/env python3
# =============================================================================
# EH3D-PHOENIX-3DP - tools/make_screenshots.py
# Copyright (C) 2026 JuanenRac (Electro Hobby 3D) <electrohobby3d@gmail.com>
# GPL-3.0-or-later - see LICENSE
# =============================================================================
"""Takes the pictures of the window for the READMEs with the picture mode of the program, one for each tab.

    py -3.14 -I tools/make_screenshots.py PATH/TO/phoenix-3dp[.exe] [--out window-pictures] [--lang en] [--size 1360x860]
    py -3.14 -I tools/make_screenshots.py --check [--out window-pictures]

The program must have been built with `--features gui`. It is copied into a new empty folder and run from there, with the folders of the user (APPDATA,
LOCALAPPDATA, USERPROFILE, HOME) pointing at that folder, so no picture carries the name of a folder or of a person. Each picture is quantized to a palette with Pillow and made
smaller until it is at most 250 KB; the program is asked for `--demo` (a model and a server made in memory), so nothing here reaches a network or a printer.
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

MAX_BYTES = 250 * 1024
WINDOWS = ("prepare", "preview", "device", "project")
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


def quantize(src: Path, dst: Path) -> int:
    """Writes `dst` as a PNG of a palette that is at most MAX_BYTES; returns its size."""
    from PIL import Image  # Pillow, only needed to take pictures

    image = Image.open(src).convert("RGB")
    for width_share in (1.0, 0.85, 0.7, 0.55):
        scaled = image if width_share == 1.0 else image.resize(
            (int(image.width * width_share), int(image.height * width_share)), Image.LANCZOS)
        for colours in (256, 192, 128, 96, 64, 48, 32):
            small = scaled.quantize(colors=colours, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
            small.save(dst, format="PNG", optimize=True)
            if dst.stat().st_size <= MAX_BYTES:
                return dst.stat().st_size
    raise SystemExit(f"{dst.name}: could not be made smaller than {MAX_BYTES} bytes")


def take(binary: Path, window: str, lang: str, size: str, out: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="p3dp_shot_") as tmp:
        work = Path(tmp)
        exe = work / binary.name
        shutil.copy2(binary, exe)
        env = {k: v for k, v in os.environ.items() if k.upper() in ("PATH", "SYSTEMROOT", "WINDIR", "COMSPEC", "DISPLAY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR")}
        for name in ("APPDATA", "LOCALAPPDATA", "USERPROFILE", "HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME"):
            env[name] = str(work)
        args = [str(exe), "gui", "--shot", "raw.png", "--window", window, "--demo", "--lang", lang,
                "--size", size, "--wait", "12"]
        if window == "preview":
            # look inside, so the picture shows the walls and the infill and not only the top skin
            args += ["--slice", "--peek"]
        done = subprocess.run(args, cwd=work, env=env, capture_output=True, text=True, timeout=300,
                              creationflags=0x08000000 if os.name == "nt" else 0)
        raw = work / "raw.png"
        if done.returncode != 0 or not raw.is_file():
            raise SystemExit(f"{window}: the program did not write the picture ({done.returncode}): {done.stderr.strip()[:300]}")
        out.parent.mkdir(parents=True, exist_ok=True)
        size_bytes = quantize(raw, out)
        print(f"{out.name}: {size_bytes} bytes")


def check(out: Path) -> int:
    bad = 0
    for window in WINDOWS:
        f = out / f"{window}.png"
        if not f.is_file():
            print(f"missing {f}")
            bad += 1
            continue
        data = f.read_bytes()
        if not data.startswith(PNG_MAGIC):
            print(f"{f.name}: not a PNG file")
            bad += 1
        if len(data) > MAX_BYTES:
            print(f"{f.name}: {len(data)} bytes, more than {MAX_BYTES}")
            bad += 1
        for needle in (b"Users", b"home/", b"AppData"):
            if needle in data:
                print(f"{f.name}: carries the text {needle!r}")
                bad += 1
    print("SCREENSHOTS=" + ("PASS" if bad == 0 else f"FAIL ({bad})"))
    return 1 if bad else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("binary", nargs="?", help="the program built with --features gui")
    ap.add_argument("--out", default="window-pictures")
    ap.add_argument("--lang", default="en")
    ap.add_argument("--size", default="1360x860")
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    out = Path(a.out)
    if a.check:
        return check(out)
    if not a.binary:
        ap.error("give the program, or --check")
    binary = Path(a.binary)
    if not binary.is_file():
        raise SystemExit(f"{binary} is not a file")
    for window in WINDOWS:
        take(binary, window, a.lang, a.size, out / f"{window}.png")
    return check(out)


if __name__ == "__main__":
    sys.exit(main())
