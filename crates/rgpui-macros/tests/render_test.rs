#[test]
fn test_derive_render() {
    use rgpui_macros::Render;

    #[derive(Render)]
    struct _Element;
}
