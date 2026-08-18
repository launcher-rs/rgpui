//! QR 码组件：将任意文本编码为二维码，并以彩色方块在画布上绘制。

use qrcode::{Color, QrCode, types::EcLevel};
use rgpui::*;

/// QR 码绘制的预计算数据。
#[derive(Clone)]
struct QRPaintData {
    /// 二维码模块矩阵（true 表示深色模块）。
    modules: Vec<Vec<bool>>,
    /// 前景颜色。
    fg_color: Hsla,
    /// 背景颜色。
    bg_color: Hsla,
}

/// QR 码组件。
#[derive(IntoElement)]
pub struct QRCodeComponent {
    /// 需要编码的数据。
    data: SharedString,
    /// 整体尺寸。
    size: Pixels,
    /// 前景颜色（默认取主题前景色）。
    fg_color: Option<Hsla>,
    /// 背景颜色（默认取主题背景色）。
    bg_color: Option<Hsla>,
    /// 纠错级别。
    error_correction: EcLevel,
    /// 用户样式。
    style: StyleRefinement,
}

impl QRCodeComponent {
    /// 创建 QR 码组件，默认尺寸 200px、纠错级别 M。
    pub fn new(data: impl Into<SharedString>) -> Self {
        Self {
            data: data.into(),
            size: px(200.0),
            fg_color: None,
            bg_color: None,
            error_correction: EcLevel::M,
            style: StyleRefinement::default(),
        }
    }

    /// 设置整体尺寸。
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// 设置前景颜色。
    pub fn fg_color(mut self, color: Hsla) -> Self {
        self.fg_color = Some(color);
        self
    }

    /// 设置背景颜色。
    pub fn bg_color(mut self, color: Hsla) -> Self {
        self.bg_color = Some(color);
        self
    }

    /// 设置纠错级别（L / M / Q / H，级别越高越抗污损、图案越密）。
    pub fn error_correction(mut self, level: EcLevel) -> Self {
        self.error_correction = level;
        self
    }
}

impl Styled for QRCodeComponent {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// 根据文本与纠错级别生成二维码模块矩阵。
///
/// 编码失败时返回 21x21 的空白矩阵（避免渲染崩溃）。
fn generate_modules(data: &str, ec_level: EcLevel) -> Vec<Vec<bool>> {
    match QrCode::with_error_correction_level(data, ec_level) {
        Ok(code) => {
            let width = code.width();
            let colors = code.into_colors();
            let mut grid = Vec::with_capacity(width);
            for row in 0..width {
                let mut row_vec = Vec::with_capacity(width);
                for col in 0..width {
                    let idx = row * width + col;
                    row_vec.push(colors[idx] == Color::Dark);
                }
                grid.push(row_vec);
            }
            grid
        }
        Err(_) => vec![vec![false; 21]; 21],
    }
}

impl RenderOnce for QRCodeComponent {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;

        let fg = self.fg_color.unwrap_or(*theme.tokens.foreground);
        let bg = self.bg_color.unwrap_or(*theme.tokens.background);

        let modules = generate_modules(&self.data, self.error_correction);
        let paint_data = QRPaintData {
            modules,
            fg_color: fg,
            bg_color: bg,
        };

        let qr_size = self.size;

        let mut root = div().w(qr_size).h(qr_size).child(
            canvas(
                move |_, _, _| paint_data,
                move |bounds, data, window, _cx| {
                    if data.modules.is_empty() {
                        return;
                    }

                    let module_count = data.modules.len();
                    let module_size_w = bounds.size.width / px(1.0) / module_count as f32;
                    let module_size_h = bounds.size.height / px(1.0) / module_count as f32;
                    let module_size = module_size_w.min(module_size_h);

                    let total_w = module_size * module_count as f32;
                    let total_h = module_size * module_count as f32;
                    let offset_x = (bounds.size.width / px(1.0) - total_w) * 0.5;
                    let offset_y = (bounds.size.height / px(1.0) - total_h) * 0.5;

                    window.paint_quad(fill(bounds, data.bg_color));

                    for (row_idx, row) in data.modules.iter().enumerate() {
                        for (col_idx, &is_dark) in row.iter().enumerate() {
                            if is_dark {
                                let x = bounds.left() + px(offset_x + col_idx as f32 * module_size);
                                let y = bounds.top() + px(offset_y + row_idx as f32 * module_size);
                                let cell_bounds = Bounds::new(
                                    point(x, y),
                                    size(px(module_size), px(module_size)),
                                );
                                window.paint_quad(fill(cell_bounds, data.fg_color));
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
