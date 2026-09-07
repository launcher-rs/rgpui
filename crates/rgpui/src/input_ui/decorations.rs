use rgpui::{App, Context, HighlightStyle, WeakEntity};
use ropey::Rope;
use std::ops::Range;

use super::{InputState, RopeExt as _};

/// 装饰集合的 ID。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TextDecorationCollectionId(usize);

/// 文本装饰，包含一个范围和样式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDecoration {
    /// 装饰的范围（UTF-8 字节偏移）。
    pub range: Range<usize>,
    /// 装饰的样式。
    pub style: HighlightStyle,
}

impl TextDecoration {
    /// 创建新的文本装饰。
    pub fn new(range: Range<usize>, style: HighlightStyle) -> Self {
        Self { range, style }
    }
}

/// 装饰集合，允许独立添加/清除一组装饰。
#[derive(Clone)]
pub struct TextDecorationCollection {
    state: WeakEntity<InputState>,
    id: TextDecorationCollectionId,
}

impl TextDecorationCollection {
    /// 在已持有 state 可变借用时清空集合内容（见 [`Self::set_in_place`]）。
    pub(super) fn clear_in_place(&self, decorations: &mut DecorationCollections) -> bool {
        decorations.set(self.id, Vec::new())
    }

    /// 在已持有 state 可变借用时替换集合内容。
    ///
    /// [`Self::set`] 内部走 `entity.update`，在 `InputState` 方法内（已借用中）
    /// 调用会重入 panic；此方法直接写存储，由调用方负责 `normalize` + `notify`。
    pub(super) fn set_in_place(
        &self,
        decorations: &mut DecorationCollections,
        decorations_new: Vec<TextDecoration>,
    ) -> bool {
        decorations.set(self.id, decorations_new)
    }

    /// 将此集合中的装饰替换为给定装饰。
    ///
    /// 对应 Monaco 的
    /// [`IEditorDecorationsCollection.set`](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.IEditorDecorationsCollection.html#set)。
    pub fn set(&self, decorations: Vec<TextDecoration>, cx: &mut App) {
        let _ = self.state.update(cx, |state, cx| {
            let decorations = normalize(&state.text, decorations);
            if state.decorations.set(self.id, decorations) {
                cx.notify();
            }
        });
    }

    /// 向此集合追加装饰。
    ///
    /// 对应 Monaco 的
    /// [`IEditorDecorationsCollection.append`](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.IEditorDecorationsCollection.html#append)。
    pub fn append(&self, decorations: Vec<TextDecoration>, cx: &mut App) {
        let _ = self.state.update(cx, |state, cx| {
            let decorations = normalize(&state.text, decorations);
            if state.decorations.append(self.id, decorations) {
                cx.notify();
            }
        });
    }

    /// 清除此集合中的所有装饰。
    ///
    /// 对应 Monaco 的
    /// [`IEditorDecorationsCollection.clear`](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.IEditorDecorationsCollection.html#clear)。
    pub fn clear(&self, cx: &mut App) {
        self.set(Vec::new(), cx);
    }

