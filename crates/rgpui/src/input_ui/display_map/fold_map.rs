/// FoldMap：折叠投影层（Wrap 行 → Display 行）。
///
/// 本模块通过以下方式管理代码折叠：
/// - 过滤掉属于折叠区域的 wrap 行
/// - 维护双向映射：wrap_row ↔ display_row
/// - 处理折叠状态变化并重建投影
use super::folding::FoldRange;
use super::wrap_map::WrapMap;

/// FoldMap 通过隐藏折叠区域将 wrap 行投影到 display 行。
pub struct FoldMap {
    /// 映射：display_row → wrap_row
    /// index = display_row, value = 实际 wrap_row
    visible_wrap_rows: Vec<usize>,

    /// 反向映射：wrap_row → display_row
    /// index = wrap_row, value = 可见时为 Some(display_row)，折叠时为 None
    wrap_row_to_display_row: Vec<Option<usize>>,

    /// 候选折叠范围（来自 tree-sitter/LSP）
    /// 按 start_line 排序，start_line 唯一
    candidates: Vec<FoldRange>,

    /// 当前折叠的范围
    /// candidates 的子集，按 start_line 排序
    folded: Vec<FoldRange>,

    /// 指示折叠投影是否需要重建的标记
    /// 用于懒评估以避免每次文本变更都重建
    needs_rebuild: bool,

    /// 上次重建的缓存 wrap_row_count
    /// 用于检测 WrapMap 是否变更以及是否需要重建
    cached_wrap_row_count: usize,
}

impl FoldMap {
    /// 创建新的 FoldMap。
    pub fn new() -> Self {
        Self {
            visible_wrap_rows: Vec::new(),
            wrap_row_to_display_row: Vec::new(),
            candidates: Vec::new(),
            folded: Vec::new(),
            needs_rebuild: true,
            cached_wrap_row_count: 0,
        }
    }

    /// 更新缓存的 wrap_row_count，不做完整重建。
    /// 在没有活动折叠时使用（假设恒等映射）。
    pub(super) fn mark_dirty_with_wrap_count(&mut self, wrap_row_count: usize) {
        self.needs_rebuild = true;
        self.cached_wrap_row_count = wrap_row_count;
    }

    /// 获取可见 display 行总数。
    pub fn display_row_count(&self) -> usize {
        if self.folded.is_empty() {
            return self.cached_wrap_row_count;
        }
        self.visible_wrap_rows.len()
    }

    /// 将 wrap_row 转换为 display_row。
    /// 如果 wrap_row 被折叠隐藏则返回 None。
    pub fn wrap_row_to_display_row(&self, wrap_row: usize) -> Option<usize> {
        if self.folded.is_empty() {
            return if wrap_row < self.cached_wrap_row_count {
                Some(wrap_row)
            } else {
                None
            };
        }
        self.wrap_row_to_display_row
            .get(wrap_row)
            .copied()
            .flatten()
    }

    /// 将 display_row 转换为 wrap_row。
    pub fn display_row_to_wrap_row(&self, display_row: usize) -> Option<usize> {
        if self.folded.is_empty() {
            return if display_row < self.cached_wrap_row_count {
                Some(display_row)
            } else {
                None
            };
        }
        self.visible_wrap_rows.get(display_row).copied()
    }

    /// 为给定 wrap_row 找到最近的可见 display_row。
    pub fn nearest_visible_display_row(&self, wrap_row: usize) -> usize {
        if self.folded.is_empty() {
            return wrap_row.min(self.cached_wrap_row_count.saturating_sub(1));
        }

        if let Some(dr) = self.wrap_row_to_display_row(wrap_row) {
            return dr;
        }

        match self.visible_wrap_rows.binary_search(&wrap_row) {
            Ok(idx) => idx,
            Err(insert_pos) => insert_pos.saturating_sub(1),
        }
    }

