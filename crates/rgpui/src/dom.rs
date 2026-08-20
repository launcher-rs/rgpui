//! rgpui Web DOM 后端：DOM 树数据模型与构建器。
//!
//! 该模块由 `dom-backend` feature 门控，默认关闭，桌面端不受影响。
//!
//! 设计要点（详见 `docs/web-dom-backend-plan.md` 与 `docs/web-dom-backend-analysis.md`）：
//! - element 树每帧重建、非保留，因此 DOM 后端不能做"两棵保留树 diff"，
//!   而是把实现了 [`crate::Element::dom`] 的元素在 paint 阶段登记进一棵**保留的** DOM 树；
//! - DOM 节点以 [`DomNodeKey`]（由 `GlobalElementId` 路径 + 匿名兄弟序号构成）为跨帧稳定 key；
//! - 布局沿用 Taffy 结果，以 `position: absolute + left/top/width/height` 1:1 落地，
//!   不依赖浏览器 flex 重排；
//! - 每帧由 [`DomTreeBuilder`] 重建一棵新鲜树，平台侧（`rgpui-dom`）拿新旧两棵树做增量 reconcile。

use std::sync::Arc;

use crate::collections::{FxHashMap, FxHashSet};
use crate::{
    Bounds, CursorStyle, ElementId, FontStyle, FontWeight, GlobalElementId, Hsla, Pixels, Point,
    SharedString, TextAlign, WhiteSpace,
};

/// DOM 显示类型（v1 仅区分显示/隐藏）。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DomDisplay {
    /// 正常显示（块级）。
    #[default]
    Block,
    /// 不显示（`display: none`）。
    None,
}

/// 背景渐变类型。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DomGradientKind {
    /// 线性渐变。
    #[default]
    Linear,
    /// 径向渐变。
    Radial,
    /// 锥形渐变。
    Conic,
}

/// 一个背景渐变（v1 支持单次线性/径向渐变，多色标）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DomGradient {
    /// 渐变类型。
    pub kind: DomGradientKind,
    /// 渐变角度（线性：顺时针方向角度；锥形：起始角）。
    pub angle: f32,
    /// 色标（颜色 + 位置 0..=1）。
    pub stops: Vec<(Hsla, f32)>,
}

/// 一个盒阴影，字段与 [`crate::BoxShadow`] 一一对应（CSS `box-shadow`）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DomBoxShadow {
    /// 阴影颜色。
    pub color: Hsla,
    /// 偏移量（x, y）。
    pub offset_x: Pixels,
    /// 偏移量（y 分量）。
    pub offset_y: Pixels,
    /// 模糊半径。
    pub blur_radius: Pixels,
    /// 扩散半径。
    pub spread_radius: Pixels,
    /// 是否为内阴影。
    pub inset: bool,
}

/// 溢出处理方式。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DomOverflow {
    /// 可见。
    #[default]
    Visible,
    /// 裁剪隐藏。
    Hidden,
    /// 可滚动（原生滚动）。
    Scroll,
}

