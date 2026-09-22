# rgpui 二进制体积优化指南

> 文档日期：2026-09-22
> 基线：`examples/hello_world`，Windows x86_64 MSVC，`cargo build --release`
>
> | 构建 | 优化前 | 优化后 | 说明 |
> |------|--------|--------|------|
> | `release`（日常） | 11.41 MB | 8.92 MB | `-22%`，默认 `cargo build --release` 直接受益 |
> | `size`（打安装包） | — | 5.97 MB | 相对基线 `-48%`，见 §4 |

本文记录 rgpui 体积问题的**分析方法**、已落地的**优化手段**、当前体积**构成**，以及将来可做/难做/不做的优化点。目标是让任何人都能复现归因过程，而不是只记住几个数字。

## 1. 分析方法

### 1.1 测量：先拿到可复现的数字

```powershell
# 本机约定：先切 UTF-8（见 AGENTS.md），再执行
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$PSStyle.OutputRendering = 'PlainText'

cargo build --release -p hello_world
Get-ChildItem -LiteralPath "target\release" -Filter "hello_world*" |
    Select-Object Name, @{N="MB";E={[math]::Round($_.Length/1MB,2)}}
```

注意同时看 `.exe` 和 `.pdb`：PDB 是独立文件，不进安装包，但它的大小变化能佐证符号裁剪是否生效（本轮 `strip` 生效后 PDB 从 5.70MB 降到 2.50MB）。

### 1.2 归因：三板斧

**第一板斧：看有没有配 `[profile.release]`。**

```powershell
rg "\[profile" --glob "Cargo.toml"
```

rgpui 之前全 workspace 一个 profile 都没配，等于用 cargo 默认值发版：`opt-level=3`、`lto=false`、`codegen-units=16`、`strip=none`。这是 11MB 里最不冤但最容易修的一块。

**第二板斧：数依赖，看谁把"重型武器"带进来了。**

```powershell
# 依赖总数（本轮基线：约 480 个）
cargo tree -p hello_world --no-dev-dependencies --prefix none |
    Sort-Object | Get-Unique | Measure-Object
# 反查某个重型 crate 是谁引入的（经典：rav1e 整套 AV1 编码器）
cargo tree -p hello_world --no-dev-dependencies -i rav1e
# 看某个依赖是以哪些 feature 被启用的
cargo tree -p hello_world --no-dev-dependencies -e features -i image
```

判断"重型"的经验法则：编码器（rav1e）、视频/图像全格式解码（exr/tiff/avif）、字体光栅化栈、ICU/unicode 数据表、TLS/HTTP 全功能客户端、async 全家桶（tar/zip/bzip2）。看到名字先反查引入链，再决定是裁特性、加 feature 门，还是不动。

**第三板斧：区分"编译了"和"链接了"。**

这是最容易误判的地方，举两个本轮实例：

- `hdrhistogram`：代码侧早已 `#[cfg(feature = "input-latency-histogram")]` 门控，默认构建里它的代码根本没编译——但依赖是无条件的。把它改成可选依赖后，`cargo tree -i hdrhistogram` 为空，**二进制体积纹丝不动**（链接器本来就没链它），省的只是编译时间。
- `windows` 特性：没被引用的绑定模块同样进不了最终链接（尤其开了 LTO 后）。裁 15 个特性后体积不变，符合预期。

推论：**只有"被实际引用且被链接"的代码才值得动**；先确认引用关系（`rg "use windows::"` 这类审计），再动手。反过来，体积没变化不代表改动无用——依赖卫生和编译时间同样是收益。

### 1.3 踩过的坑

- `use windows::{ Win32::{...} }` 这类嵌套导入，正则 `windows::A::B` 抓不到，必须把 use 块整体 dump 出来审计（本轮就是这么做的，见 §5）。
- `smol::process::windows::CommandExt` 这类路径会污染 `windows::` 的 grep 结果，注意是 `smol` 的模块，不是 `windows` crate。
- `image::ImageFormat::from_extension("avif")` 这类纯字符串映射**不受 feature 门控影响**，只有真正的编解码会失败；静态扩展名列表和解码能力是两回事，改文档时别改错地方。
- Windows 的 `cargo check` 不编译其他平台 `cfg` 代码，跨平台改动靠 CI 矩阵验证（本轮的 windows 特性裁剪同理）。

