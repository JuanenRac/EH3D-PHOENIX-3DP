#!/usr/bin/env bash
# Runs phoenix-3dp with its sample. Extra arguments replace the sample: ./run.sh [ARGUMENTS]
set -u
cd "$(dirname "$0")"
pause() { [ -n "${NOPAUSE:-}" ] || { read -r -p "Press Enter to close..." _ || true; }; }
trap pause EXIT
if [ "$#" -eq 0 ]; then set -- info examples/cube-20mm.stl --printer bambu-lab-a1; fi
cargo run --release --quiet -- "$@"
