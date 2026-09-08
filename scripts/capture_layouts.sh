#!/usr/bin/env bash
# Captures the native window at desktop and narrow sizes under Xvfb.
#
# Usage: scripts/capture_layouts.sh [output-dir]
#
# Each capture starts the application with demonstration data preloaded via
# SMITH_SPHERE_STARTUP, selects a point with SMITH_SPHERE_SELECT, and writes a
# PNG through the `capture` cargo feature. Requires xvfb-run and a Mesa
# software OpenGL driver.
set -euo pipefail

# winit prefers Wayland when WAYLAND_DISPLAY is set, so the window would open
# on the desktop compositor and never render under Xvfb.
unset WAYLAND_DISPLAY

out="${1:-private/captures}"
mkdir -p "$out"
cargo build -p smith-sphere-gui --features capture --release
bin="target/release/smith-sphere-gui"

capture() {
  local name="$1" size="$2" startup="$3" select="$4" view="${5:-}"
  SMITH_SPHERE_WINDOW="$size" SMITH_SPHERE_STARTUP="$startup" SMITH_SPHERE_SELECT="$select" \
    SMITH_SPHERE_VIEW="$view" SMITH_SPHERE_CAPTURE="$out/$name.png" \
    xvfb-run -a -s "-screen 0 1600x1200x24" "$bin"
  echo "wrote $out/$name.png"
}

capture desktop-crossing 1280x820 crossing 12
capture desktop-rlc-negative 1280x820 rlc,negative 30
capture desktop-landmarks 1280x820 landmarks 0 positive
capture narrow-crossing 390x900 crossing 25
capture welcome 1100x700 "" 0
