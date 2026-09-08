# SmithSphere

[简体中文](README.zh-CN.md)

SmithSphere maps the whole complex impedance plane onto a sphere and reads both halves on two planar charts. The right chart is the Smith chart for R > 0. The left chart is a mirrored projection for R < 0. Rust, egui, native and WebAssembly.

[Open SmithSphere in your browser](https://vowstar.github.io/smith-sphere/)

## Views

The sphere, the two charts, and the details panel share one selection, one hover preview, and one frequency slider. Rotating the sphere leaves the charts unchanged. A point appears on the chart that owns its sign of R, and the other chart states where the point is. Points with R = 0 appear on both charts. A trace that crosses R = 0 is split at an interpolated crossing, drawn as a hollow diamond.

| Action | Result |
| --- | --- |
| Hover | Preview a point |
| Click | Fix the selection |
| Drag, wheel, pinch | Rotate and zoom the sphere |
| Slider, left and right arrow keys | Step through sampled points |

Narrow windows stack the views in one column.

## Inputs

| Source | Accepted content |
| --- | --- |
| Manual Z | R and X in ohm, or an expression such as `25+j30`, `-20-j5`, `j50`, with an optional frequency |
| Manual Γ | Magnitude and phase, or real and imaginary, with its Z0 |
| Touchstone 1.x | `.s1p` and `.s2p` in RI, MA, or DB form, Hz to GHz, comments, wrapped rows. The noise block is skipped |
| CSV or TSV | `frequency`, `R`, `X` columns, or reflection columns `re`/`im`, `mag`/`phase`, `dB`/`phase` |
| Paste | The same parsers with an explicit or automatic format choice |
| Examples | Series RLC sweep, negative resistance device, R = 0 crossing, six landmark points |

The reference impedance declared in the file wins and is shown. A CSV without a frequency unit asks for one. Reflection columns without a Z0 column ask for a reference. An `.s2p` file shows S11 and offers S22. Both are port reflections with the other port matched, and S21 and S12 are never plotted. Touchstone 2.0, mixed mode, more than two ports, G and H parameters, and tables without phase are rejected with the reason. Changing the plot Z0 recomputes every view from the physical impedance.

Sample files are in `examples/`. `scripts/generate_examples.py` produces them.

## Mathematics

With z = Z/Z0 = r + jx and d = r² + x² + 1:

```text
u = (r² + x² − 1)/d    v = 2x/d    w = 2r/d
```

| Region | Condition |
| --- | --- |
| Positive resistance hemisphere | w > 0 |
| Negative resistance hemisphere | w < 0 |
| Inductive | v > 0 |
| Capacitive | v < 0 |

| Z | Sphere point |
| --- | --- |
| 0 | (−1, 0, 0) |
| ∞ | (1, 0, 0) |
| ±Z0 | (0, 0, ±1) |
| ±jZ0 | (0, ±1, 0) |

The positive chart is p+ = (z − 1)/(z + 1) = (u + jv)/(1 + w), the classic Smith chart for r ≥ 0.

The negative chart is p− = (conj z + 1)/(conj z − 1) = (u + jv)/(1 − w) for r ≤ 0. It is a compressed mirrored projection. Its radius is not |Γ|. Its centre is Z = −Z0, where Γ = (Z − Z0)/(Z + Z0) diverges.

Both charts keep inductive on top, short on the left, open on the right, and share the R = 0 rim. The grids come from these relations. Z = ∞, a divergent Γ, and the undefined phase at Γ = 0 are reported as such, never as NaN.

## Build

Rust 1.95 or newer.

```sh
cargo run --release -p smith-sphere-gui
```

WebAssembly:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.21.14 --locked
trunk serve
trunk build --release --public-url ./
```

`trunk build` writes a static site to `dist/`.

`cargo install trunk` can fail inside `libdeflate-sys` on a new GCC or Clang. Set CFLAGS for the install:

```sh
CFLAGS="-DLIBDEFLATE_ASSEMBLER_DOES_NOT_SUPPORT_AVX512VNNI \
-DLIBDEFLATE_ASSEMBLER_DOES_NOT_SUPPORT_AVX_VNNI \
-DLIBDEFLATE_ASSEMBLER_DOES_NOT_SUPPORT_VPCLMULQDQ \
-DLIBDEFLATE_ASSEMBLER_DOES_NOT_SUPPORT_AVX512BW" \
cargo install trunk --version 0.21.14 --locked
```

## Tests

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo check --workspace --target wasm32-unknown-unknown
scripts/capture_layouts.sh
```

[docs/acceptance.md](docs/acceptance.md) maps each requirement to its test. Run `python3 scripts/subset_font.py` after changing interface strings.

## Scope

Not in this version: Touchstone 2.0, mixed mode and multiport files, load terminations, admittance grids, Q circles, complex Z0, and data export. Negative resistance is displayed as such and is not read as instability.

## References and license

The sphere follows C. Zelley, [A spherical representation of the Smith chart](https://ieeexplore.ieee.org/document/4213223/), IEEE Microwave Magazine 8(3), 2007, and A. A. Muller et al., [A 3-D Smith Chart Based on the Riemann Sphere for Active and Passive Microwave Circuits](https://ieeexplore.ieee.org/document/5766788/), IEEE Microwave and Wireless Components Letters 21(6), 2011. File parsing follows the [Touchstone 2.0 specification](https://ibis.org/touchstone_ver2.0/touchstone_ver2_0.pdf) from the IBIS Open Forum. Thanks to their authors.

The interface font SmithSphere Sans is a renamed glyph subset of Source Han Sans CN by Adobe under the SIL Open Font License 1.1. The application is built with egui and eframe. Font notices and the list of compiled crates are in [LICENSES](LICENSES/).

Licensed under either the [Apache License 2.0](LICENSE-APACHE) or the [MIT License](LICENSE-MIT), at your option.