/// DOM 节点样式，最终映射为内联 CSS。
///
/// 布局字段（`left/top/width/height`）直接来自 Taffy 结果；
/// 视觉字段由各元素的 [`crate::Element::dom`] 实现从 rgpui `Style`/`TextStyle` 转换而来。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DomStyle {
    /// 显示类型。
    pub display: DomDisplay,
    /// 绝对定位的左边距（Taffy bounds 结果）。
    pub left: Pixels,
    /// 绝对定位的上边距（Taffy bounds 结果）。
    pub top: Pixels,
    /// 宽度（Taffy bounds 结果）。
    pub width: Pixels,
    /// 高度（Taffy bounds 结果）。
    pub height: Pixels,
    /// 文本颜色。
    pub color: Option<Hsla>,
    /// 背景颜色。
    pub background_color: Option<Hsla>,
    /// 背景渐变（优先级高于背景色）。
    pub background_gradient: Option<DomGradient>,
    /// 圆角半径。
    pub border_radius: Option<Pixels>,
    /// 边框颜色。
    pub border_color: Option<Hsla>,
    /// 边框宽度（统一四条边，v1 不支持逐边宽度）。
    pub border_width: Option<Pixels>,
    /// 盒阴影列表。
    pub box_shadows: Vec<DomBoxShadow>,
    /// 字体大小。
    pub font_size: Option<Pixels>,
    /// 字体系列。
    pub font_family: Option<SharedString>,
    /// 字重。
    pub font_weight: Option<FontWeight>,
    /// 字体样式（斜体等）。
    pub font_style: Option<FontStyle>,
    /// 行高。
    pub line_height: Option<Pixels>,
    /// 文本对齐。
    pub text_align: Option<TextAlign>,
    /// 空白处理。
    pub white_space: Option<WhiteSpace>,
    /// 溢出处理。
    pub overflow: DomOverflow,
    /// 鼠标光标样式。
    pub cursor: Option<CursorStyle>,
    /// 不透明度。
    pub opacity: Option<f32>,
    /// z 轴层级（paint 顺序，由构建器填充）。
    pub z_index: u32,
}

impl DomStyle {
    /// 从 Taffy 布局结果构造 DOM 样式（绝对定位）。
    ///
    /// 视觉字段保持默认值，由元素侧按需覆盖。
    pub fn from_bounds(bounds: Bounds<Pixels>) -> Self {
        Self {
            left: bounds.origin.x,
            top: bounds.origin.y,
            width: bounds.size.width,
            height: bounds.size.height,
            ..Default::default()
        }
    }
}

/// DOM 节点类型。
#[derive(Clone, Debug, PartialEq)]
pub enum DomNodeKind {
    /// 元素节点，对应 HTML 标签与属性。
    Element {
        /// HTML 标签名（如 `div`、`span`、`button`）。
        tag: &'static str,
        /// 额外属性（如 `src`、`role`、`aria-*`）。
        attrs: Vec<(String, String)>,
    },
    /// 文本节点，由浏览器负责选择/复制/IME/无障碍。
    Text {
        /// 原始文本内容。
        text: SharedString,
    },
}

/// 一个已登记的 DOM 节点。
///
/// `key`（跨帧稳定标识）由 [`DomTreeBuilder`] 填充，元素侧只需提供 `kind` 与 `style`。
#[derive(Clone, Debug, PartialEq)]
pub struct DomNode {
    /// 节点类型（元素/文本）。
    pub kind: DomNodeKind,
    /// 节点样式（含绝对定位与视觉样式）。
    pub style: DomStyle,
}

/// DOM 节点的跨帧稳定 key。
///
/// 由「元素路径命名空间」+「DOM 层级定位」构成，保证带 id 与匿名的元素互不冲突：
/// - `global_id`：带 id 的 element 路径（匿名元素为最近带 id 祖先的路径，即当前 `element_id_stack`）。
///   带 id 的元素以 `global_id` 自身即可唯一标识（DOM 全局 id 唯一）；
/// - `dom_path`：该节点在其 DOM 父链中的兄弟序号路径。带 id 元素为空；
///   匿名元素为 `父节点.dom_path + [父下兄弟序号]`，从而与匿名祖先/后代天然区分。
///
/// 特殊情形：当同一 `ElementId`（如同一个 entity 复用于多个输入框）导致多个带 id
/// 元素的 `global_id` 相同产生碰撞时，重复实例会回退为「匿名式」消歧（`dom_path`
/// 追加父下兄弟序号），首个实例保留干净 key，从而保证 key 唯一且跨帧稳定。
///
/// 在「确定性渲染」前提下跨帧稳定，与 React 的 index-key 语义一致。
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct DomNodeKey {
    /// element 路径（命名空间）。
    pub global_id: GlobalElementId,
    /// DOM 父链兄弟序号路径（带 id 元素为空）。
    pub dom_path: Vec<u32>,
}