    /// 设置折叠候选（来自 tree-sitter/LSP），全量替换。
    pub fn set_candidates(&mut self, mut candidates: Vec<FoldRange>) {
        // 按 start_line 排序和去重
        candidates.sort_by_key(|r| r.start_line);
        candidates.dedup_by_key(|r| r.start_line);
        self.candidates = candidates;

        // 移除不再在候选中的折叠范围
        self.folded.retain(|fold| {
            self.candidates
                .iter()
                .any(|c| c.start_line == fold.start_line)
        });
    }

    /// 将从编辑区域提取的新候选合并到现有候选。
    ///
    /// 将 [edit_start_line, edit_end_line] 范围内的候选替换为 `new_candidates`，
    /// 保持编辑范围之外的候选不变。
    pub fn merge_candidates_for_edit(
        &mut self,
        edit_start_line: usize,
        edit_end_line: usize,
        new_candidates: Vec<FoldRange>,
    ) {
        // 移除编辑范围内的旧候选（已由 adjust_folds_for_edit 完成）
        // 但以防 adjust 未被调用或范围不同，再次执行
        self.candidates
            .retain(|c| c.start_line < edit_start_line || c.start_line > edit_end_line);

        // 添加新候选
        self.candidates.extend(new_candidates);
        self.candidates.sort_by_key(|r| r.start_line);
        self.candidates.dedup_by_key(|r| r.start_line);
    }

    /// 在给定 start_line 处设置折叠（必须在候选中）。
    pub fn set_folded(&mut self, start_line: usize, folded: bool) {
        if folded {
            // 找到此 start_line 的候选范围
            if let Some(candidate) = self.candidates.iter().find(|c| c.start_line == start_line) {
                // 如果尚未存在则添加到折叠
                if !self.folded.iter().any(|f| f.start_line == start_line) {
                    self.folded.push(*candidate);
                    self.folded.sort_by_key(|r| r.start_line);
                    self.needs_rebuild = true;
                }
            }
        } else {
            // 从折叠中移除
            self.folded.retain(|f| f.start_line != start_line);
            self.needs_rebuild = true;
        }
    }

    /// 切换给定 start_line 处的折叠。
    pub fn toggle_fold(&mut self, start_line: usize) {
        let is_folded = self.is_folded_at(start_line);
        self.set_folded(start_line, !is_folded);
    }

    /// 检查一行当前是否折叠。
    pub fn is_folded_at(&self, start_line: usize) -> bool {
        self.folded.iter().any(|f| f.start_line == start_line)
    }

    /// 检查一行是否为折叠候选。
    pub fn is_fold_candidate(&self, start_line: usize) -> bool {
        self.candidates.iter().any(|c| c.start_line == start_line)
    }

    /// 获取所有折叠候选。
    #[inline]
    pub fn fold_candidates(&self) -> &[FoldRange] {
        &self.candidates
    }

    /// 获取所有当前折叠的范围。
    #[inline]
    pub fn folded_ranges(&self) -> &[FoldRange] {
        &self.folded
    }

    /// 清除所有折叠。
    #[inline]
    pub fn clear_folds(&mut self) {
        self.folded.clear();
    }

    /// 在文本编辑后调整折叠和候选。
    ///
    /// - 与编辑行范围重叠的折叠/候选被移除
    /// - 编辑之后的折叠/候选按 line_delta 平移
    ///
    /// 这避免了每次按键都进行昂贵的完整树遍历。
    pub fn adjust_folds_for_edit(
        &mut self,
        edit_start_line: usize,
        edit_end_line: usize,
        line_delta: isize,
    ) {
        // 调整折叠范围
        if !self.folded.is_empty() {
            self.folded.retain(|fold| {
                !(fold.start_line <= edit_end_line && fold.end_line >= edit_start_line)
            });

            if line_delta != 0 {
                for fold in &mut self.folded {
                    if fold.start_line > edit_end_line {
                        fold.start_line = (fold.start_line as isize + line_delta).max(0) as usize;
                        fold.end_line = (fold.end_line as isize + line_delta).max(0) as usize;
                    }
                }
            }
        }

        // 同样调整候选
        if !self.candidates.is_empty() {
            self.candidates
                .retain(|c| !(c.start_line <= edit_end_line && c.end_line >= edit_start_line));

            if line_delta != 0 {
                for c in &mut self.candidates {
                    if c.start_line > edit_end_line {
                        c.start_line = (c.start_line as isize + line_delta).max(0) as usize;
                        c.end_line = (c.end_line as isize + line_delta).max(0) as usize;
                    }
                }
            }
        }

        self.needs_rebuild = true;
    }

