//! SVG 路径渲染组件：解析 SVG path 数据（M/L/H/V/C/Q/Z 命令），通过 PathBuilder 绘制。

use rgpui::*;

/// SVG 路径命令。
#[derive(Clone)]
enum SvgCommand {
    /// M 移动到。
    MoveTo(f32, f32),
    /// L 直线。
    LineTo(f32, f32),
    /// C 三次贝塞尔曲线。
    CurveTo(f32, f32, f32, f32, f32, f32),
    /// Q 二次贝塞尔曲线。
    QuadTo(f32, f32, f32, f32),
    /// Z 闭合路径。
    Close,
}

/// 解析 SVG path 数据为命令序列。
fn parse_svg_path(data: &str) -> Vec<SvgCommand> {
    let mut commands = Vec::new();
    let mut chars = data.chars().peekable();
    let mut current_cmd = ' ';

    /// 跳过空白字符与逗号。
    fn skip_ws_and_commas(chars: &mut std::iter::Peekable<std::str::Chars>) {
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == ',' {
                chars.next();
            } else {
                break;
            }
        }
    }

    /// 解析一个浮点数。
    fn parse_number(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<f32> {
        skip_ws_and_commas(chars);
        let mut s = String::new();
        if let Some(&c) = chars.peek() {
            if c == '-' || c == '+' {
                s.push(c);
                chars.next();
            }
        }
        let mut has_dot = false;
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                chars.next();
            } else if c == '.' && !has_dot {
                has_dot = true;
                s.push(c);
                chars.next();
            } else {
                break;
            }
        }
        if s.is_empty() || s == "-" || s == "+" {
            None
        } else {
            s.parse().ok()
        }
    }

    while chars.peek().is_some() {
        skip_ws_and_commas(&mut chars);
        if let Some(&c) = chars.peek() {
            if c.is_ascii_alphabetic() {
                current_cmd = c;
                chars.next();
            }
        }

        match current_cmd {
            'M' => {
                if let (Some(x), Some(y)) = (parse_number(&mut chars), parse_number(&mut chars)) {
                    commands.push(SvgCommand::MoveTo(x, y));
                    current_cmd = 'L';
                } else {
                    break;
                }
            }
            'L' => {
                if let (Some(x), Some(y)) = (parse_number(&mut chars), parse_number(&mut chars)) {
                    commands.push(SvgCommand::LineTo(x, y));
                } else {
                    break;
                }
            }
            'H' => {
                if let Some(x) = parse_number(&mut chars) {
                    commands.push(SvgCommand::LineTo(x, f32::NAN));
                } else {
                    break;
                }
            }
            'V' => {
                if let Some(y) = parse_number(&mut chars) {
                    commands.push(SvgCommand::LineTo(f32::NAN, y));
                } else {
                    break;
                }
            }
            'C' => {
                if let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                ) {
                    commands.push(SvgCommand::CurveTo(x1, y1, x2, y2, x, y));
                } else {
                    break;
                }
            }
            'Q' => {
                if let (Some(cx1), Some(cy1), Some(x), Some(y)) = (
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                ) {
                    commands.push(SvgCommand::QuadTo(cx1, cy1, x, y));
                } else {
                    break;
                }
            }
            'Z' | 'z' => {
                commands.push(SvgCommand::Close);
                current_cmd = 'M';
            }
            _ => {
                chars.next();
            }
        }
    }

    commands
}

/// SVG 绘制所需的数据。
#[derive(Clone)]
struct SvgPaintData {
    /// 命令序列。
    commands: Vec<SvgCommand>,
    /// 视口范围。
    view_box: Bounds<f32>,
    /// 填充颜色。
    fill_color: Hsla,
    /// 描边颜色。
    stroke_color: Option<Hsla>,
    /// 描边宽度。
    stroke_width: f32,
}

/// SVG 路径渲染组件：将 path 数据按视口等比缩放绘制。
#[derive(IntoElement)]
pub struct SVGRenderer {
    /// SVG path 数据字符串。
    path_data: SharedString,
    /// 视口范围。
    view_box: Bounds<f32>,
    /// 填充颜色（默认主题前景色）。
    fill_color: Option<Hsla>,
    /// 描边颜色。
    stroke_color: Option<Hsla>,
    /// 描边宽度。
    stroke_width: f32,
    /// 用户样式。
    style: StyleRefinement,
}

impl SVGRenderer {
    /// 创建 SVG 渲染组件，默认视口 100x100、无描边。
    pub fn new() -> Self {
        Self {
            path_data: SharedString::default(),
            view_box: Bounds::new(point(0.0_f32, 0.0_f32), size(100.0_f32, 100.0_f32)),
            fill_color: None,
            stroke_color: None,
            stroke_width: 1.0,
            style: StyleRefinement::default(),
        }
    }

    /// 设置 SVG path 数据。
    pub fn path_data(mut self, data: impl Into<SharedString>) -> Self {
        self.path_data = data.into();
        self
    }

    /// 设置视口范围（x, y, 宽, 高）。
    pub fn view_box(mut self, x: f32, y: f32, w: f32, h: f32) -> Self {
        self.view_box = Bounds::new(point(x, y), size(w, h));
        self
    }

    /// 设置填充颜色。
    pub fn fill(mut self, color: Hsla) -> Self {
        self.fill_color = Some(color);
        self
    }

    /// 设置描边颜色。
    pub fn stroke(mut self, color: Hsla) -> Self {
        self.stroke_color = Some(color);
        self
    }

    /// 设置描边宽度。
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }
}