## 2. 已落地的优化

### 2.1 `[profile.release]` + `[profile.size]`（根 `Cargo.toml`）

```toml
[profile.release]
strip = true          # 去符号表；PDB 照常单独生成，不影响崩溃上报
lto = "thin"          # 跨 crate 内联 + 死代码消除，增量代价可接受
codegen-units = 1     # 更好内联，配合 LTO 压缩

[profile.size]
inherits = "release"
opt-level = "z"       # 体积优先，构建更慢，只用于打安装包
lto = true            # fat LTO
codegen-units = 1
strip = true
```

- 日常开发/CI 继续用 `release`：构建快，体积 8.92MB。
- 发版打安装包用 `cargo build --profile size -p <pkg>`：构建慢（约 7–10 分钟），体积 5.97MB。
- 刻意没开 `panic = "abort"`：会改变 panic 捕获语义，async 执行器相关代码有风险，省的几百 KB 不值得。

### 2.2 `image` 去掉 `avif`，加 `image-avif` 开关

- workspace 的 `image` 从默认特性改为显式 14 种格式，唯一拿掉的是 `avif`。它是最大单点：`image/avif` 会引入 ravif → rav1e 整套 **AV1 编码器**，而 UI 框架只做解码。
- 其他格式（png/jpeg/webp/gif/bmp/ico/tiff/pnm/dds/exr/hdr/qoi/tga/ff）全保留，零回归。
- `rgpui` 新增 `image-avif = ["image/avif"]` 特性，需要 AVIF 解码的应用显式开启即可加回；`Img::extensions()` 文档已注明。
- 验证：默认树里 `cargo tree -i rav1e` 为空；`cargo check -p rgpui --features image-avif` 通过。

### 2.3 `hdrhistogram` 改为可选依赖

- `input-latency-histogram = ["dep:hdrhistogram"]`，代码零改动（门控早就在）。
- 唯一开启该特性的 `examples/input_latency` 检查通过。

### 2.4 `windows` 特性表 52 → 37

- 审计方法：dump 全部 `use windows::` 块（`rgpui-windows/src` 17 个文件 + `rgpui/src` 4 个文件），映射到特性名，删除 15 个无引用项（`Foundation_Numerics`、`Globalization_DateTimeFormatting`、`Storage_Search/Streams`、`System_Threading`（WinRT 版）、`Imaging`、`WinSock`、`Security_Cryptography`、`Storage_FileSystem`、`Console`、`IO`、`Pipes`、`RestartManager`、`Variant`、`WinRT`）。
- 保留项里注意两个特例：`Fxc`/`Hlsl` 只在 `debug_assertions` 下编译 shader 用，但 `cargo test` 需要，必须保留；`accesskit_windows`、`muda` 等第三方 crate 自己拉的特性（如 `Accessibility`、`Variant`）不受我们裁剪影响，属正常统一。
- `rgpui` 直连的 `windows` 依赖同步去掉没用到的 `Win32_Security`、`Win32_System_Power`（全文 grep 确认无引用）。

### 2.5 `image` 去掉 dds/exr/ff/hdr/tga/qoi/tiff，加 opt-in 特性

- workspace 的 `image` 特性从 14 种格式精简为 7 种（bmp/gif/ico/jpeg/png/pnm/webp），拿掉的 7 种在桌面 UI 里极为罕见。
- `rgpui` 新增 7 个 opt-in 特性：`image-dds`、`image-exr`、`image-ff`、`image-hdr`、`image-tga`、`image-tiff`、`image-qoi`（对应 `image/dds` 等），需要时显式开启。
- `Img::extensions()` 改为动态构建，按 `#[cfg(feature = "image-xxx")]` 条件追加扩展名。
- 消除的重型依赖：exr 拉入的 `num-complex`/`zune-inflate`/`raw-cpuid`/`miniz_oxide`（第二份副本）等 ~8 个 crate，以及 tiff 拉入的 `fax`/`half`/`weezl`。

