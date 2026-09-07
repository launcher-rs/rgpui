//! Mermaid 子集：`flowchart`（LR/TB/RL/BT）文本转 SVG 渲染。
//!
//! 支持节点形状：矩形 `A[文本]`、圆角 `A(文本)`、菱形 `A{文本}`、圆形 `A((文本))`；
//! 边：`A --> B`、`A --- B`、带标签 `A -->|文本| B`；多语句可用 `;` 分隔。
//! 布局为简单的按深度分层（无自动避让），完整 Mermaid 语义（子图/曲线边等）明确不支持。
//! 渲染走现有 [`Svg`] 元素（usvg 光栅化），零新渲染代码。

use crate::{prelude::FluentBuilder as _, *};
use std::collections::{HashMap, HashSet};

/// 节点形状。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum NodeShape {
    /// 矩形 `A[文本]`。
    #[default]
    Rect,
    /// 圆角 `A(文本)`。
    Rounded,
    /// 菱形 `A{文本}`。
    Diamond,
    /// 圆形 `A((文本))`。
    Circle,
}

/// 解析出的节点。
#[derive(Debug, Clone)]
struct FlowNode {
    id: String,
    label: String,
    shape: NodeShape,
}

/// 解析出的边。
#[derive(Debug, Clone)]
struct FlowEdge {
    from: String,
    to: String,
    label: String,
}

/// 流程图方向。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Direction {
    /// 从左到右。
    #[default]
    LeftRight,
    /// 从右到左。
    RightLeft,
    /// 从上到下。
    TopBottom,
    /// 从下到上。
    BottomTop,
}

/// 解析 `flowchart` 子集，返回（方向，节点，边）。
fn parse_flowchart(source: &str) -> (Direction, Vec<FlowNode>, Vec<FlowEdge>) {
    let mut direction = Direction::LeftRight;
    let mut nodes: Vec<FlowNode> = Vec::new();
    let mut node_ids: HashSet<String> = HashSet::new();
    let mut edges: Vec<FlowEdge> = Vec::new();

    let mut ensure_node = |id: &str, nodes: &mut Vec<FlowNode>, node_ids: &mut HashSet<String>| {
        if !id.is_empty() && !node_ids.contains(id) {
            node_ids.insert(id.to_string());
            nodes.push(FlowNode {
                id: id.to_string(),
                label: id.to_string(),
                shape: NodeShape::Rect,
            });
        }
    };

    // 从 token 中拆出 id 与内联形状：`A[文本]` → ("A", Rect, "文本")，`A` → ("A", None)。
    fn split_node_token(token: &str) -> (String, Option<(NodeShape, String)>) {
        let token = token.trim();
        for (open, shape) in [
            ("((", NodeShape::Circle),
            ("([", NodeShape::Rounded),
            ("[[", NodeShape::Rect),
            ("[", NodeShape::Rect),
            ("(", NodeShape::Rounded),
            ("{", NodeShape::Diamond),
        ] {
            if let Some(pos) = token.find(open) {
                let id = token[..pos].trim().to_string();
                let close = match open {
                    "((" => "))",
                    "([" => "])",
                    "[[" => "]]",
                    "[" => "]",
                    "(" => ")",
                    _ => "}",
                };
                let inner = token[pos + open.len()..].trim();
                if let Some(label) = inner.strip_suffix(close) {
                    return (id, Some((shape, label.trim().to_string())));
                }
                return (id, None);
            }
        }
        (token.to_string(), None)
    }

    for raw_line in source.lines() {
        // 去注释（`%%` 后面全是注释）。
        let line = match raw_line.find("%%") {
            Some(ix) => &raw_line[..ix],
            None => raw_line,
        };
        for stmt in line.split(';') {
            let stmt = stmt.trim();
            if stmt.is_empty() {
                continue;
            }
            // 方向声明。
            if let Some(rest) = stmt
                .strip_prefix("flowchart")
                .or_else(|| stmt.strip_prefix("graph"))
            {
                direction = match rest.trim() {
                    "RL" => Direction::RightLeft,
                    "TB" => Direction::TopBottom,
                    "BT" => Direction::BottomTop,
                    _ => Direction::LeftRight,
                };
                continue;
            }
            // 边：按 `-->` / `---` 切分（支持链式 `A --> B --> C`，UTF-8 安全）。
            if stmt.contains("-->") || stmt.contains("---") {
                // 找出所有箭头位置并切出节点 token。
                let mut tokens: Vec<String> = Vec::new();
                let mut current = String::new();
                let mut ix = 0;
                let mut edge_labels: Vec<String> = Vec::new();
                while ix < stmt.len() {
                    let rest = &stmt[ix..];
                    if rest.starts_with("-->") || rest.starts_with("---") {
                        let taken = std::mem::take(&mut current);
                        tokens.push(taken.trim().to_string());
                        ix += 3;
                        // 跳过箭头与 `|label|` 之间的空白。
                        ix += stmt[ix..].len() - stmt[ix..].trim_start().len();
                        // 箭头后可能跟 `|label|`。
                        if stmt[ix..].starts_with('|') {
                            ix += 1;
                            match stmt[ix..].find('|') {
                                Some(end) => {
                                    edge_labels.push(stmt[ix..ix + end].trim().to_string());
                                    ix += end + 1;
                                }
                                None => edge_labels.push(String::new()),
                            }
                        } else {
                            edge_labels.push(String::new());
                        }
                    } else {
                        let ch = rest.chars().next().unwrap_or_default();
                        current.push(ch);
                        ix += ch.len_utf8().max(1);
                    }
                }
                tokens.push(current.trim().to_string());
                let mut prev: Option<(String, usize)> = None;
                for (token_ix, token) in tokens.iter().enumerate() {
                    if token.is_empty() {
                        continue;
                    }
                    let (id, shaped) = split_node_token(token);
                    if id.is_empty() {
                        continue;
                    }
                    ensure_node(&id, &mut nodes, &mut node_ids);
                    if let Some((shape, label)) = shaped {
                        if let Some(node) = nodes.iter_mut().find(|n| n.id == id) {
                            node.shape = shape;
                            node.label = label;
                        }
                    }
                    if let Some((prev_id, prev_ix)) = prev.take() {
                        let label = edge_labels.get(prev_ix).cloned().unwrap_or_default();
                        edges.push(FlowEdge {
                            from: prev_id,
                            to: id.clone(),
                            label,
                        });
                    }
                    prev = Some((id, token_ix));
                }
            } else {
                // 独立节点定义（如 `A[开始]` 独占一行）。
                let (id, shaped) = split_node_token(stmt);
                if id.is_empty() {
                    continue;
                }
                ensure_node(&id, &mut nodes, &mut node_ids);
                if let Some((shape, label)) = shaped {
                    if let Some(node) = nodes.iter_mut().find(|n| n.id == id) {
                        node.shape = shape;
                        node.label = label;
                    }
                }
            }
        }
    }

    (direction, nodes, edges)
}