    /// 返回此集合中的 UTF-8 字节范围。
    ///
    /// 对应 Monaco 的
    /// [`IEditorDecorationsCollection.getRanges`](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.IEditorDecorationsCollection.html#getRanges)。
    pub fn get_ranges(&self, cx: &App) -> Vec<Range<usize>> {
        self.state
            .read_with(cx, |state, _| {
                state
                    .decorations
                    .get(self.id)
                    .unwrap_or_default()
                    .iter()
                    .map(|decoration| decoration.range.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// 装饰集合的存储，按创建顺序分层。
#[derive(Default)]
pub(super) struct DecorationCollections {
    entries: Vec<(TextDecorationCollectionId, Vec<TextDecoration>)>,
}

impl DecorationCollections {
    fn create(&mut self, decorations: Vec<TextDecoration>) -> TextDecorationCollectionId {
        let id = TextDecorationCollectionId(self.entries.len());
        self.entries.push((id, decorations));
        id
    }

    fn set(&mut self, id: TextDecorationCollectionId, decorations: Vec<TextDecoration>) -> bool {
        let Some((_, current)) = self
            .entries
            .iter_mut()
            .find(|(entry_id, _)| *entry_id == id)
        else {
            return false;
        };
        *current = decorations;
        true
    }

    fn append(&mut self, id: TextDecorationCollectionId, decorations: Vec<TextDecoration>) -> bool {
        let Some((_, current)) = self
            .entries
            .iter_mut()
            .find(|(entry_id, _)| *entry_id == id)
        else {
            return false;
        };
        current.extend(decorations);
        true
    }

    fn get(&self, id: TextDecorationCollectionId) -> Option<&[TextDecoration]> {
        self.entries
            .iter()
            .find(|(entry_id, _)| *entry_id == id)
            .map(|(_, decorations)| decorations.as_slice())
    }

    pub(super) fn adjust_for_edit(&mut self, edited_range: &Range<usize>, inserted_len: usize) {
        for (_, decorations) in &mut self.entries {
            decorations.retain_mut(|decoration| {
                decoration.range =
                    adjust_range_for_edit(&decoration.range, edited_range, inserted_len);
                !decoration.range.is_empty()
            });
        }
    }

    pub(super) fn clear(&mut self) {
        for (_, decorations) in &mut self.entries {
            decorations.clear();
        }
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &[TextDecoration]> {
        self.entries
            .iter()
            .map(|(_, decorations)| decorations.as_slice())
    }
}

fn adjust_range_for_edit(
    range: &Range<usize>,
    edited_range: &Range<usize>,
    inserted_len: usize,
) -> Range<usize> {
    let removed_len = edited_range.end.saturating_sub(edited_range.start);
    let shift = |offset: usize| {
        if inserted_len >= removed_len {
            offset.saturating_add(inserted_len - removed_len)
        } else {
            offset.saturating_sub(removed_len - inserted_len)
        }
    };

    if edited_range.is_empty() {
        let start = if range.start < edited_range.start {
            range.start
        } else {
            shift(range.start)
        };
        let end = if range.end <= edited_range.start {
            range.end
        } else {
            shift(range.end)
        };
        return start..end;
    }

    let inserted_end = edited_range.start + inserted_len;
    let start = if range.start <= edited_range.start {
        range.start
    } else if range.start >= edited_range.end {
        shift(range.start)
    } else {
        edited_range.start
    };
    let end = if range.end <= edited_range.start {
        range.end
    } else if range.end >= edited_range.end {
        shift(range.end)
    } else {
        inserted_end
    };
    start..end
}

/// 规范化装饰范围：裁剪到文本内、去空区间，并吸附到字符边界。
///
/// 布局管线按字节切分 runs，范围端点落在多字节字符内部会直接 panic，
/// 因此这里是最后防线（stale 范围、IME 合成中的中间状态都经此兜底）。
pub(super) fn normalize(text: &Rope, decorations: Vec<TextDecoration>) -> Vec<TextDecoration> {
    decorations
        .into_iter()
        .filter_map(|decoration| {
            let range = text.clip_offset(decoration.range.start, rgpui::sum_tree::Bias::Left)
                ..text.clip_offset(decoration.range.end, rgpui::sum_tree::Bias::Right);
            let range = text.floor_char_boundary(range.start)
                ..text.ceil_char_boundary(range.end).min(text.len());
            (!range.is_empty()).then_some(TextDecoration {
                range,
                style: decoration.style,
            })
        })
        .collect()
}

impl InputState {
    /// 创建一个独立管理的文本装饰集合。
    ///
    /// 遵循 Monaco 的
    /// [`createDecorationsCollection`](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.ICodeEditor.html#createDecorationsCollection)
    /// 所有权模型。范围使用 [`Self::value`] 的 UTF-8 字节偏移。
    ///
    /// 装饰范围跟随文本编辑而调整，编辑后无需重新设置。在范围边界处插入不会
    /// 扩展范围，与 Monaco 的
    /// [`NeverGrowsWhenTypingAtEdges`](https://microsoft.github.io/monaco-editor/typedoc/enums/editor_editor_api.editor.TrackedRangeStickiness.html#NeverGrowsWhenTypingAtEdges)
    /// 行为一致。输入处于掩码状态时不渲染装饰。
    ///
    /// 集合按创建顺序分层；当重叠装饰设置相同的 [`HighlightStyle`] 属性时，
    /// 先创建的集合优先。调用者应避免在同一集合内产生冲突的重叠。
    ///
    /// 搜索场景 recipe（全部匹配标黄 + 当前匹配用选区）：
    ///
    /// ```ignore
    /// use rgpui::input_ui::{TextDecoration, TextDecorationCollection};
    ///
    /// // 每个匹配一个装饰，背景标黄；装饰层级低于选区，当前匹配仍可用
    /// // set_selected_range 高亮，不会互相覆盖。
    /// let collection: TextDecorationCollection = state.create_decorations_collection(
    ///     matches
    ///         .iter()
    ///         .map(|range| TextDecoration::new(range.clone(), HighlightStyle {
    ///             background_color: Some(Hsla::yellow()),
    ///             ..Default::default()
    ///         }))
    ///         .collect(),
    ///     cx,
    /// );
    /// // 查询变化时刷新：collection.set(new_decorations, cx);
    /// // 不再需要时：collection.clear(cx);
    /// ```
    pub fn create_decorations_collection(
        &mut self,
        decorations: Vec<TextDecoration>,
        cx: &mut Context<Self>,
    ) -> TextDecorationCollection {
        let decorations = normalize(&self.text, decorations);
        let id = self.decorations.create(decorations);
        cx.notify();
        TextDecorationCollection {
            state: cx.entity().downgrade(),
            id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgpui::FontWeight;

    #[test]
    fn collections_are_independent_and_ranges_are_clipped() {
        let text = Rope::from("héllo");
        let first_style = HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        };
        let second_style = HighlightStyle {
            background_color: Some(rgpui::red()),
            ..Default::default()
        };
        let mut collections = DecorationCollections::default();

        let first = collections.create(normalize(
            &text,
            vec![TextDecoration::new(2..4, first_style)],
        ));
        let second = collections.create(normalize(
            &text,
            vec![TextDecoration::new(5..100, second_style)],
        ));

        assert_ne!(first, second);
        assert_eq!(
            collections.get(first),
            Some(&[TextDecoration::new(1..4, first_style)][..])
        );
        assert_eq!(
            collections.get(second),
            Some(&[TextDecoration::new(5..6, second_style)][..])
        );

        assert!(collections.append(first, vec![TextDecoration::new(4..5, second_style)]));
        assert_eq!(
            collections.get(first),
            Some(
                &[
                    TextDecoration::new(1..4, first_style),
                    TextDecoration::new(4..5, second_style),
                ][..]
            )
        );

        assert!(collections.set(first, Vec::new()));
        assert_eq!(collections.get(first), Some(&[][..]));
        assert_eq!(
            collections.get(second),
            Some(&[TextDecoration::new(5..6, second_style)][..])
        );
    }

    #[test]
    fn decoration_ranges_follow_text_edits() {
        let style = HighlightStyle::default();
        let mut collections = DecorationCollections::default();
        let collection = collections.create(vec![TextDecoration::new(2..6, style)]);

        collections.adjust_for_edit(&(0..0), 2);
        assert_eq!(
            collections.get(collection),
            Some(&[TextDecoration::new(4..8, style)][..])
        );

        collections.adjust_for_edit(&(6..6), 2);
        assert_eq!(
            collections.get(collection),
            Some(&[TextDecoration::new(4..10, style)][..])
        );

        collections.adjust_for_edit(&(4..10), 3);
        assert_eq!(
            collections.get(collection),
            Some(&[TextDecoration::new(4..7, style)][..])
        );

        assert_eq!(adjust_range_for_edit(&(2..6), &(2..2), 2), 4..8);
        assert_eq!(adjust_range_for_edit(&(2..6), &(6..6), 2), 2..6);
        assert_eq!(adjust_range_for_edit(&(2..6), &(2..6), 3), 2..5);
    }
}
