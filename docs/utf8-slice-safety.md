# UTF-8 字符串切片安全：中文 panic 事故与规范

> 状态：现行规范。面向所有接触 `str`/`String`/`SharedString` 切片的代码。
> 起因：`&code[..16]` 按字节截断复制按钮 ID，遇到中文（如 `运` 占 3 字节，`14..17`）直接 panic。
> 本文档把这一次排查定为长期经验：讲清根因、列出本次修掉的全部同类 bug、给出以后写代码必须遵守的写法。

## 1. 根因一句话

`str` 是**保证合法 UTF-8 的字节序列**，而 `&s[a..b]` 的下标是**字节下标**。

* ASCII（`a`/`#`/`-`/`|`` 等）恰好 1 字节，所以按字节切和按字符切结果一致，bug 被掩盖。
* 中文通常 3 字节，emoji 通常 4 字节。`&"你好呀"[..2]` 是想取“前 2 个字节”，
  正好把 `你`（`E4 BD A0`，字节 `0..3`）从中间切断。
* Rust 宁可 panic，也不会交出一个非法的 `str`。这不是中文特殊，是设计如此。

字节布局示意（`你好呀`，每字 3 字节）：

```text
字节：0 1 2 | 3 4 5 | 6 7 8
字符：  你   |  好   |  呀
[..2] 切在这里 ──┘  ← 非字符边界，panic
[..3] 切在这里 ────┘ ← 边界，得到 "你"
```

## 2. 三个层次不要混用

| 层次 | 含义 | 示例 |
|------|------|------|
| 字节 byte | UTF-8 存储单位 | `s.len()`、`as_bytes()[i]` |
| 字符 char | Unicode 码点，Rust 的 `char` | `'运'` 1 个 char、3 字节 |
| 字形 grapheme | 人眼看到的一个字 | `👨‍👩‍👧‍👦` 1 个字形、多个 char；`e + ◌́` 1 个字形、2 个 char |

推论：

* 编辑器光标、排版、装饰区间用的是**字节偏移**（`Rope`/`ShapedLine` 全是字节坐标），
  不能用 `chars().take(n)` 逐个重数——那是 `O(n)` 还会拆散字形。
* 用户可见的“第 N 个字”才考虑 `unicode-segmentation` 的 grapheme。
* 字节坐标 + 字符边界检查，才是本项目的标准组合。

## 3. 选型表（直接抄）

| 需求 | 写法 |
|------|------|
| 已知是 ASCII 或已知边界（如 `find("#")` 返回的位置 `+1`） | `&s[a..b]` 可用，但必须注释为什么安全 |
| 不确定边界，允许拿不到 | `s.get(a..b)` / `s.get(..n)`，`None` 即非边界 |
| 任意字节坐标转子串，永不崩（截断/日志/ID 前缀/行列转偏移） | `SafeStrSlice`（见 §5），`start` 向下吸附、`end` 向下吸附 |
| 按字符推进扫描（如搜索、标黄、分词） | `char_indices` / `chars().next().len_utf8()` 推进，禁止 `+1` 字节步进 |
| 大小写不敏感匹配 | 逐字符 `to_lowercase` 比较并累加原串字节数（见 §4 反模式 2），禁止整串 `to_lowercase` 后复用偏移 |
| 真正的“前 N 个用户可见字” | `unicode-segmentation` grapheme |

## 4. 本次挖出的反模式（全部已修，仅供对照）

### 反模式 1：`col += pos + 1` 按字节步进

```rust
// 旧代码（source_map::search）：query 为中文或行内有中文时，
// col 落进多字节字符中间，下一次 line[col..] 直接 panic。
while let Some(pos) = line[col..].find(query) {
    col += pos + 1;
}
```

正模式：按查询**首字符字节数**推进（重叠语义保留），`col` 恒为边界，
外加吸附兜底；空查询直接返回：

```rust
let advance = query.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
while col <= line.len() {
    let rest = line.get(col..).unwrap_or("");
    let Some(pos) = rest.find(query) else { break };
    col += pos + advance;
    while col < line.len() && !line.is_char_boundary(col) { col += 1; }
}
```

### 反模式 2：整串 `to_lowercase()` 后复用字节偏移

```rust
// 旧代码（label::highlight_ranges）：小写可能改变字节数（如 `İ` → `i̇`），
// 小写串的偏移切回原串必然错位；再叠加按字节 +1 步进，中文必崩。
let lower = full.to_lowercase();
while let Some(pos) = lower[start..].find(&query_lower) {
    ranges.push(start + pos..start + pos + query.len()); // 长度也是错的
    start += 1;
}
```

正模式：查询预小写为 `Vec<char>`，原串逐字符小写比较，
返回**原串字节数**（`search_panel::literal_insensitive_prefix_len` 同款，
`label` 内为 `insensitive_prefix_len`）：

```rust
fn insensitive_prefix_len(rest: &str, query_lower: &[char]) -> Option<usize> {
    let mut chars = rest.chars();
    let mut consumed = 0;
    let mut qi = 0;
    while qi < query_lower.len() {
        let c = chars.next()?;
        consumed += c.len_utf8();
        for lc in c.to_lowercase() {
            if qi >= query_lower.len() || lc != query_lower[qi] { return None; }
            qi += 1;
        }
    }
    Some(consumed)
}
```

### 反模式 3：想当然的直接切片

```rust
let id = format!("code-block-copy-{}", &code[..code.len().min(16)]); // 中文 panic
line[indent + prefix.len()..].starts_with(' ')                        // prefix 含多字节即崩
source[..start]                       // tree-sitter 字节偏移未必是边界
self.edit_value[0..range.start]       // 输入法中间态未必是边界
```

正模式：`get` + `floor_char_boundary`，或走 `SafeStrSlice`。

## 5. 新规：`SafeStrSlice`（`crates/rgpui/src/util/mod.rs`）

```rust
pub trait SafeStrSlice {
    fn safe_get(&self, range: Range<usize>) -> Option<&str>;
    fn safe_slice_floor(&self, start: usize, end: usize) -> &str;
    fn safe_suffix_from(&self, start: usize) -> &str;
    fn safe_prefix_until(&self, end: usize) -> &str;
    /// 掐头去尾：返回 `(head, tail)`，中间丢弃；`head.len()` 即吸附后的 `start`。
    fn safe_head_tail(&self, start: usize, end: usize) -> (&str, &str);
}
```

根导出为 `rgpui::SafeStrSlice`，依赖 `rgpui` 的 crate（含示例）都可直接用，
禁止各 crate 本地手写 `while !is_char_boundary` 循环。

规则：

1. 新增代码**禁止直接 `&s[a..b]`**（`a`/`b` 为变量时），一律经 `SafeStrSlice` 或 `get`。
2. 例外仅两种，且必须写注释说明为什么安全：纯 ASCII 步进（如 `#` 计数、
   `0x` 前缀、`---` 箭头），或下标来自 `find`/`char_indices` 且已证明是边界。
