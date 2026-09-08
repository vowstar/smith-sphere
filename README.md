# SmithSphere 史密斯球

Interactive RF visualization that maps the complete complex impedance plane onto
a sphere and reads the negative- and positive-resistance halves on two planar
circle charts. Rust, egui/eframe, native and WebAssembly.

把完整复阻抗平面映射到球面，并用左下“负电阻区 / R < 0”和右下“正电阻区 /
R > 0”两张圆图精确读取阻抗、反射系数以及它们随频率的变化。

## 运行 / Running

Requires Rust 1.95 or newer (edition 2024).

**Native:**

```sh
cargo run --release -p smith-sphere-gui
```

**WebAssembly (Trunk):**

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.21.14 --locked
trunk serve            # http://127.0.0.1:8080
trunk build --release --public-url ./   # static site in dist/
```

If `trunk` fails to compile `libdeflate-sys` on a very new GCC or Clang, set
`CFLAGS="-DLIBDEFLATE_ASSEMBLER_DOES_NOT_SUPPORT_AVX512VNNI
-DLIBDEFLATE_ASSEMBLER_DOES_NOT_SUPPORT_AVX_VNNI
-DLIBDEFLATE_ASSEMBLER_DOES_NOT_SUPPORT_VPCLMULQDQ
-DLIBDEFLATE_ASSEMBLER_DOES_NOT_SUPPORT_AVX512BW"` for the install.

## 界面 / Layout

Desktop: the rotatable sphere on top, the negative-resistance chart bottom-left,
the positive-resistance chart bottom-right, and a details panel on the right.
Narrow windows stack the views vertically: sphere, negative chart, positive
chart, details, data. The three views share the same data, selection, hover
preview, and frequency slider.

- Hover previews a point; click fixes the selection (dot with dark ring and ticks).
- The frequency slider snaps to sampled points; `←` / `→` move one point.
- The sphere supports drag to rotate, wheel or pinch to zoom, `复位视角`,
  `查看正区`, `查看负区`, and `定位选中点` when the selected point is hidden.
  Rotating the sphere never changes the orientation of the planar charts.
- A point in the positive region only appears on the positive chart; the other
  chart says where the point is. `R = 0` points are highlighted on both charts.
- A trajectory that crosses `R = 0` is split; the interpolated crossing is
  drawn as a hollow diamond and dashed lines, never as a fake sample.

## 输入 / Inputs

| Entry | Details |
| --- | --- |
| 输入阻抗 | `R`, `X` in Ω or an expression such as `25+j30`, `-20-j5`, `j50`; optional frequency; advanced tab for `Γ` (magnitude/phase or real/imaginary) with its `Z0`. |
| 打开文件 / 拖入 | Touchstone 1.x `.s1p` and `.s2p` (RI, MA, DB; Hz–GHz; `!` comments; wrapped rows; noise block skipped); CSV/TSV with `frequency`, `R`, `X` columns, or reflection columns (`re`/`im`, `mag`/`phase`, `dB`/`phase`). |
| 粘贴数据 | Same parsers with an explicit or automatic format choice. |
| 试用示例 | Series RLC sweep, ideal negative-resistance device, an `R = 0` crossing trajectory, and the six landmark points. All are labeled 演示数据. |

Rules the importer follows:

- The file's declared reference impedance wins and is shown. CSV without a
  frequency unit asks for one instead of guessing; reflection columns without a
  `Z0` column ask for a reference.
- `.s2p` shows `S11` by default and offers `S22`. Both are port reflections
  with the other port matched, not `Z11`/`Z22` of the Z matrix. `S21`/`S12`
  are never plotted as reflections.
- Touchstone 2.0 keywords, mixed-mode data, more than two ports, and `G`/`H`
  parameters are rejected with a message naming the supported range.
- Magnitude-only, VSWR, or return-loss tables are rejected with an explanation
  that phase is required for a unique position.
- Changing the plotting `Z0` recomputes every view from the recovered physical
  impedance; the data itself never changes.

Sample files live in `examples/` (see `scripts/generate_examples.py`).

## 数学约定 / Mathematics

With `z = Z/Z0 = r + jx` and `d = r² + x² + 1`:

```text
u = (r² + x² − 1)/d      v = 2x/d      w = 2r/d
```

`w > 0` is the positive-resistance hemisphere, `w < 0` the negative one,
`v > 0` inductive, `v < 0` capacitive. `Z = 0` sits at `(−1, 0, 0)`, `Z = ∞`
at `(1, 0, 0)`, `Z = ±Z0` at `(0, 0, ±1)`, `Z = ±jZ0` at `(0, ±1, 0)`.

- Positive chart: `p+ = (z − 1)/(z + 1) = (u + jv)/(1 + w)`, the classic Smith chart, `r ≥ 0`.
- Negative chart: `p− = (conj z + 1)/(conj z − 1) = (u + jv)/(1 − w)`, `r ≤ 0`.
  This is a compressed mirrored projection; its radius is not `|Γ|` and its
  centre is `Z = −Z0`, where the true `Γ = (Z − Z0)/(Z + Z0)` diverges.
- Both charts keep inductive on top, short on the left, open on the right, and
  share the `R = 0` rim. The grids are generated from these relations; the
  constant-`|r|` and constant-`x` circles have identical geometry in both
  charts with negated resistance labels on the left.

`Z = ∞` and a divergent `Γ` are represented explicitly rather than as NaN or
infinity. `Γ = 0` reports an undefined phase.

## 验证 / Verification

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --features smith-sphere-gui/capture -- -D warnings
cargo test --workspace
cargo check --workspace --target wasm32-unknown-unknown
trunk build --release
scripts/capture_layouts.sh          # native captures at desktop and narrow sizes
python3 scripts/subset_font.py      # after changing interface strings
```

The tests cover the landmark positions, `u² + v² + w² = 1`, both charts staying
inside their unit discs, the shared boundary agreeing in both charts, the
positive chart equalling the classic Smith chart, segmentation at `R = 0`, RI,
MA, and DB Touchstone equivalence, the sample files reproducing their analytic
models through Touchstone and CSV, unit and reference prompts, rejected
formats, `Z0` changes leaving physical impedance untouched, and the layouts
rendering at desktop and narrow widths.

## 已支持与限制 / Scope and limitations

Supported: real positive `Z0`; manual `Z` and `Γ`; CSV/TSV; Touchstone 1.x
`.s1p`/`.s2p` S parameters and one-port normalized `Z`/`Y` parameters; three
linked views with hover, click, slider, and keyboard selection; basic and
detailed impedance grids; Ω grid labels.

Not in this version: Touchstone 2.0, mixed-mode and multi-port files, arbitrary
load terminations, admittance grids, Q circles, complex `Z0`, and data export.
Negative resistance is displayed as such and is not interpreted as instability.

## Acknowledgements

- Interface font: a glyph subset of Source Han Sans CN (Adobe), SIL Open Font
  License 1.1; see `LICENSES/`.
- Built with egui and eframe.
- The sphere follows C. Zelley, IEEE Microwave Magazine 8(3), 2007, and
  A. A. Muller et al., IEEE Microwave and Wireless Components Letters 21(6),
  2011, with the Touchstone 2.0 specification published by the IBIS Open
  Forum. Thanks to their authors.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option. Font licenses and notices are preserved in [LICENSES](LICENSES/).