/// 估算文本宽度（ASCII 7px，CJK 14px，14px 字号）。
fn text_width(label: &str) -> f32 {
    label
        .chars()
        .map(|c| if c.is_ascii() { 7.0 } else { 14.0 })
        .sum()
}

/// 布局后的节点矩形（中心坐标 + 宽高）。
struct PlacedNode {
    id: String,
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
    shape: NodeShape,
    label: String,
}

/// 分层布局：按最长路径深度分列（LR），同列按序排列。
fn layout_nodes(nodes: &[FlowNode], edges: &[FlowEdge]) -> Vec<PlacedNode> {
    const NODE_H: f32 = 44.0;
    const GAP_X: f32 = 80.0;
    const GAP_Y: f32 = 28.0;
    const PAD_X: f32 = 18.0;

    // 入边表。
    let mut incoming: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges {
        incoming
            .entry(edge.to.as_str())
            .or_default()
            .push(edge.from.as_str());
    }
    // 最长路径深度（BFS + 环保护）。
    let mut depth: HashMap<&str, usize> = HashMap::new();
    let mut changed = true;
    let mut rounds = 0;
    while changed && rounds <= nodes.len() + 1 {
        changed = false;
        rounds += 1;
        for node in nodes {
            let id = node.id.as_str();
            let preds = incoming.get(id).cloned().unwrap_or_default();
            let mut best = 0;
            let mut ready = true;
            for pred in &preds {
                match depth.get(pred) {
                    Some(d) => best = best.max(*d + 1),
                    None => {
                        if *pred != id {
                            ready = false;
                        }
                    }
                }
            }
            if ready && depth.get(id).is_none_or(|d| *d < best) {
                depth.insert(id, best);
                changed = true;
            }
        }
    }
    // 环上解不出的节点放到最右。
    let max_depth = depth.values().copied().max().unwrap_or(0);
    for node in nodes {
        depth.entry(node.id.as_str()).or_insert(max_depth + 1);
    }

    // 按列分组（列内保持出现顺序）。
    let mut columns: Vec<Vec<&FlowNode>> = vec![Vec::new(); max_depth + 2];
    for node in nodes {
        columns[depth[node.id.as_str()]].push(node);
    }
    // 列宽 = 列内最大节点宽 + 间距。
    let col_widths: Vec<f32> = columns
        .iter()
        .map(|col| {
            col.iter()
                .map(|n| text_width(&n.label) + PAD_X * 2.0 + 20.0)
                .fold(0.0f32, f32::max)
        })
        .collect();

    let mut placed = Vec::new();
    let mut x = PAD_X;
    for (col_ix, col) in columns.iter().enumerate() {
        let mut y = PAD_X;
        for node in col {
            let w = text_width(&node.label) + PAD_X * 2.0 + 20.0;
            let (w, h) = match node.shape {
                NodeShape::Diamond => (w.max(NODE_H + 30.0), NODE_H + 24.0),
                NodeShape::Circle => {
                    let d = (text_width(&node.label) + 30.0).max(NODE_H + 10.0);
                    (d, d)
                }
                _ => (w, NODE_H),
            };
            placed.push(PlacedNode {
                id: node.id.clone(),
                cx: x + col_widths[col_ix] / 2.0,
                cy: y + h / 2.0,
                w,
                h,
                shape: node.shape,
                label: node.label.clone(),
            });
            y += h + GAP_Y;
        }
        x += col_widths[col_ix] + GAP_X;
    }
    placed
}