impl DomNodeKey {
    /// 构造一棵树的根节点 key（空路径 + 空 dom_path）。
    pub fn root() -> Self {
        Self {
            global_id: GlobalElementId::default(),
            dom_path: Vec::new(),
        }
    }

    /// 该节点是否为带 id 的元素（`dom_path` 为空）。
    pub fn is_keyed(&self) -> bool {
        self.dom_path.is_empty()
    }

    /// 生成用于 DOM `data-gpui-id` 属性的数值 id。
    ///
    /// 用 DefaultHasher 对 key 哈希，与 `GlobalElementId::accesskit_node_id` 同一思路。
    /// 平台侧维护 `数值 id -> DomNodeKey` 反查表即可做事件桥接。
    pub fn to_dom_id(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish().to_string()
    }
}

/// 一棵保留的 DOM 树（每帧由构建器新鲜生成，平台侧与上一帧对账）。
#[derive(Clone, Debug, Default)]
pub struct DomTree {
    /// 根节点 key。
    pub root: DomNodeKey,
    /// 全部已登记节点（key -> 节点）。
    pub nodes: FxHashMap<DomNodeKey, DomNode>,
    /// 父子关系（key -> 子 key 列表，按 paint 顺序）。
    pub children: FxHashMap<DomNodeKey, Vec<DomNodeKey>>,
    /// 每个节点的 z 序（paint 顺序，越大越靠上）。
    pub z_orders: FxHashMap<DomNodeKey, u32>,
}

impl DomTree {
    /// 树是否为空（除根外无节点）。
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// 按 paint 顺序遍历全部节点（z 序升序）。
    pub fn for_each_in_paint_order(&self, mut f: impl FnMut(&DomNodeKey, &DomNode, u32)) {
        let mut keys: Vec<&DomNodeKey> = self.nodes.keys().collect();
        keys.sort_by_key(|key| self.z_orders.get(*key).copied().unwrap_or(0));
        for key in keys {
            let z = self.z_orders.get(key).copied().unwrap_or(0);
            if let Some(node) = self.nodes.get(key) {
                f(key, node, z);
            }
        }
    }
}

/// 在 paint 阶段收集 DOM 节点的构建器。
///
/// 用法（由 [`crate::Window`] 的 `dom_element`/`dom_exit` 驱动）：
/// 每帧 `begin_frame` 后，paint 过程里带 DOM 映射的元素依次 `register`；
/// 帧末 `finish` 取走新鲜树。
pub struct DomTreeBuilder {
    tree: DomTree,
    /// 当前 DOM 父链（栈）。
    stack: Vec<DomNodeKey>,
    /// 与 `stack` 平行：每个栈帧对应的窗口绝对原点（px）。
    ///
    /// CSS 中所有节点均为 `position:absolute`，子节点的 `left/top` 是相对最近
    /// 的 positioned 祖先计算的。若直接使用 Taffy 的窗口绝对坐标，嵌套节点的
    /// 偏移会被逐层累加导致错位（"按钮文字丢失、布局错乱"的根因）。因此
    /// `register` 时把窗口绝对坐标换算为「相对父节点原点」的坐标。
    origins: Vec<Point<Pixels>>,
    /// 每个父节点下的匿名子节点计数。
    anon_counts: FxHashMap<DomNodeKey, u32>,
    /// paint 顺序计数器（z 序）。
    order: u32,
    /// 已用 key 集合（debug 断言防碰撞）。
    seen: FxHashSet<DomNodeKey>,
}

impl Default for DomTreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DomTreeBuilder {
    /// 创建一个空构建器。
    pub fn new() -> Self {
        Self {
            tree: DomTree::default(),
            stack: Vec::new(),
            origins: Vec::new(),
            anon_counts: FxHashMap::default(),
            order: 0,
            seen: FxHashSet::default(),
        }
    }

