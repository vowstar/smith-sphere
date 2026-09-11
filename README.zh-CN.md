# SmithSphere 史密斯球

[English](README.md)

SmithSphere 把完整的复阻抗平面映射到球面，并用两张平面圆图读取球的两半。右图是 R > 0 的史密斯圆图，左图是 R < 0 的镜像投影。Rust 与 egui 编写，支持原生与 WebAssembly。

[在浏览器中打开 SmithSphere](https://vowstar.github.io/smith-sphere/)

![中文界面下的 SmithSphere，桌面布局与窄屏布局并排，展示 R = 0 穿越示例](https://github.com/user-attachments/assets/bf265a80-c751-4c77-82e9-e46a972656b3)

## 视图

球面、两张圆图和详情面板共用同一个选中点、悬停预览和频率滑块。旋转球面不改变圆图的方向。一个点只出现在其 R 符号所属的圆图上，另一张圆图会说明它在哪里。R = 0 的点在两张圆图上同时出现。穿越 R = 0 的轨迹会在插值得到的交点处断开，交点画成空心菱形。

界面支持中文和英文。启动时按浏览器或系统语言显示，工具栏里的选择器可随时切换。

| 操作 | 结果 |
| --- | --- |
| 悬停 | 预览一个点 |
| 单击 | 固定选中点 |
| 拖动、滚轮、双指缩放 | 旋转和缩放球面 |
| 滑块、左右方向键 | 在采样点之间逐点移动 |

窄窗口把各视图竖排成一列。

## 输入

| 来源 | 接受的内容 |
| --- | --- |
| 手动 Z | 以 Ω 为单位的 R 与 X，或 `25+j30`、`-20-j5`、`j50` 这样的表达式，频率可选 |
| 手动 Γ | 幅度与相位，或实部与虚部，并附带其 Z0 |
| Touchstone 1.x | RI、MA 或 DB 形式的 `.s1p` 与 `.s2p`，Hz 到 GHz，支持注释与折行。噪声块被跳过 |
| CSV 或 TSV | `frequency`、`R`、`X` 三列，或反射系数列 `re`/`im`、`mag`/`phase`、`dB`/`phase` |
| 粘贴 | 同一套解析器，格式可指定或自动识别 |
| 示例 | 串联 RLC 扫频、负电阻器件、穿越 R = 0 的轨迹、六个标志点 |

文件中声明的参考阻抗优先并显示出来。没有频率单位的 CSV 会询问单位。没有 Z0 列的反射系数数据会询问参考阻抗。`.s2p` 默认显示 S11 并可切换到 S22，两者都是另一端口匹配时的端口反射，S21 与 S12 不会被绘制。Touchstone 2.0、混合模、三端口以上、G 和 H 参数以及没有相位的表格都会被拒绝并说明原因。修改绘图 Z0 会从物理阻抗重新计算所有视图。

示例文件在 `examples/` 中。合成数据由 `scripts/generate_examples.py` 生成，`kc901v_open_s11.s1p` 是用 kcsdi-rs 从 KC901V 导出的开路端口实测扫频，5 kHz 到 650 MHz。

## 数学约定

令 z = Z/Z0 = r + jx，d = r² + x² + 1：

```text
u = (r² + x² − 1)/d    v = 2x/d    w = 2r/d
```

| 区域 | 条件 |
| --- | --- |
| 正电阻半球 | w > 0 |
| 负电阻半球 | w < 0 |
| 感性 | v > 0 |
| 容性 | v < 0 |

| Z | 球面上的点 |
| --- | --- |
| 0 | (−1, 0, 0) |
| ∞ | (1, 0, 0) |
| ±Z0 | (0, 0, ±1) |
| ±jZ0 | (0, ±1, 0) |

正圆图为 p+ = (z − 1)/(z + 1) = (u + jv)/(1 + w)，即 r ≥ 0 的传统史密斯圆图。

负圆图为 p− = (conj z + 1)/(conj z − 1) = (u + jv)/(1 − w)，对应 r ≤ 0。它是压缩后的镜像投影，半径不是 |Γ|。圆心对应 Z = −Z0，此处 Γ = (Z − Z0)/(Z + Z0) 发散。

两张圆图都保持感性在上、短路在左、开路在右，并共用 R = 0 的外圈。网格由上述关系生成。Z = ∞、发散的 Γ 以及 Γ = 0 时无定义的相位都按原样报告，不会出现 NaN。

## 构建

需要 Rust 1.95 或更新版本。

```sh
cargo run --release -p smith-sphere-gui
```

WebAssembly：

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.21.14 --locked
trunk serve
trunk build --release --public-url ./
```

`trunk build` 把静态站点写入 `dist/`。

`cargo install trunk` 在较新的 GCC 或 Clang 上可能在 `libdeflate-sys` 处失败。安装时设置 CFLAGS：

```sh
CFLAGS="-DLIBDEFLATE_ASSEMBLER_DOES_NOT_SUPPORT_AVX512VNNI \
-DLIBDEFLATE_ASSEMBLER_DOES_NOT_SUPPORT_AVX_VNNI \
-DLIBDEFLATE_ASSEMBLER_DOES_NOT_SUPPORT_VPCLMULQDQ \
-DLIBDEFLATE_ASSEMBLER_DOES_NOT_SUPPORT_AVX512BW" \
cargo install trunk --version 0.21.14 --locked
```

## 测试

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo check --workspace --target wasm32-unknown-unknown
scripts/capture_layouts.sh
```

[docs/acceptance.md](docs/acceptance.md) 把每条需求对应到测试。修改界面文案后运行 `python3 scripts/subset_font.py`。

## 范围

本版本不包含：Touchstone 2.0、混合模与多端口文件、负载终接计算、导纳网格、Q 圆、复数 Z0 和数据导出。负电阻按负电阻显示，不解释为不稳定。

## 参考与许可

SmithSphere 画的是阻抗平面的黎曼球。它所依据的来源：

| 来源 | 提供的内容 |
| --- | --- |
| P. H. Smith，Transmission-line calculator，Electronics 12(1)，1939，以及 An improved transmission-line calculator，Electronics 17(1)，1944 | 史密斯圆图 |
| H. F. Lenzing 与 C. D'Elio，Transmission line parameters with negative conductance loads and the negative Smith chart，Proc. IEEE 51(3)，1963 | 负电阻圆图 |
| C. Zelley，[A spherical representation of the Smith chart](https://ieeexplore.ieee.org/document/4213223/)，IEEE Microwave Magazine 8(3)，2007 | 把阻抗平面包在球面上，负电阻占另一半球 |
| A. A. Muller、P. Soto、D. Dascalu、D. Neculoiu 与 V. E. Boria，[A 3-D Smith Chart Based on the Riemann Sphere for Active and Passive Microwave Circuits](https://ieeexplore.ieee.org/document/5766788/)，IEEE Microwave and Wireless Components Letters 21(6)，2011 | 作为黎曼球面球极投影的三维史密斯圆图 |
| IBIS Open Forum，[Touchstone 2.0 规范](https://ibis.org/touchstone_ver2.0/touchstone_ver2_0.pdf) | 文件格式 |

代码、网格、界面和文案均为原创。感谢以上作者。

界面字体 SmithSphere Sans 是 Adobe Source Han Sans CN 的改名字形子集，遵循 SIL Open Font License 1.1。应用使用 egui 与 eframe 构建。字体声明和编译进应用的 crate 清单见 [LICENSES](LICENSES/)。

以 [Apache License 2.0](LICENSE-APACHE) 或 [MIT License](LICENSE-MIT) 双许可发布，任选其一。