/// XML 转义。
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 生成完整 SVG 文档（浅色字 on 深底由调用方主题色决定，见 `render_svg` 参数）。
fn emit_svg(
    placed: &[PlacedNode],
    edges: &[FlowEdge],
    direction: Direction,
    fg: &str,
    border: &str,
    accent: &str,
) -> String {
    let by_id: HashMap<&str, &PlacedNode> = placed.iter().map(|p| (p.id.as_str(), p)).collect();
    let max_x = placed
        .iter()
        .map(|p| p.cx + p.w / 2.0)
        .fold(0.0f32, f32::max);
    let max_y = placed
        .iter()
        .map(|p| p.cy + p.h / 2.0)
        .fold(0.0f32, f32::max);
    let pad = 18.0;

    // 方向变换：先按 LR 布局，再整体镜像/转置。
    let horizontal = matches!(direction, Direction::LeftRight | Direction::RightLeft);
    let mirror_x = matches!(direction, Direction::RightLeft);
    let mirror_y = matches!(direction, Direction::BottomTop);
    let (w, h) = if horizontal {
        (max_x + pad, max_y + pad)
    } else {
        (max_y + pad, max_x + pad)
    };
    // 逻辑坐标 → SVG 坐标。
    let map = |x: f32, y: f32| -> (f32, f32) {
        let (mut x, mut y) = if horizontal { (x, y) } else { (y, x) };
        if mirror_x {
            x = w - x;
        }
        if mirror_y {
            y = h - y;
        }
        (x, y)
    };

    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"0 0 {w:.0} {h:.0}\">"
    );
    svg.push_str(&format!(
        "<defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"8\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M 0 1 L 9 5 L 0 9 z\" fill=\"{border}\"/></marker></defs>"
    ));

    // 边（直线 + 箭头 + 中点标签）。
    for edge in edges {
        let (Some(from), Some(to)) = (by_id.get(edge.from.as_str()), by_id.get(edge.to.as_str()))
        else {
            continue;
        };
        // 从源节点边缘指向目标节点边缘（按主轴方向取出口/入口点）。
        let (x1, y1, x2, y2) = if horizontal {
            (from.cx + from.w / 2.0, from.cy, to.cx - to.w / 2.0, to.cy)
        } else {
            (from.cx, from.cy + from.h / 2.0, to.cx, to.cy - to.h / 2.0)
        };
        let (sx1, sy1) = map(x1, y1);
        let (sx2, sy2) = map(x2, y2);
        svg.push_str(&format!(
            "<line x1=\"{sx1:.1}\" y1=\"{sy1:.1}\" x2=\"{sx2:.1}\" y2=\"{sy2:.1}\" stroke=\"{border}\" stroke-width=\"1.5\" marker-end=\"url(#arrow)\"/>"
        ));
        if !edge.label.is_empty() {
            let (lx, ly) = map((x1 + x2) / 2.0, (y1 + y2) / 2.0 - 8.0);
            svg.push_str(&format!(
                "<text x=\"{lx:.1}\" y=\"{ly:.1}\" text-anchor=\"middle\" font-size=\"12\" font-family=\"sans-serif\" fill=\"{accent}\">{}</text>",
                escape_xml(&edge.label)
            ));
        }
    }

    // 节点。
    for node in placed {
        let (cx, cy) = map(node.cx, node.cy);
        let x = cx - node.w / 2.0;
        let y = cy - node.h / 2.0;
        match node.shape {
            NodeShape::Rect => svg.push_str(&format!(
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"4\" fill=\"none\" stroke=\"{border}\" stroke-width=\"1.5\"/>",
                node.w, node.h
            )),
            NodeShape::Rounded => svg.push_str(&format!(
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"{:.1}\" fill=\"none\" stroke=\"{border}\" stroke-width=\"1.5\"/>",
                node.w,
                node.h,
                node.h / 2.0
            )),
            NodeShape::Diamond => {
                let (top_x, top_y) = (cx, y);
                let (r_x, r_y) = (x + node.w, cy);
                let (b_x, b_y) = (cx, y + node.h);
                let (l_x, l_y) = (x, cy);
                svg.push_str(&format!(
                    "<polygon points=\"{top_x:.1},{top_y:.1} {r_x:.1},{r_y:.1} {b_x:.1},{b_y:.1} {l_x:.1},{l_y:.1}\" fill=\"none\" stroke=\"{border}\" stroke-width=\"1.5\"/>"
                ));
            }
            NodeShape::Circle => svg.push_str(&format!(
                "<circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"{:.1}\" fill=\"none\" stroke=\"{border}\" stroke-width=\"1.5\"/>",
                node.w / 2.0
            )),
        }
        svg.push_str(&format!(
            "<text x=\"{cx:.1}\" y=\"{cy:.1}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"14\" font-family=\"sans-serif\" fill=\"{fg}\">{}</text>",
            escape_xml(&node.label)
        ));
    }

    svg.push_str("</svg>");
    svg
}

