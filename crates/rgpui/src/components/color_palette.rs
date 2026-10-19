//! 色板。
//!
//! 等尺寸色块的自动换行网格：用于呈现**主题语义色**、**命名色阶**或**最近颜色**，
//! 点击色块经回调抛出所选颜色。与 [`ColorPicker`](crate::ColorPickerState) 配合时
//! 作为取色器的快捷色板，也可独立用于任何「选一个预设色」的场景。
//!
//! 最近颜色的维护由纯函数 [`push_recent`] 承担（去重置顶 + 截断），应用可自行持有
//! 列表并复用同一份「最近使用」。

use std::sync::Arc;

use crate::{
    ActiveTheme as _, App, ColorName, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};

/// 最近颜色的默认上限。
pub const RECENT_COLORS_LIMIT: usize = 12;
/// 色块的默认边长（像素）。
pub const SWATCH_SIZE: f32 = 16.0;
/// 色块的默认间距（像素）。
pub const SWATCH_GAP: f32 = 4.0;

/// 把颜色置顶插入最近颜色列表：已存在时先移除再置顶，最后截断到 `limit`。
///
/// 返回列表是否发生变化（用于决定是否重绘）：颜色已在首位且未超限时返回 `false`。
pub fn push_recent(recents: &mut Vec<Hsla>, color: Hsla, limit: usize) -> bool {
    let limit = limit.max(1);
    let was_first = recents.first() == Some(&color);
    let over_limit = recents.len() > limit;
    if !was_first {
        recents.retain(|existing| *existing != color);
        recents.insert(0, color);
    }
    recents.truncate(limit);
    !was_first || over_limit
}

/// 当前主题的语义色板（主色 / 强调 / 语义色 / 命名色）。
pub fn theme_palette(cx: &App) -> Vec<Hsla> {
    let theme = cx.theme();
    vec![
        theme.primary,
        theme.primary_hover,
        theme.secondary,
        theme.accent,
        theme.danger,
        theme.success,
        theme.warning,
        theme.info,
        theme.link,
        theme.red,
        theme.green,
        theme.blue,
        theme.yellow,
        theme.cyan,
        theme.foreground,
        theme.muted,
        theme.border,
        theme.ring,
    ]
}

/// 指定色阶（`50`/`100`/…/`950`）下所有命名颜色的色板。
pub fn color_name_palette(scale: usize) -> Vec<Hsla> {
    ColorName::all()
        .iter()
        .map(|name| name.scale(scale))
        .collect()
}

/// 色板元素：色块的自动换行网格。
#[derive(IntoElement)]
pub struct ColorPalette {
    /// 标识前缀（用于生成稳定的子元素 id）。
    id: SharedString,
    /// 颜色列表。
    colors: Vec<Hsla>,
    /// 当前选中颜色（相同颜色的色块加描边）。
    selected: Option<Hsla>,
    /// 色块边长（像素）。
    size: f32,
    /// 色块间距（像素）。
    gap: f32,
    /// 点击回调。
    on_select: Option<Arc<dyn Fn(Hsla, &mut Window, &mut App) + 'static>>,
}

impl ColorPalette {
    /// 创建色板（`id` 需在其父级内唯一）。
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            colors: Vec::new(),
            selected: None,
            size: SWATCH_SIZE,
            gap: SWATCH_GAP,
            on_select: None,
        }
    }

    /// 设置颜色列表。
    pub fn colors(mut self, colors: impl IntoIterator<Item = Hsla>) -> Self {
        self.colors = colors.into_iter().collect();
        self
    }

    /// 设置选中颜色。
    pub fn selected(mut self, color: Option<Hsla>) -> Self {
        self.selected = color;
        self
    }

    /// 设置色块边长（像素，最小 6）。
    pub fn swatch_size(mut self, size: f32) -> Self {
        self.size = size.max(6.0);
        self
    }

    /// 设置色块间距（像素，非负）。
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    /// 设置点击色块的回调。
    pub fn on_select<F>(mut self, handler: F) -> Self
    where
        F: Fn(Hsla, &mut Window, &mut App) + 'static,
    {
        self.on_select = Some(Arc::new(handler));
        self
    }

    /// 使用共享回调（同一回调可复用给多个色板）。
    pub fn on_select_arc(
        mut self,
        handler: Arc<dyn Fn(Hsla, &mut Window, &mut App) + 'static>,
    ) -> Self {
        self.on_select = Some(handler);
        self
    }
}