### 2.6 `uuid` 去掉 v5/v7

- workspace 的 `uuid` 从 `v4/v5/v7/serde` 精简为 `v4/serde`。
- `v5` 被 `rgpui-windows` 和 `rgpui-linux` 的 `generate_uuid()` 使用（生成确定性显示器 UUID），这两个 crate 单独启用 `uuid/v5` 特性。
- `v7` 全仓库无引用，直接删除。
- 消除 `sha1_smol` 依赖。

### 2.7 `serde_json` 去掉 `raw_value`

- 全仓库无 `RawValue` 引用，该 feature 完全未使用，直接删除。

### 2.8 `url` 去掉 IDNA（Unicode 国际化域名）

- `url` 从默认特性改为 `default-features = false, features = ["std"]`，移除 IDNA 支持。
- IDNA 拉入 `idna` → `idna_adapter` → `icu_normalizer`/`icu_properties` 等 ~15 个 crate（Unicode 数据表），桌面 UI 几乎不需要国际化域名解析。
- 消除的依赖链：`idna`/`idna_adapter`/`icu_normalizer`/`icu_normalizer_data`/`icu_properties`/`icu_properties_data`/`icu_collections`/`icu_provider`/`icu_locale_core`/`zerovec`/`zerotrie`/`litemap`/`yoke`/`zerofrom` 等。

### 2.9 `async-compression` bzip2 改为 opt-in

- workspace `async-compression` 移除 `bzip2` feature，保留 `gzip` + `futures-io`。
- `rgpui` 新增 `bzip2-decompress = ["async-compression/bzip2"]` 特性，需要解压 `.tar.bz2` 的应用显式开启。
- `github_download.rs` 中 `BzDecoder` 和 `extract_tar_bz2` 用 `#[cfg(feature = "bzip2-decompress")]` 门控，未启用时返回清晰错误。
- 消除 `libbz2-rs-sys`（完整 bzip2 C 库）。

### 2.10 `image` 去掉 tiff，加 `image-tiff` opt-in

- workspace `image` 移除 `tiff` feature，格式从 8 种精简为 7 种。
- `rgpui` 新增 `image-tiff = ["image/tiff"]` 特性。
- `Img::extensions()` 和 `platform.rs` 的 `to_image_data()` 中 `ImageFormat::Tiff` 分支均用 `#[cfg(feature = "image-tiff")]` 门控。
- 消除 `tiff` → `fax`/`half`/`weezl` 依赖链。

### 2.11 `log` 去掉 kv/serde 特性

- `log` 从 `features = ["kv_unstable_serde", "serde"]` 改为无额外 feature。
- `kv_unstable_serde` 拉入 `kv-log-macro`/`value-bag`/`value-bag-serde1`/`erased-serde`/`serde_fmt` 等结构化日志基础设施，全仓库零引用。

### 2.12 `serde_json_lenient` 去掉 `raw_value`

- 与 §2.7 同理，`serde_json_lenient` 的 `raw_value` feature 完全未使用，移除。

### 2.13 `postage` 移到 dev-dependencies

- `postage` 仅在 `test_context.rs`（已 behind `#[cfg(any(test, feature = "test-support"))]`）中使用。
- 从 `rgpui` 的 `[dependencies]` 移到 `[dev-dependencies]`，`test-support` feature 中加 `dep:postage`。
- 生产构建不再链接 `postage` 及其 `crossbeam-queue`/`futures` 重导出。

### 2.14 `async_zip` 去掉 `deflate64`

- `async_zip` 从 `features = ["deflate", "deflate64"]` 精简为 `features = ["deflate"]`。
- DEFLATE64 压缩在 zip 文件中极为罕见，标准 DEFLATE 覆盖 >99.9% 场景。