    /// 在 wrap map 或折叠状态变化后重建折叠映射。
    ///
    /// 这是将 wrap 行投影到 display 行的核心算法。
    pub fn rebuild(&mut self, wrap_map: &WrapMap) {
        let wrap_row_count = wrap_map.wrap_row_count();

        // 性能优化：如果没有任何变化则跳过重建
        if !self.needs_rebuild && wrap_row_count == self.cached_wrap_row_count {
            return;
        }

        self.cached_wrap_row_count = wrap_row_count;

        self.visible_wrap_rows.clear();
        self.wrap_row_to_display_row = vec![None; wrap_row_count];

        if self.folded.is_empty() {
            // 快速路径：无折叠，所有 wrap 行可见
            self.visible_wrap_rows = (0..wrap_row_count).collect();
            for (display_row, &wrap_row) in self.visible_wrap_rows.iter().enumerate() {
                self.wrap_row_to_display_row[wrap_row] = Some(display_row);
            }
            self.needs_rebuild = false;
            return;
        }

        // 从折叠 buffer 行构建隐藏 wrap 行范围集合
        let mut hidden_ranges = Vec::new();
        for fold in &self.folded {
            // 隐藏从 (start_line + 1) 到 (end_line - 1)（包含）的 wrap 行
            // 折叠的第一行和最后一行保持可见
            let hide_start_line = fold.start_line + 1;
            let hide_end_line = fold.end_line.saturating_sub(1);

            if hide_start_line > hide_end_line {
                continue; // 无可隐藏的中间行（起始和结束之间 0 或 1 行）
            }

            // 获取隐藏 buffer 行的 wrap 行范围
            let start_wrap_row = wrap_map.buffer_line_to_first_wrap_row(hide_start_line);
            let end_wrap_row = if hide_end_line + 1 < wrap_map.buffer_line_count() {
                wrap_map.buffer_line_to_first_wrap_row(hide_end_line + 1)
            } else {
                wrap_row_count
            };

            if start_wrap_row < end_wrap_row {
                hidden_ranges.push(start_wrap_row..end_wrap_row);
            }
        }

        // 合并重叠的隐藏范围
        hidden_ranges.sort_by_key(|r| r.start);
        let mut merged_hidden = Vec::new();
        for range in hidden_ranges {
            if let Some(last) = merged_hidden.last_mut() {
                if range.start <= *last {
                    // 重叠或相邻，合并
                    *last = (*last).max(range.end);
                } else {
                    merged_hidden.push(range.start);
                    merged_hidden.push(range.end);
                }
            } else {
                merged_hidden.push(range.start);
                merged_hidden.push(range.end);
            }
        }

        // 扫描所有 wrap 行并过滤掉隐藏的行
        let mut display_row = 0;
        let mut hidden_iter = merged_hidden.chunks_exact(2);
        let mut current_hidden = hidden_iter.next();

        for wrap_row in 0..wrap_row_count {
            // 检查 wrap_row 是否在当前隐藏范围内
            let is_hidden = if let Some(&[start, end]) = current_hidden {
                if wrap_row >= end {
                    current_hidden = hidden_iter.next();
                    if let Some(&[new_start, new_end]) = current_hidden {
                        wrap_row >= new_start && wrap_row < new_end
                    } else {
                        false
                    }
                } else {
                    wrap_row >= start && wrap_row < end
                }
            } else {
                false
            };

            if !is_hidden {
                self.visible_wrap_rows.push(wrap_row);
                self.wrap_row_to_display_row[wrap_row] = Some(display_row);
                display_row += 1;
            }
        }

        self.needs_rebuild = false;
    }
}
