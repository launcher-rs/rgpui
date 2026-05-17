use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

pub fn derive_into_plot(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let type_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics rgpui::IntoElement for #type_name #type_generics #where_clause {
            type Element = Self;

            fn into_element(self) -> Self::Element {
                self
            }
        }

        impl #impl_generics rgpui::Element for #type_name #type_generics #where_clause {
            type RequestLayoutState = ();
            type PrepaintState = ();

            fn id(&self) -> Option<rgpui::ElementId> {
                None
            }

            fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
                None
            }

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
