#!/usr/bin/env bash
# Captures the native window at desktop and narrow sizes under Xvfb.
#
# Usage: scripts/capture_layouts.sh [output-dir] [language]
#
# language is zh or en and defaults to zh. Each capture starts the application
# with demonstration data preloaded via SMITH_SPHERE_STARTUP, selects a point
# with SMITH_SPHERE_SELECT, sets the interface language with SMITH_SPHERE_LANG,
# and writes a PNG through the `capture` cargo feature. The narrow window is
# tall enough to hold the whole stacked layout so the screenshot is not cut.
# Requires xvfb-run and a Mesa software OpenGL driver.
set -euo pipefail

# winit prefers Wayland when WAYLAND_DISPLAY is set, so the window would open
# on the desktop compositor and never render under Xvfb.
unset WAYLAND_DISPLAY

out="${1:-private/captures}"
lang="${2:-zh}"
mkdir -p "$out"
cargo build -p smith-sphere-gui --features capture --release
bin="target/release/smith-sphere-gui"

# name size startup select [view] [screen]
capture() {
  local name="$1" size="$2" startup="$3" select="$4" view="${5:-}" screen="${6:-1600x1200}"
  SMITH_SPHERE_WINDOW="$size" SMITH_SPHERE_STARTUP="$startup" SMITH_SPHERE_SELECT="$select" \
    SMITH_SPHERE_VIEW="$view" SMITH_SPHERE_LANG="$lang" SMITH_SPHERE_CAPTURE="$out/$name.png" \
    xvfb-run -a -s "-screen 0 ${screen}x24" "$bin"
  echo "wrote $out/$name.png"
}

capture desktop-crossing 1280x820 crossing 12
capture desktop-rlc-negative 1280x820 rlc,negative 30
capture desktop-landmarks 1280x820 landmarks 0 positive
capture narrow-crossing 400x1600 crossing 25 "" 700x1700
capture welcome 1100x760 "" 0