3. 外部输入（输入法、tree-sitter、LSP、剪贴板、网络）进来的偏移，
   在切片前必须过一次 `floor_char_boundary`。
4. `+1` 字节步进默认视为 bug；想保留重叠匹配语义时，按首字符 `len_utf8()` 推进。

## 6. 本次修复清单（2026-09）

### 第一轮：`rgpui` + `rgpui-markdown`

| 位置 | 问题 | 修法 |
|------|------|------|
| `rgpui-markdown::code_block::code_id_prefix` | `&code[..16]` 切中文 panic | 已在此前修复为边界吸附，本轮改为复用 `SafeStrSlice::safe_prefix_until(16)`，回归用例保留 |
| `rgpui-markdown::code_block::tokenize` | 字节扫描可能切中文 | 全分支字符边界对齐 + 防御网 |
| `rgpui::source_map::SourceMap::search` | `col += pos + 1` | §4 反模式 1 |
| `rgpui::elements::label::highlight_ranges` | `to_lowercase` 偏移漂移 + `+1` | §4 反模式 2，逐字符匹配 |
| `rgpui::components::search_panel` | 同类问题 | 已为逐字符实现，本次仅对照确认 |
| `rgpui::input_ui::editor::line_ops` | `line[indent..]` 直接切片 | `get` + `starts_with` |
| `rgpui::highlight::tree_sitter::document_symbols` | `source[..start]` 未必边界 | `floor_char_boundary` + `get` |
| `rgpui::text_system::line::ShapedLine::split_at` | 调用方可能传非边界 | 入口 `floor` 吸附 |
| `rgpui::components::inline_edit` | 输入法 range 未必边界 | `floor` + `get` 拼接 |
| `rgpui::util::SafeStrSlice` | — | 新增统一工具 |

### 第二轮：workspace 其他 crate（含示例）

