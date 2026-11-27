#!/usr/bin/env python3
# =============================================================================
# EH3D-PHOENIX-3DP - tools/make_screenshots.py
# Copyright (C) 2026 JuanenRac (Electro Hobby 3D) <electrohobby3d@gmail.com>
# GPL-3.0-or-later - see LICENSE
# =============================================================================
"""Takes the pictures of the window for the READMEs with the picture mode of the program, one for each tab and two of a project of the suite (its attachments and its G-code).

    py -3.14 -I tools/make_screenshots.py PATH/TO/phoenix-3dp[.exe] [--out window-pictures] [--lang en] [--size 1360x860] [--suite-tool PATH/TO/eh3d-project]
    py -3.14 -I tools/make_screenshots.py --check [--out window-pictures]

The program must have been built with `--features gui`. It is copied into a new empty folder and run from there, with the folders of the user (APPDATA,
LOCALAPPDATA, USERPROFILE, HOME) pointing at that folder, so no picture carries the name of a folder or of a person. Each picture is quantized to a palette with Pillow and made
smaller until it is at most 250 KB; the program is asked for `--demo` (a model and a server made in memory), so nothing here reaches a network or a printer. The picture of the attachments needs the tool `eh3d-project` of EH3D-COMMON (`--suite-tool`): a made-up project is made with it
and with the command `project` of the program, in the same empty folder.
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
PICTURES = WINDOWS + ("attachments", "program")
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


def made_up_program() -> str:
    """A made-up G-code program to look at: one layer of a square ring filled with lines, and a command that stores settings, to show that it is told."""
    lines = ["G28", "M140 S60", "M104 S210", "M83", "G1 Z0.2 F600"]
    for inset in (0.0, 1.0):
        lo, hi = 70.0 + inset, 186.0 - inset
        lines.append(f"G1 X{lo} Y{lo} F6000")
        for x, y in ((hi, lo), (hi, hi), (lo, hi), (lo, lo)):
            lines.append(f"G1 X{x} Y{y} E2 F1800")
    for k in range(1, 28):
        at = 72.0 + 4.0 * k
        ends = (at, 72.0, 72.0, at) if at <= 184.0 else (184.0, at - 112.0, at - 112.0, 184.0)
        lines.append(f"G1 X{ends[0]} Y{ends[1]} F6000")
        lines.append(f"G1 X{ends[2]} Y{ends[3]} E1 F1800")
    lines += ["M500", "M104 S0", "M140 S0"]
    return "\n".join(lines) + "\n"


def made_up_project(exe: Path, tool: Path, work: Path, env: dict) -> None:
    """A project `demo.eh3d` in `work` with the sample cube on its bed, a note and a document with macros (made-up bytes), to show the panel and its warning."""
    shutil.copy2(Path(__file__).resolve().parent.parent / "examples" / "cube-20mm.stl", work / "cube-20mm.stl")
    (work / "assembly-guide.md").write_text("# Assembly guide\n\n1. Print the cube with three walls.\n2. Measure it with a calliper.\n", encoding="utf-8")
    (work / "label-maker.docm").write_bytes(b"made-up bytes for a picture")
    (work / "job.gcode").write_text(made_up_program(), encoding="utf-8")
    commands = [
        [str(exe), "project", "save", "demo.eh3d", "--model", "cube-20mm.stl", "--printer", "bambu-lab-p1s", "--gcode", "job.gcode"],
        [str(tool), "attach", "demo.eh3d", "assembly-guide.md", "--title", "Assembly guide", "--ref", "PART1"],
        [str(tool), "attach", "demo.eh3d", "label-maker.docm", "--title", "Label maker"],
    ]
    for command in commands:
        done = subprocess.run(command, cwd=work, env=env, capture_output=True, text=True, timeout=120,
                              creationflags=0x08000000 if os.name == "nt" else 0)
        if done.returncode not in (0, 1):
            raise SystemExit(f"{command[1]}: {done.returncode}: {done.stderr.strip()[:300]}")


def take(binary: Path, window: str, lang: str, size: str, out: Path, tool: Path | None = None) -> None:
    with tempfile.TemporaryDirectory(prefix="p3dp_shot_") as tmp:
        work = Path(tmp)
        exe = work / binary.name
        shutil.copy2(binary, exe)
        env = {k: v for k, v in os.environ.items() if k.upper() in ("PATH", "SYSTEMROOT", "WINDIR", "COMSPEC", "DISPLAY", "XAUTHORITY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR")}
        for name in ("APPDATA", "LOCALAPPDATA", "USERPROFILE", "HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME"):
            env[name] = str(work)
        args = [str(exe), "gui", "--shot", "raw.png", "--window", window, "--demo", "--lang", lang,
                "--size", size, "--wait", "12"]
        if window in ("attachments", "program") and tool is not None:
            made_up_project(exe, tool, work, env)
            # the project is read on a thread of its own, so the picture waits longer
            args = [str(exe), "gui", "--shot", "raw.png", "--window", "project", "--open", "demo.eh3d", "--lang", lang,
                    "--size", size, "--wait", "90"]
            if window == "program":
                # the panel Project is the one that shows the G-code of the project
                args += ["--panel", "project"]
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
    for window in PICTURES:
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
    ap.add_argument("--suite-tool", help="the tool eh3d-project of EH3D-COMMON, for the picture of the attachments")
    a = ap.parse_args()
    out = Path(a.out)
    if a.check:
        return check(out)
    if not a.binary:
        ap.error("give the program, or --check")
    binary = Path(a.binary)
    if not binary.is_file():
        raise SystemExit(f"{binary} is not a file")
    if not a.suite_tool or not Path(a.suite_tool).is_file():
        ap.error("give the tool eh3d-project with --suite-tool")
    tool = Path(a.suite_tool).resolve()
    for window in PICTURES:
        take(binary, window, a.lang, a.size, out / f"{window}.png", tool)
    return check(out)


if __name__ == "__main__":
    sys.exit(main())