impl RenderOnce for ColorPalette {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            colors,
            selected,
            size,
            gap,
            on_select,
        } = self;

        let theme = cx.theme();
        let border = theme.border;
        let ring = theme.ring;

        let swatches: Vec<_> = colors
            .into_iter()
            .enumerate()
            .map(|(index, color)| {
                let is_selected = selected == Some(color);
                let mut swatch = div()
                    .id((id.clone(), index))
                    .w(px(size))
                    .h(px(size))
                    .rounded_sm()
                    .bg(color)
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .hover(move |style| style.border_color(ring));
                if is_selected {
                    swatch = swatch.border_2().border_color(ring);
                }
                if let Some(handler) = on_select.clone() {
                    swatch = swatch.on_click(move |_event, window, cx| handler(color, window, cx));
                }
                swatch.into_any_element()
            })
            .collect();

        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(gap))
            .children(swatches)
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{
        ColorPalette, RECENT_COLORS_LIMIT, color_name_palette, push_recent, theme_palette,
    };
    use crate::{
        ActiveTheme as _, ColorName, Context, Entity, Hsla, IntoElement, ParentElement as _,
        Render, Styled as _, Window, div, hsla,
    };
    use std::cell::Cell;
    use std::rc::Rc;

    /// 渲染色板的测试宿主。
    struct Probe {
        /// 色板中被点击的颜色（未点击为 None）。
        picked: Rc<Cell<Option<Hsla>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let picked = self.picked.clone();
            let selected = cx.theme().primary;
            div()
                .flex()
                .flex_col()
                .size_full()
                .child(
                    ColorPalette::new("theme-palette")
                        .colors(theme_palette(cx))
                        .selected(Some(selected))
                        .on_select(move |color, _window, _cx| picked.set(Some(color))),
                )
                .child(
                    ColorPalette::new("scale-palette")
                        .colors(color_name_palette(500))
                        .swatch_size(12.0)
                        .gap(2.0),
                )
        }
    }

    /// 最近颜色：新增置顶、去重前移、截断上限、重复时无变化。
    #[test]
    fn push_recent_dedupes_and_truncates() {
        let a = hsla(0.0, 0.5, 0.5, 1.0);
        let b = hsla(0.2, 0.5, 0.5, 1.0);
        let c = hsla(0.4, 0.5, 0.5, 1.0);

        let mut recents = Vec::new();
        assert!(push_recent(&mut recents, a, RECENT_COLORS_LIMIT));
        assert_eq!(recents, vec![a]);

        // 已在首位：无变化。
        assert!(!push_recent(&mut recents, a, RECENT_COLORS_LIMIT));
        assert_eq!(recents, vec![a]);

        // 新颜色置顶。
        assert!(push_recent(&mut recents, b, RECENT_COLORS_LIMIT));
        assert_eq!(recents, vec![b, a]);

        // 已有颜色被前移（而不是重复插入）。
        assert!(push_recent(&mut recents, a, RECENT_COLORS_LIMIT));
        assert_eq!(recents, vec![a, b]);

        // 达到上限后挤出最旧的一条。
        assert!(push_recent(&mut recents, c, 2));
        assert_eq!(recents, vec![c, a]);
        assert_eq!(recents.len(), 2);

        // 上限为 0 时按 1 处理。
        let mut recents = vec![a, b];
        assert!(push_recent(&mut recents, c, 0));
        assert_eq!(recents, vec![c]);
    }

    /// 命名色阶色板与颜色名一一对应，且色阶回退到 500。
    #[test]
    fn color_name_palette_matches_named_colors() {
        let palette = color_name_palette(500);
        assert_eq!(palette.len(), ColorName::all().len());
        // 顺序与 `ColorName::all()` 一致。
        let expected: Vec<Hsla> = ColorName::all()
            .iter()
            .map(|name| name.scale(500))
            .collect();
        assert_eq!(palette, expected);
        assert!(palette.contains(&ColorName::Blue.scale(500)));
        // 非法色阶回退到 500。
        assert_eq!(ColorName::Blue.scale(999), ColorName::Blue.scale(500));
    }

    /// 主题色板包含主色与语义色，且两种色板都能渲染。
    #[rgpui::test]
    fn theme_palette_contains_semantic_colors_and_renders(cx: &mut crate::TestAppContext) {
        cx.update(crate::theme::init);
        cx.update(|cx| {
            let palette = theme_palette(cx);
            // 主色 / 危险色 / 前景色都在主题色板里。
            assert!(palette.contains(&cx.theme().primary));
            assert!(palette.contains(&cx.theme().danger));
            assert!(palette.contains(&cx.theme().foreground));
            assert_eq!(palette.len(), 18);
        });

        let picked: Rc<Cell<Option<Hsla>>> = Rc::new(Cell::new(None));
        let picked_ref = picked.clone();
        let (probe, mut cx) = cx.add_window_view(move |_window, _cx| Probe { picked: picked_ref });
        let _: Entity<Probe> = probe;

        // 强制走一遍绘制，验证色块的 id / 悬停 / 点击接线都能渲染。
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        assert!(picked.get().is_none());
    }
}