/// Mermaid 流程图组件（子集，见模块文档）。
#[derive(IntoElement)]
pub struct MermaidDiagram {
    source: SharedString,
    style: StyleRefinement,
}

impl MermaidDiagram {
    /// 由 `flowchart` 源码创建。
    pub fn new(source: impl Into<SharedString>) -> Self {
        Self {
            source: source.into(),
            style: StyleRefinement::default(),
        }
    }

    /// 解析 + 布局 + 生成 SVG 文档（纯函数，可单测）。
    pub fn render_svg(&self, fg: Hsla, border: Hsla, accent: Hsla) -> String {
        let (direction, nodes, edges) = parse_flowchart(&self.source);
        if nodes.is_empty() {
            return "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>".to_string();
        }
        let placed = layout_nodes(&nodes, &edges);
        emit_svg(
            &placed,
            &edges,
            direction,
            &to_hex(&fg),
            &to_hex(&border),
            &to_hex(&accent),
        )
    }
}

impl Styled for MermaidDiagram {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for MermaidDiagram {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let fg = theme.tokens.foreground.color;
        let border = theme.tokens.border.color;
        let accent = theme.tokens.accent.color;
        let svg_doc = self.render_svg(fg, border, accent);
        let user_style = self.style;
        svg().data(svg_doc.as_bytes()).map(|mut this| {
            this.style().refine(&user_style);
            this
        })
    }
}

/// `Hsla` 转 `#rrggbb`（SVG 用）。
fn to_hex(color: &Hsla) -> String {
    let rgba: Rgba = (*color).into();
    format!(
        "#{:02x}{:02x}{:02x}",
        (rgba.r * 255.0) as u8,
        (rgba.g * 255.0) as u8,
        (rgba.b * 255.0) as u8
    )
}

#[cfg(test)]
mod tests {
    use super::{Direction, MermaidDiagram, NodeShape, layout_nodes, parse_flowchart};
    use crate::hsla;
    use std::collections::HashMap;

    /// 基本解析：方向 + 节点形状 + 边标签。
    #[test]
    fn parse_basic_flowchart() {
        let (direction, nodes, edges) =
            parse_flowchart("flowchart LR\nA[开始] -->|确认| B{判断}\nB --> C(结束)");
        assert_eq!(direction, Direction::LeftRight);
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[1].shape, NodeShape::Diamond);
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].label, "确认");
        assert!(edges[1].label.is_empty());
    }

    /// 深度分层：链式节点深度递增。
    #[test]
    fn layout_depth_order() {
        let (_, nodes, edges) = parse_flowchart("flowchart LR\nA --> B --> C");
        let placed = layout_nodes(&nodes, &edges);
        let x: HashMap<&str, f32> = placed.iter().map(|p| (p.id.as_str(), p.cx)).collect();
        assert!(x["A"] < x["B"] && x["B"] < x["C"]);
    }

    /// SVG 输出包含节点文本与箭头标记。
    #[test]
    fn svg_contains_shapes_and_arrows() {
        let diagram = MermaidDiagram::new("flowchart TB\nA[上] --> B{下}");
        let svg = diagram.render_svg(
            hsla(0.0, 0.0, 1.0, 1.0),
            hsla(0.0, 0.0, 0.5, 1.0),
            hsla(0.6, 1.0, 0.5, 1.0),
        );
        assert!(svg.contains("<polygon"));
        assert!(svg.contains("marker-end"));
        assert!(svg.contains("上") && svg.contains("下"));
    }
}
