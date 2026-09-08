#!/usr/bin/env python3
"""Build the bundled UI font subset.

Scans the Rust sources for every character the interface can display, adds
the Latin, Greek, and symbol ranges used for engineering notation, and writes
a subset of Source Han Sans CN Regular (SIL Open Font License 1.1) to
assets/fonts/. Run it after changing user-facing strings:

    python3 scripts/subset_font.py

Requires fonttools (pyftsubset) and the system Source Han Sans CN font.
"""

from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_SOURCE = pathlib.Path("/usr/share/fonts/source-han-sans/SourceHanSansCN-Regular.otf")
OUTPUT = ROOT / "assets" / "fonts" / "SourceHanSansCN-Regular.subset.otf"
CHARSET_RECORD = ROOT / "assets" / "fonts" / "charset.txt"

# Ranges that must always be present regardless of the current source text.
BASE_RANGES = [
    (0x0020, 0x007E),  # ASCII printable
    (0x00A0, 0x00FF),  # Latin-1 supplement (degree, micro, plus-minus, ohm-like)
    (0x0391, 0x03A9),  # Greek capitals
    (0x03B1, 0x03C9),  # Greek small letters
    (0x2010, 0x2027),  # dashes, quotes, bullet, ellipsis
    (0x2030, 0x203A),  # per mille, prime, guillemets
    (0x2190, 0x2199),  # arrows
    (0x2200, 0x22FF),  # mathematical operators
    (0x3000, 0x303F),  # CJK punctuation
    (0xFF01, 0xFF5E),  # full-width forms
]
EXTRA_CHARS = "ΩΓ°∞±×÷−≤≥≠≈·•…→←↑↓√∠"


def collect_source_characters() -> set[str]:
    characters: set[str] = set()
    for path in (ROOT / "crates").rglob("*.rs"):
        text = path.read_text(encoding="utf-8")
        characters.update(c for c in text if ord(c) > 0x7F)
    return characters


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=pathlib.Path, default=DEFAULT_SOURCE)
    parser.add_argument("--output", type=pathlib.Path, default=OUTPUT)
    args = parser.parse_args()

    if not args.source.exists():
        print(f"source font not found: {args.source}", file=sys.stderr)
        return 1

    characters = collect_source_characters()
    characters.update(EXTRA_CHARS)
    for start, end in BASE_RANGES:
        characters.update(chr(code) for code in range(start, end + 1))

    ordered = sorted(characters)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    text_file = args.output.parent / "charset.txt"
    text_file.write_text("".join(c for c in ordered if not c.isspace()) + "\n", encoding="utf-8")

    command = [
        "pyftsubset",
        str(args.source),
        f"--text-file={text_file}",
        f"--output-file={args.output}",
        "--layout-features=kern,liga,calt",
        "--no-hinting",
        "--desubroutinize",
        "--name-IDs=0,1,2,3,4,5,6,13,14",
        "--drop-tables+=DSIG",
    ]
    subprocess.run(command, check=True)
    size = args.output.stat().st_size
    print(f"wrote {args.output} ({size} bytes, {len(ordered)} characters) and {CHARSET_RECORD.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