## 3. 当前体积构成（5.97MB 里是什么）

按"动它的代价"排序，剩下的全是真实链接的代码：

| 构成 | 说明 | 能动吗 |
|------|------|--------|
| D3D11/DirectWrite/DComp 渲染路径 | Windows GPU 渲染核心 | 不能，立身之本 |
| resvg + tiny-skia + fontdb | SVG 渲染 | 很难，SVG 是核心能力 |
| 字体栈（swash/skrifa/read-fonts） | 文本塑形/光栅化 | 不能 |
| image 剩余 7 种解码 | 图片解码 | 已精简，剩余为核心格式 |
| url（无 IDNA） | URL 解析 | 已去掉 IDNA，剩余为核心功能 |
| regex unicode 表 | `\p{Emoji}` 等需要 unicode 支持 | 裁了就错，不做 |
| async 全家桶（smol/async-std/tar/zip） | 命令执行、压缩包、调度 | 架构级依赖，见 §5.1 |
| backtrace + object/gimli | `TestScheduler` 帧级回溯 | 做不了，见 §5.2 |
| rayon / chrono / schemars | 并行/日期/JSON Schema | 小而散，不值得 |

## 4. 发版操作手册

```powershell
# 1. 打安装包体积构建
cargo build --profile size -p <your_app>

# 2. 体积复测（记录 .exe 即可，.pdb 不进包）
Get-ChildItem -LiteralPath "target\size" -Filter "<your_app>.exe" |
    Select-Object Name, Length
```

经验值：5.97MB 的 exe 经安装包工具（NSIS/WiX）的 LZMA 压缩后约 3–3.5MB。**觉得安装包大时，先看压缩选项，再看二进制**，前者往往更有效。

## 5. 将来的优化点

### 5.1 可做：async 解压栈 feature-gate（预计 200–400KB）

整个 async 压缩/解压栈（async-std/smol/async-tar/async-compression/async_zip/async-fs）可以 feature-gate 为 `downloader`，不过改动较大，需要 `github_download` 和 `archive` 模块整体走 cfg。

### 5.2 难做：`backtrace`（预计 300–600KB，明确不建议）

`TestScheduler` 是公开 API，`pending_traces: BTreeMap<TraceId, Backtrace>` 和 `exclude_wakers_from_trace` 用到了 `backtrace` crate 独有的帧级 API（`BacktraceFrame` 转换），`std::backtrace` 没有对等能力。用 `test-support` 门控会改变公开 API 形状。结论：保留，_async 诊断链_的代价。

### 5.3 难做：`schemars` feature-gate（预计 100–300KB）

`schemars` 的 `JsonSchema` derive 深度集成在 rgpui 类型系统中（100+ 处），feature-gate 需要在所有 derive 上加 `#[cfg(feature = "json-schema")]`，改动量大但收益可观。

### 5.4 不做（记录决策，免得后人重踩）

- `panic = "abort"`：省 unwind 表，但改变 panic 语义，async 执行器风险。
- `regex` 去 unicode：`\p{Emoji}` 等正则会直接错。
- `url` 完全移除：`Url` 在公开 API 里，动即破坏。
- `hdrhistogram` 结构体字段门控：已经做完了（本轮 §2.3）。
- UPX 加壳：省体积但触发杀软误报，且影响启动速度，安装包场景不如 LZMA 实在。

### 5.5 流程建议：CI 体积门禁

建议在 CI 里加一条（`hello_world`，`size` profile）：

```yaml
- run: cargo build --profile size -p hello_world
- run: |
    $size = (Get-Item target/size/hello_world.exe).Length
    if ($size -gt 8MB) { throw "体积回归：$size bytes" }
```

阈值按 5.97MB 现状上浮 30%（8MB），只防回归、不卡正常演进。注意 `size` 构建约 7–10 分钟，建议放独立 job，别塞进主矩阵里拖慢反馈。