    /// 开始一帧：重置状态并压入根节点。
    pub fn begin_frame(&mut self) {
        self.tree = DomTree::default();
        self.tree.root = DomNodeKey::root();
        self.stack.clear();
        self.origins.clear();
        self.anon_counts.clear();
        self.seen.clear();
        self.order = 0;
        self.stack.push(self.tree.root.clone());
        self.origins.push(Point::default());
    }

    /// 登记一个节点，返回其 key，并把它压入 DOM 父链（元素有子节点时使用）。
    ///
    /// - `node`：元素提供的节点（kind + style）；
    /// - `is_keyed`：元素是否带 `.id()`（决定 key 是否只由 `global_id` 标识）；
    /// - `element_path`：当前 `Window::element_id_stack`（含本元素已压入的 id）。
    ///
    /// 带 id 的元素：`global_id` 取 `element_path`，`dom_path` 为空；
    /// 匿名元素：`global_id` 与父一致，`dom_path` 为父的 `dom_path` 追加本节点在父下的兄弟序号，
    /// 从而与匿名祖先/后代天然区分，不会产生 key 碰撞。
    pub fn register(
        &mut self,
        node: DomNode,
        is_keyed: bool,
        element_path: &[ElementId],
    ) -> DomNodeKey {
        let parent = self
            .stack
            .last()
            .expect("dom stack 不能为空，需先 begin_frame")
            .clone();
        // 父节点的窗口绝对原点（根节点为 0,0）。
        let parent_origin = *self
            .origins
            .last()
            .expect("dom origins 与 stack 平行，不能为空");

        // 把本节点的窗口绝对坐标换算为相对父节点原点的坐标。
        // 绝对定位的 CSS 子元素相对最近 positioned 祖先定位，若保留绝对坐标，
        // 嵌套节点的偏移会累加（按钮文字丢失/布局错乱的根因）。
        let window_origin = Point {
            x: node.style.left,
            y: node.style.top,
        };
        let mut node = node;
        node.style.left = window_origin.x - parent_origin.x;
        node.style.top = window_origin.y - parent_origin.y;

        let global_id = GlobalElementId(Arc::from(element_path));
        let mut key = if is_keyed {
            DomNodeKey {
                global_id,
                dom_path: Vec::new(),
            }
        } else {
            let index = self.anon_counts.entry(parent.clone()).or_insert(0);
            *index += 1;
            let mut dom_path = parent.dom_path.clone();
            dom_path.push(*index);
            DomNodeKey {
                global_id,
                dom_path,
            }
        };

        // 带 id 的元素通常以 `global_id` 唯一标识（DOM 全局 id 唯一）。但当同一
        // `ElementId` 复用于多个兄弟元素时（如故事里三个 Input 共享同一个
        // `InputState`，且外层包裹为匿名元素不压入 `element_id_stack`），
        // `global_id` 相同 → key 碰撞。此时回退为「匿名式」消歧：首个实例保留
        // 干净 key（跨帧稳定），重复实例在父节点下追加兄弟序号，保证 key 唯一。
        if self.seen.contains(&key) {
            let index = self.anon_counts.entry(parent.clone()).or_insert(0);
            *index += 1;
            let mut dom_path = parent.dom_path.clone();
            dom_path.push(*index);
            key = DomNodeKey {
                global_id: key.global_id,
                dom_path,
            };
        }

        debug_assert!(!self.seen.contains(&key), "DOM key 重复：{}", key.global_id);
        self.seen.insert(key.clone());

        self.tree.nodes.insert(key.clone(), node);
        self.tree
            .children
            .entry(parent)
            .or_default()
            .push(key.clone());
        self.tree.z_orders.insert(key.clone(), self.order);
        self.order += 1;

        self.stack.push(key.clone());
        self.origins.push(window_origin);
        key
    }

    /// 元素 paint 结束后弹出其 DOM 栈帧（与 `register` 配对）。
    pub fn exit(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
            self.origins.pop();
        }
    }

    /// 当前 DOM 父节点 key。
    pub fn current_parent(&self) -> DomNodeKey {
        self.stack.last().expect("dom stack 不能为空").clone()
    }

    /// 当前 DOM 栈深度（根节点也算一层）。
    pub fn stack_len(&self) -> usize {
        self.stack.len()
    }

    /// 结束一帧，取走新鲜树。
    pub fn finish(&mut self) -> DomTree {
        let mut tree = std::mem::take(&mut self.tree);
        tree.root = DomNodeKey::root();
        self.stack.clear();
        tree
    }
}