| 位置 | 结论 |
|------|------|
| `rgpui::theme::color::parse_hex`、`rgpui-term::parse_hex_color` | **修**：长度门控后仍可能切到非 ASCII，增加 `is_ascii()` 提前拒绝（否则返回 `Err`/`None` 之前先 panic） |
| `rgpui-wgpu::compute_run_spans`、`shape_segment` | **加固**：`run_offset/run_end`、`range` 入口 `floor_char_boundary` 吸附（含字形 `index` 重基配对修正）；正常对齐时与原逻辑等价 |
| `examples/input`、`examples/view_example` 的 `text_for_range` / `replace_*` | **修**：与核心 `inline_edit` 同款 `floor` + `get` 拼接（含 `marked_range`/`selected_range` 跟随修正） |
| `rgpui-term::strip_prompt`（`trimmed[pos+1..]`） | 安全：`pos` 来自 ASCII 分隔符的 `rfind`/`find`，`+1` 恒为边界 |
| `rgpui-macos::StringIndexConverter`（`text[utf8_ix..]`） | 安全：`utf8_ix` 从 0 起按 `char_indices` 累加，恒为边界 |
| `rgpui-windows::direct_write::slice_at_char_boundary` | 已有吸附兜底，本次仅对照确认 |
| `v1_1_showcase` / `rgpui_story::v1_1_features` 的 `remaining[..start]` / `&remaining[start+2..]` | 安全：`start/end` 来自 `"**"`/`'*'`（ASCII）的 `find`，`+1/+2` 恒为边界 |
| `desktop_pet_3d` 的 `anim_id[5..]` | 安全：`starts_with("anim_")` 门控，`5` 为 ASCII 前缀边界 |
| `command_palette` / `combobox` / `rgpui_term_basic` 的 `to_lowercase().contains/starts_with` | 安全：自有小写串内比较，不把偏移用回原串（与 §4 反模式 2 的区别） |
| `img` 的 `body.truncate(first_line.len())` | 安全：`first_line` 是 `body` 的行前缀 + `trim_end`，恒为边界 |
| `rel_path::pop` 的 `truncate(rfind('/'))` | 安全：`/` 为 ASCII，`rfind` 返回边界 |
| `rgpui-windows::build.rs` 的 header 解析 | 安全：构建期 + ASCII 模式的 `find` 链 |
| `clipboard` / `x11/window` 的 `[0..4]` 等 | 安全：`u8`/`u16` 数组索引，非 `str` 切片 |

### 第三轮：统一复用 `rgpui::SafeStrSlice`（消除各 crate 手写循环）

| 位置 | 动作 |
|------|------|
| `rgpui::inline_edit` 的两处替换拼接 | `floor` + `get` 手写版改为 `safe_head_tail`（`head.len()` 即吸附后的 `start`） |
| `examples/input` 的 `text_for_range` / 两处替换 | 改为 `safe_slice_floor` / `safe_head_tail` |
| `examples/view_example` 的 `text_for_range` / 替换 / 外部写入钳制 | 改为 `safe_slice_floor` / `safe_head_tail` / `floor_char_boundary`（原 `while !is_char_boundary` 循环删除） |
| `rgpui-wgpu`、`direct_write` 的 `floor_char_boundary` | 保留：std 标准工具即规范写法，无手写循环，无需改 |

经排查确认安全的（模式成立，附理由）：`block_render` 的 `#` 计数切片（ASCII 步进）、
`mermaid` 的 `find`/`trim` 链（`find` 返回边界，`+=` 按 `len_utf8` 或 ASCII 长度）、
`util::truncate*`（`char_indices` 出下标）、`line_wrapper`（`char_indices` 出
`truncate_ix` + `ceil_char_boundary`）、`bracket_match`（`floor`/`ceil` 窗口）、
`marked_text`（`match_indices` 出边界）、`theme::color` 的 hex 切片（ASCII 且有长度门控）。

## 7. 自查清单（提交前）

1. `rg` 搜 `\[[^]]*\.\.`：每个命中要么是 `Vec`/`数组` 索引，要么能说清字符边界理由，否则改掉。
2. 搜 `+ 1` 步进：凡是作用在 `str` 字节下标上的，改按 `len_utf8()` 或边界吸附。
3. 搜 `to_lowercase()`：整串小写后**禁止**把偏移用回原串。
4. 新增中文单测：截断、分词、搜索、高亮至少覆盖 `运`（3 字节）、emoji（4 字节）、
   空串、边界恰好 `len()` 四种情况。
5. `cargo check --workspace` + 相关包 `clippy -D warnings` + `cargo fmt --all`。
