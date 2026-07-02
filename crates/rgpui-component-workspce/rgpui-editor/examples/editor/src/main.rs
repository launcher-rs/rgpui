use rgpui::*;
use rgpui_component::{ActiveTheme as _, Root, v_flex};
use rgpui_component_assets::Assets;
use rgpui_editor::{Editor, EditorState};

const INITIAL_CODE: &str = r#"use rgpui::*;
use rgpui_component::{Root, v_flex};

struct Counter {
    value: i32,
}

impl Counter {
    fn increment(&mut self) {
        self.value += 1;
    }
}

fn main() {
    println!("rgpui-editor example");
}
"#;

pub struct Example {
    editor: Entity<EditorState>,
}

impl Example {
    /// 创建编辑器示例视图。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let mut state = EditorState::new(window, cx)
                .code_editor("rust")
                .line_number(true)
                .searchable(true)
                .soft_wrap(false)
                .placeholder("Write Rust code...");

            state.set_value(INITIAL_CODE, window, cx);
            state
        });

        Self { editor }
    }
}

impl Render for Example {
    /// 渲染编辑器示例界面。
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .p_4()
            .gap_3()
            .child(
                v_flex()
                    .gap_1()
                    .child("rgpui-editor")
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD),
            )
            .child(
                Editor::new(&self.editor)
                    .h_full()
                    .appearance(true)
                    .bordered(true)
                    .focus_bordered(true)
                    .rounded_lg(),
            )
    }
}

/// 启动 rgpui-editor 示例应用。
fn main() {
    let app = rgpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        rgpui_component::init(cx);
        rgpui_editor::init(cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(960.), px(720.)), cx)),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|cx| Example::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
            })
            .expect("failed to open rgpui-editor example window");
        })
        .detach();
    });
}