// 线程局部：DOM 层（canvas 之上的文本覆盖层）是否启用。
//
// 默认关闭：Web 平台默认走纯 canvas 渲染，与启用 `dom-backend` feature 之前的
// 行为完全一致。应用需要在打开窗口前调用 set_dom_layer_enabled 显式开启。
thread_local! {
    static DOM_LAYER_ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 设置 DOM 层是否启用（Web 平台）。
///
/// 启用后，窗口会额外把实现了 [`Element::dom`] 的元素（div、文本等）渲染为一层
/// 绝对定位的 DOM 覆盖层，浏览器原生提供文本选择 / 复制 / IME 等能力（v1 接受
/// 与 canvas 双重绘制）。必须在打开窗口之前调用，例如：
///
/// ```text
/// rgpui::set_dom_layer_enabled(true);
/// ```
///
/// 桌面平台不实现 `supports_dom`，此开关对其无影响。
pub fn set_dom_layer_enabled(enabled: bool) {
    DOM_LAYER_ENABLED.with(|cell| cell.set(enabled));
}

/// 查询 DOM 层是否启用（Web 平台）。
pub fn dom_layer_enabled() -> bool {
    DOM_LAYER_ENABLED.with(|cell| cell.get())
}

// 线程局部：DOM 覆盖层的字体面注册表。
//
// Web 平台 canvas 与应用共享内嵌字体，但浏览器并不知道这些字体。
// 应用把嵌入的字体字节（与 cosmic-text 使用的完全相同）注册到这里，
// DOM 后端（`rgpui-dom`）会据此注入 `@font-face`，使覆盖层的文本与应用
// 使用同一字面，消除双重绘制时的字体回退错位（“重影”）。
thread_local! {
    static DOM_FONT_FACES: std::cell::RefCell<Vec<DomFontFace>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// DOM 覆盖层使用的一个字体面：`family` 是字体族名（须与 DOM 样式中的
/// `font-family` 一致，通常即主题的 `font_family`），`data` 是该字体的原始字节。
#[derive(Clone, Debug)]
pub struct DomFontFace {
    /// 字体族名（例如 `"Inter Variable"`）。
    pub family: SharedString,
    /// 字体文件原始字节（TTF/OTF/WOFF2 均可）。
    pub data: Arc<Vec<u8>>,
}

/// 注册一个供 DOM 覆盖层使用的字体面。
///
/// 调用时机：必须在打开窗口之前（DOM 后端挂载覆盖层时一次性读取注册表）。
/// 字体族名必须与 DOM 样式输出的 `font-family` 一致（一般取主题的 `font_family` /
/// `mono_font_family`），并把与应用内嵌完全相同的字体字节传进来。
///
/// ```text
/// rgpui::set_dom_font_face("Inter Variable", include_bytes!(".../Inter-Regular.ttf"));
/// ```
pub fn set_dom_font_face(family: impl Into<SharedString>, data: impl AsRef<[u8]>) {
    DOM_FONT_FACES.with(|faces| {
        faces.borrow_mut().push(DomFontFace {
            family: family.into(),
            data: Arc::new(data.as_ref().to_vec()),
        });
    });
}

/// 读取已注册的 DOM 字体面列表（供 DOM 后端注入 `@font-face`）。
pub fn dom_font_faces() -> Vec<DomFontFace> {
    DOM_FONT_FACES.with(|faces| faces.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SharedString;

    fn key(path: &[ElementId], dom_path: &[u32]) -> DomNodeKey {
        DomNodeKey {
            global_id: GlobalElementId(Arc::from(path)),
            dom_path: dom_path.to_vec(),
        }
    }

    fn div_node() -> DomNode {
        DomNode {
            kind: DomNodeKind::Element {
                tag: "div",
                attrs: Vec::new(),
            },
            style: DomStyle::default(),
        }
    }

    fn text_node(text: &str) -> DomNode {
        DomNode {
            kind: DomNodeKind::Text {
                text: SharedString::from(text),
            },
            style: DomStyle::default(),
        }
    }

    #[test]
    fn test_builder_hierarchy_and_keys() {
        let mut builder = DomTreeBuilder::new();
        builder.begin_frame();

        // 根 > div(id="a") > 文本1、文本2
        let a = ElementId::Name("a".into());
        let div_key = builder.register(div_node(), true, &[a.clone()]);
        let text1_key = builder.register(text_node("hello"), false, &[a.clone()]);
        builder.exit();
        let text2_key = builder.register(text_node("world"), false, &[a.clone()]);
        builder.exit();
        builder.exit();

        let tree = builder.finish();

        // key 语义：id 元素仅由 global_id 标识；匿名兄弟在父下按 1、2 计数
        assert_eq!(div_key, key(&[a.clone()], &[]));
        assert_eq!(text1_key, key(&[a.clone()], &[1]));
        assert_eq!(text2_key, key(&[a.clone()], &[2]));

        // 父子关系
        let children = tree.children.get(&tree.root).unwrap();
        assert_eq!(children, &vec![div_key.clone()]);
        let div_children = tree.children.get(&div_key).unwrap();
        assert_eq!(div_children, &vec![text1_key.clone(), text2_key.clone()]);

        // 文本内容
        assert_eq!(tree.nodes.get(&text1_key).unwrap(), &text_node("hello"));

        // z 序：根 > div > 文本1 > 文本2
        assert_eq!(tree.z_orders.get(&div_key), Some(&0));
        assert_eq!(tree.z_orders.get(&text1_key), Some(&1));
        assert_eq!(tree.z_orders.get(&text2_key), Some(&2));
    }

    #[test]
    fn test_anonymous_nested_container() {
        let mut builder = DomTreeBuilder::new();
        builder.begin_frame();

        // 根 > div(id="a") > 匿名div > 文本
        let a = ElementId::Name("a".into());
        let div_key = builder.register(div_node(), true, &[a.clone()]);
        let anon_div = builder.register(div_node(), false, &[a.clone()]);
        let text_key = builder.register(text_node("x"), false, &[a.clone()]);
        builder.exit();
        builder.exit();

        let tree = builder.finish();

        // 匿名 div 的 dom_path=[1]；其匿名子文本的 dom_path 追加为 [1,1]，与父不冲突
        assert_eq!(anon_div, key(&[a.clone()], &[1]));
        assert_eq!(text_key, key(&[a.clone()], &[1, 1]));

        let div_children = tree.children.get(&div_key).unwrap();
        assert_eq!(div_children, &vec![anon_div.clone()]);
        let anon_children = tree.children.get(&anon_div).unwrap();
        assert_eq!(anon_children, &vec![text_key.clone()]);
    }

    #[test]
    fn test_nested_keyed_child() {
        let mut builder = DomTreeBuilder::new();
        builder.begin_frame();

        // 根 > div(id="a") > div(id="b")
        let a = ElementId::Name("a".into());
        let b = ElementId::Name("b".into());
        let div_a = builder.register(div_node(), true, &[a.clone()]);
        let div_b = builder.register(div_node(), true, &[a.clone(), b.clone()]);
        builder.exit();
        builder.exit();

        let tree = builder.finish();

        assert_eq!(div_a, key(&[a.clone()], &[]));
        assert_eq!(div_b, key(&[a.clone(), b.clone()], &[]));
        let root_children = tree.children.get(&tree.root).unwrap();
        assert_eq!(root_children, &vec![div_a.clone()]);
        let a_children = tree.children.get(&div_a).unwrap();
        assert_eq!(a_children, &vec![div_b.clone()]);
    }
}
