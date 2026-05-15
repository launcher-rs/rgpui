use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// 为类型生成 `gpui::IntoElement` 特质实现。
///
/// 该宏将类型包装为 `gpui::Component<Self>`，使其可以作为 UI 元素使用。
/// 通常用于实现了 `RenderOnce` 特质的类型。
///
/// # 生成的实现
///
/// - `type Element = gpui::Component<Self>` - 关联类型定义为 Component
/// - `into_element()` - 将自身转换为 Component 元素
pub fn derive_into_element(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let type_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();

    let r#gen = quote! {
        impl #impl_generics gpui::IntoElement for #type_name #type_generics
        #where_clause
        {
            type Element = gpui::Component<Self>;

            #[track_caller]
            fn into_element(self) -> Self::Element {
                gpui::Component::new(self)
            }
        }
    };

    r#gen.into()
}
