#[test]
fn test_derive_context() {
    use rgpui::{App, Window};
    use rgpui_macros::{AppContext, VisualContext};

    #[derive(AppContext, VisualContext)]
    struct _MyCustomContext<'a, 'b> {
        #[app]
        app: &'a mut App,
        #[window]
        window: &'b mut Window,
    }
}
