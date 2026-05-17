use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

/// 为类型生成 `rgpui::Render` 特质实现。
///
/// 该宏提供一个默认的渲染实现，返回 `rgpui::Empty` 作为空元素。
/// 这是一个内部使用的派生宏（标记为 `#[doc(hidden)]`），通常不直接使用。
///
/// # 生成的实现
///
/// - `render()` - 返回 `rgpui::Empty` 作为默认渲染内容
pub fn derive_render(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let type_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();

    let r#gen = quote! {
        impl #impl_generics rgpui::Render for #type_name #type_generics
        #where_clause
        {
            fn render(&mut self, _window: &mut rgpui::Window, _cx: &mut rgpui::Context<Self>) -> impl rgpui::Element {
                rgpui::Empty
            }
        }
    };

    r#gen.into()
}
