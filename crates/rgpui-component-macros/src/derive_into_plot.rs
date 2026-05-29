use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

/// 为图表类型派生 `IntoPlot` 宏的实现
///
/// 该宏为标注了 `#[derive(IntoPlot)]` 的类型实现 `IntoElement` 和 `Element` trait，
/// 使其可以作为 GPUI 元素使用，并在 `paint` 阶段调用 `Plot::paint` 进行渲染。
pub fn derive_into_plot(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let type_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();

    let expanded = quote! {
        /// 实现 `IntoElement` trait，使图表类型可直接作为元素使用
        impl #impl_generics rgpui::IntoElement for #type_name #type_generics #where_clause {
            type Element = Self;

            fn into_element(self) -> Self::Element {
                self
            }
        }

        /// 实现 `Element` trait，提供图表的布局和渲染逻辑
        impl #impl_generics rgpui::Element for #type_name #type_generics #where_clause {
            type RequestLayoutState = ();
            type PrepaintState = ();

            /// 返回元素 ID（图表元素不需要）
            fn id(&self) -> Option<rgpui::ElementId> {
                None
            }

            /// 返回源代码位置信息（图表元素不需要）
            fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
                None
            }

            /// 请求布局，使用全尺寸样式
            fn request_layout(
                &mut self,
                _: Option<&rgpui::GlobalElementId>,
                _: Option<&rgpui::InspectorElementId>,
                window: &mut rgpui::Window,
                cx: &mut rgpui::App,
            ) -> (rgpui::LayoutId, Self::RequestLayoutState) {
                let style = rgpui::Style {
                    size: rgpui::Size::full(),
                    ..Default::default()
                };

                (window.request_layout(style, None, cx), ())
            }

            /// 预绘制阶段（图表不需要额外处理）
            fn prepaint(
                &mut self,
                _: Option<&rgpui::GlobalElementId>,
                _: Option<&rgpui::InspectorElementId>,
                _: rgpui::Bounds<rgpui::Pixels>,
                _: &mut Self::RequestLayoutState,
                _: &mut rgpui::Window,
                _: &mut rgpui::App,
            ) -> Self::PrepaintState {
            }

            /// 绘制阶段，委托给 `Plot::paint` 进行实际渲染
            fn paint(
                &mut self,
                _: Option<&rgpui::GlobalElementId>,
                _: Option<&rgpui::InspectorElementId>,
                bounds: rgpui::Bounds<rgpui::Pixels>,
                _: &mut Self::RequestLayoutState,
                _: &mut Self::PrepaintState,
                window: &mut rgpui::Window,
                cx: &mut rgpui::App,
            ) {
                <Self as Plot>::paint(self, bounds, window, cx)
            }
        }
    };

    TokenStream::from(expanded)
}