impl Styled for SVGRenderer {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// 将视口坐标映射到元素内的实际像素坐标（等比缩放并居中）。
fn transform_point(
    vx: f32,
    vy: f32,
    view_box: &Bounds<f32>,
    bounds: &Bounds<Pixels>,
) -> Point<Pixels> {
    let scale_x = bounds.size.width / px(1.0) / view_box.size.width;
    let scale_y = bounds.size.height / px(1.0) / view_box.size.height;
    let scale = scale_x.min(scale_y);

    let offset_x = (bounds.size.width / px(1.0) - view_box.size.width * scale) * 0.5;
    let offset_y = (bounds.size.height / px(1.0) - view_box.size.height * scale) * 0.5;

    point(
        bounds.left() + px(offset_x + (vx - view_box.origin.x) * scale),
        bounds.top() + px(offset_y + (vy - view_box.origin.y) * scale),
    )
}

impl RenderOnce for SVGRenderer {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;

        let fill_color = self.fill_color.unwrap_or(*theme.tokens.foreground);
        let commands = parse_svg_path(&self.path_data);

        let paint_data = SvgPaintData {
            commands,
            view_box: self.view_box,
            fill_color,
            stroke_color: self.stroke_color,
            stroke_width: self.stroke_width,
        };

        let mut root = div().size_full().child(
            canvas(
                move |_, _, _| paint_data,
                move |bounds, data, window, _cx| {
                    if data.commands.is_empty() {
                        return;
                    }

                    let mut current_x = 0.0_f32;
                    let mut current_y = 0.0_f32;

                    if let Some(stroke_color) = data.stroke_color {
                        let mut builder = PathBuilder::stroke(px(data.stroke_width));
                        let mut started = false;

                        for cmd in &data.commands {
                            match cmd {
                                SvgCommand::MoveTo(x, y) => {
                                    let pt = transform_point(*x, *y, &data.view_box, &bounds);
                                    if started {
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, stroke_color);
                                        }
                                        builder = PathBuilder::stroke(px(data.stroke_width));
                                    }
                                    builder.move_to(pt);
                                    current_x = *x;
                                    current_y = *y;
                                    started = true;
                                }
                                SvgCommand::LineTo(x, y) => {
                                    let fx = if x.is_nan() { current_x } else { *x };
                                    let fy = if y.is_nan() { current_y } else { *y };
                                    let pt = transform_point(fx, fy, &data.view_box, &bounds);
                                    builder.line_to(pt);
                                    current_x = fx;
                                    current_y = fy;
                                }
                                SvgCommand::CurveTo(x1, y1, x2, y2, x, y) => {
                                    let _cp1 = transform_point(*x1, *y1, &data.view_box, &bounds);
                                    let cp2 = transform_point(*x2, *y2, &data.view_box, &bounds);
                                    let end = transform_point(*x, *y, &data.view_box, &bounds);
                                    builder.curve_to(cp2, end);
                                    current_x = *x;
                                    current_y = *y;
                                }
                                SvgCommand::QuadTo(cx1, cy1, x, y) => {
                                    let cp = transform_point(*cx1, *cy1, &data.view_box, &bounds);
                                    let end = transform_point(*x, *y, &data.view_box, &bounds);
                                    builder.curve_to(cp, end);
                                    current_x = *x;
                                    current_y = *y;
                                }
                                SvgCommand::Close => {
                                    builder.close();
                                }
                            }
                        }

                        if started {
                            if let Ok(path) = builder.build() {
                                window.paint_path(path, stroke_color);
                            }
                        }
                    }

                    {
                        let mut builder = PathBuilder::fill();
                        let mut started = false;
                        current_x = 0.0;
                        current_y = 0.0;

                        for cmd in &data.commands {
                            match cmd {
                                SvgCommand::MoveTo(x, y) => {
                                    if started {
                                        builder.close();
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, data.fill_color);
                                        }
                                        builder = PathBuilder::fill();
                                    }
                                    let pt = transform_point(*x, *y, &data.view_box, &bounds);
                                    builder.move_to(pt);
                                    current_x = *x;
                                    current_y = *y;
                                    started = true;
                                }
                                SvgCommand::LineTo(x, y) => {
                                    let fx = if x.is_nan() { current_x } else { *x };
                                    let fy = if y.is_nan() { current_y } else { *y };
                                    let pt = transform_point(fx, fy, &data.view_box, &bounds);
                                    builder.line_to(pt);
                                    current_x = fx;
                                    current_y = fy;
                                }
                                SvgCommand::CurveTo(x1, y1, x2, y2, x, y) => {
                                    let _cp1 = transform_point(*x1, *y1, &data.view_box, &bounds);
                                    let cp2 = transform_point(*x2, *y2, &data.view_box, &bounds);
                                    let end = transform_point(*x, *y, &data.view_box, &bounds);
                                    builder.curve_to(cp2, end);
                                    current_x = *x;
                                    current_y = *y;
                                }
                                SvgCommand::QuadTo(cx1, cy1, x, y) => {
                                    let cp = transform_point(*cx1, *cy1, &data.view_box, &bounds);
                                    let end = transform_point(*x, *y, &data.view_box, &bounds);
                                    builder.curve_to(cp, end);
                                    current_x = *x;
                                    current_y = *y;
                                }
                                SvgCommand::Close => {
                                    builder.close();
                                }
                            }
                        }

                        if started {
                            builder.close();
                            if let Ok(path) = builder.build() {
                                window.paint_path(path, data.fill_color);
                            }
                        }
                    }
                },
            )
            .size_full(),
        );
        root.style().refine(&user_style);
        root
    }
}
