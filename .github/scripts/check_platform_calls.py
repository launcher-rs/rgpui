#!/usr/bin/env python3
"""Platform/PlatformWindow 自有方法必须有真实调用点。

背景（AGENTS.md）：`App::platform` 是私有字段，没有 App/Window 包装的能力
应用层一行都调不到。`set_keep_alive_without_windows`（存了没人读）与
`os_info`/权限查询（实现了没人调）两起缺陷都是接线缺失导致的。
本检查保证不再新增同类缺陷：trait 里加方法，必须同时让某处真实调用它。
App/Window 包装、core 内部调用、示例调用点、后端内部调用（如 Windows
`open_window` 登记 HWND 时读 `get_raw_handle`）都算；只有"定义行"不算
（trait 定义 + 各后端 `fn` 实现行）。存量例外见同目录 allowlist 文件。

判定口径（出现即证据，不做语义分析）：
- 扫描 `crates/` 与 `examples/` 的 `.rs`，去掉注释后找同名单词；
  匹配 `fn <name>(` / `fn <name><` / `fn <name>:` 的定义行跳过。
- `temp/`（vendored 参考）不扫，避免参考实现把真违规洗白。
- `on_*` 回调由 core 注册、core 内部调用（如 `window.rs` 经
  `platform_window.on_resize(..)`）同样算接线正常，不要求同名包装。
- 已知局限：只计数不辨读写（如只写不读的 setter 能过），语义级检查靠评审。

用法：`python3 .github/scripts/check_platform_calls.py`（仓库根），
有未放行的违规即非零退出，CI 直接跑它。
"""

import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
PLATFORM_RS = ROOT / "crates/rgpui/src/platform.rs"
ALLOWLIST = Path(__file__).resolve().parent / "platform_calls_allowlist.txt"

TRAITS = ("Platform", "PlatformWindow")

# 不扫描的路径片段：vendored 参考实现（会把真违规洗白）与构建产物。
EXCLUDED_PARTS = ("temp/", "target/")


def strip_line_comment(line: str) -> str:
    """去行注释（处理字符串内的 //，如 "https://"）。"""
    in_str = False
    escaped = False
    for i, ch in enumerate(line):
        if in_str:
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                in_str = False
        else:
            if ch == '"':
                in_str = True
            elif ch == "/" and i + 1 < len(line) and line[i + 1] == "/":
                return line[:i]
    return line


def trait_methods(path: Path) -> dict[str, int]:
    """解析 platform.rs，返回 {方法名: 定义行号}（仅 Platform/PlatformWindow）。"""
    text = path.read_text(encoding="utf-8")
    # 先去块注释，避免花括号计数错位。
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    lines = text.splitlines()
    methods: dict[str, int] = {}
    current: str | None = None
    depth = 0
    for lineno, raw in enumerate(lines, start=1):
        line = strip_line_comment(raw)
        if current is None:
            m = re.match(r"pub trait (\w+)", line)
            if m and m.group(1) in TRAITS:
                current = m.group(1)
                depth = line.count("{") - line.count("}")
            continue
        depth += line.count("{") - line.count("}")
        if depth <= 0:
            current = None
            continue
        m = re.match(r"    fn (\w+)\s*[\(:<]", line)
        if m:
            methods.setdefault(m.group(1), lineno)
    return methods


def load_allowlist() -> dict[str, str]:
    allow: dict[str, str] = {}
    if not ALLOWLIST.exists():
        return allow
    for line in ALLOWLIST.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        name, _, reason = line.partition("#")
        allow[name.strip()] = reason.strip()
    return allow


def iter_rs_files(base: Path):
    """遍历 .rs 文件，剪掉构建产物与 vendored 参考（rglob 会逛遍它们）。"""
    for dirpath, dirnames, filenames in os.walk(base):
        dirnames[:] = [
            d
            for d in dirnames
            if d not in ("target", "temp", "node_modules", ".git", "dist")
        ]
        for name in filenames:
            if name.endswith(".rs"):
                yield Path(dirpath) / name


def build_index(methods: set[str]):
    """一次读全量文件，返回 {relpath: (含方法名的非定义行数)}。

    只统计待查方法名（集合查找），大文件（>2MB 生成代码）跳过。
    """
    index: dict[str, dict[str, int]] = {}
    for base in (ROOT / "crates", ROOT / "examples"):
        if not base.exists():
            continue
        for path in iter_rs_files(base):
            posix = path.as_posix()
            if any(part in posix for part in EXCLUDED_PARTS):
                continue
            try:
                if path.stat().st_size > 2_000_000:
                    continue
                lines = path.read_text(encoding="utf-8").splitlines()
            except OSError:
                continue
            rel = path.relative_to(ROOT).as_posix()
            hits: dict[str, int] = {}
            for line in lines:
                code = strip_line_comment(line).strip()
                if not code:
                    continue
                for word in set(re.findall(r"[A-Za-z_]\w*", code)):
                    if word not in methods:
                        continue
                    # 定义行（trait 定义 + 各后端实现）不算调用点。
                    if re.match(rf"(pub(\(crate\))? )?fn {re.escape(word)}\s*[\(:<]", code):
                        continue
                    hits[word] = hits.get(word, 0) + 1
            if hits:
                index[rel] = hits
    return index


def main() -> int:
    methods = trait_methods(PLATFORM_RS)
    allow = load_allowlist()
    print(f"检查 {len(methods)} 个 Platform/PlatformWindow 自有方法…")
    index = build_index(set(methods))
    violations: list[str] = []
    for name in sorted(methods):
        if any(hits.get(name, 0) > 0 for hits in index.values()):
            continue
        if name in allow:
            print(f"  放行 {name}（{allow[name]}）")
            continue
        where = f"（platform.rs:{methods[name]}）"
        violations.append(f"  违规 {name}{where}：缺真实调用点")
    if violations:
        print("\n".join(violations))
        print(f"\n{len(violations)} 项违规：补调用点（含 App/Window 包装），或在 {ALLOWLIST.name} 中注明理由放行。")
        return 1
    print("全部通过。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
