use rgpui::*;
use rgpui_component::{
    ActiveTheme as _,
    resizable::h_resizable,
    text::html,
};
use rgpui_component_assets::Assets;
use rgpui_editor::highlighter::Language;
use rgpui_editor::input::TabSize;
use rgpui_editor::{Editor, EditorEvent, EditorState};

pub struct Example {
    edotpr_state: Entity<EditorState>,
    _subscribe: Subscription,
}

const EXAMPLE: &str = include_str!("./fixtures/test.html");

impl Example {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor_state = cx.new(|cx| {
            EditorState::new(window, cx)
                .code_editor(Language::Html)
                .tab_size(TabSize {
                    tab_size: 4,
                    hard_tabs: false,
                })
                .default_value(EXAMPLE)
                .placeholder("Enter your HTML here...")
        });

        let _subscribe = cx.subscribe(
            &editor_state,
            |_, _, _: &EditorEvent, cx| {
                cx.notify();
            },
        );

        Self {
            edotpr_state: editor_state,
            _subscribe,
        }
    }

    fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl Render for Example {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_resizable("container")
            .child(
                div()
                    .id("source")
                    .size_full()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(cx.theme().mono_font_size)
                    .child(
                        Editor::new(&self.edotpr_state)
                            .h_full()
                            .appearance(false)
                            .focus_bordered(false),
                    )
                    .into_any(),
            )
            .child(
                html(self.edotpr_state.read(cx).value().clone())
                    .p_5()
                    .scrollable(true)
                    .selectable(true)
                    .into_any(),
            )
    }
}

fn main() {
    let app = rgpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        rgpui_component_story::init(cx);
        cx.activate(true);

        rgpui_component_story::create_new_window("HTML Render (native)", Example::view, cx);
    });
}
