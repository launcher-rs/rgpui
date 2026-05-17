//! 实现 `#[derive_inspector_reflection]` 宏，为具有 `fn method(self) -> Self` 形状的特质方法提供运行时访问能力。
//! 此代码使用 Zed Agent 与 Claude Opus 4 生成。

use heck::ToSnakeCase as _;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    Attribute, Expr, FnArg, Ident, Item, ItemTrait, Lit, Meta, Path, ReturnType, TraitItem, Type,
    parse_macro_input, parse_quote,
    visit_mut::{self, VisitMut},
};

pub fn derive_inspector_reflection(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(input as Item);

    // 首先展开特质中的任何宏
    match &mut item {
        Item::Trait(trait_item) => {
            let mut expander = MacroExpander;
            expander.visit_item_trait_mut(trait_item);
        }
        _ => {
            return syn::Error::new_spanned(
                quote!(#item),
                "#[derive_inspector_reflection] can only be applied to traits",
            )
            .to_compile_error()
            .into();
        }
    }

    // 现在处理展开后的特质
    match item {
        Item::Trait(trait_item) => generate_reflected_trait(trait_item),
        _ => unreachable!(),
    }
}

fn generate_reflected_trait(trait_item: ItemTrait) -> TokenStream {
    let trait_name = &trait_item.ident;
    let vis = &trait_item.vis;

    // 确定是否从 gpui crate 内部调用
    let call_site = Span::call_site();
    let inspector_reflection_path = if is_called_from_gpui_crate(call_site) {
        quote! { crate::inspector_reflection }
    } else {
        quote! { ::rgpui::inspector_reflection }
    };

    // 收集形如 fn name(self) -> Self 或 fn name(mut self) -> Self 的方法信息
    let mut method_infos = Vec::new();

    for item in &trait_item.items {
        if let TraitItem::Fn(method) = item {
            let method_name = &method.sig.ident;

            // 检查方法是否具有 self 或 mut self 接收器
            let has_valid_self_receiver = method
                .sig
                .inputs
                .iter()
                .any(|arg| matches!(arg, FnArg::Receiver(r) if r.reference.is_none()));

            // 检查方法是否返回 Self
            let returns_self = match &method.sig.output {
                ReturnType::Type(_, ty) => {
                    matches!(**ty, Type::Path(ref path) if path.path.is_ident("Self"))
                }
                ReturnType::Default => false,
            };

            // 检查方法是否只有一个参数（self 或 mut self）
            let param_count = method.sig.inputs.len();

            // 包含形如 fn name(self) -> Self 或 fn name(mut self) -> Self 的方法
            // 这包括具有默认实现的方法
            if has_valid_self_receiver && returns_self && param_count == 1 {
                // 提取文档注释和 cfg 属性
                let doc = extract_doc_comment(&method.attrs);
                let cfg_attrs = extract_cfg_attributes(&method.attrs);
                method_infos.push((method_name.clone(), doc, cfg_attrs));
            }
        }
    }

    // 生成反射模块名称
    let reflection_mod_name = Ident::new(
        &format!("{}_reflection", trait_name.to_string().to_snake_case()),
        trait_name.span(),
    );

    // 为每个方法生成包装函数
    // 这些包装函数使用类型擦除来允许运行时调用
    let wrapper_functions = method_infos.iter().map(|(method_name, _doc, cfg_attrs)| {
        let wrapper_name = Ident::new(
            &format!("__wrapper_{}", method_name),
            method_name.span(),
        );
        quote! {
            #(#cfg_attrs)*
            fn #wrapper_name<T: #trait_name + 'static>(value: Box<dyn std::any::Any>) -> Box<dyn std::any::Any> {
                if let Ok(concrete) = value.downcast::<T>() {
                    Box::new(concrete.#method_name())
                } else {
                    panic!("Type mismatch in reflection wrapper");
                }
            }
        }
    });

    // 生成方法信息条目
    let method_info_entries = method_infos.iter().map(|(method_name, doc, cfg_attrs)| {
        let method_name_str = method_name.to_string();
        let wrapper_name = Ident::new(&format!("__wrapper_{}", method_name), method_name.span());
        let doc_expr = match doc {
            Some(doc_str) => quote! { Some(#doc_str) },
            None => quote! { None },
        };
        quote! {
            #(#cfg_attrs)*
            #inspector_reflection_path::FunctionReflection {
                name: #method_name_str,
                function: #wrapper_name::<T>,
                documentation: #doc_expr,
                _type: ::std::marker::PhantomData,
            }
        }
    });

    // 生成完整输出
    let output = quote! {
        #trait_item

        /// 实现函数反射
        #vis mod #reflection_mod_name {
            use super::*;

            #(#wrapper_functions)*

            /// 获取实现该特质的具体类型的所有可反射方法
            pub fn methods<T: #trait_name + 'static>() -> Vec<#inspector_reflection_path::FunctionReflection<T>> {
                vec![
                    #(#method_info_entries),*
                ]
            }

            /// 按名称查找实现该特质的具体类型的方法
            pub fn find_method<T: #trait_name + 'static>(name: &str) -> Option<#inspector_reflection_path::FunctionReflection<T>> {
                methods::<T>().into_iter().find(|m| m.name == name)
            }
        }
    };

    TokenStream::from(output)
}

fn extract_doc_comment(attrs: &[Attribute]) -> Option<String> {
    let mut doc_lines = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("doc")
            && let Meta::NameValue(meta) = &attr.meta
            && let Expr::Lit(expr_lit) = &meta.value
            && let Lit::Str(lit_str) = &expr_lit.lit
        {
            let line = lit_str.value();
            let line = line.strip_prefix(' ').unwrap_or(&line);
            doc_lines.push(line.to_string());
        }
    }

    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join("\n"))
    }
}

fn extract_cfg_attributes(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .cloned()
        .collect()
}

fn is_called_from_gpui_crate(_span: Span) -> bool {
    // 通过检查调用站点来确定是否从 gpui crate 内部调用
    // 这是一种启发式方法 - 我们检查当前 crate 名称是否为 "gpui"
    std::env::var("CARGO_PKG_NAME").is_ok_and(|name| name == "gpui")
}

struct MacroExpander;

impl VisitMut for MacroExpander {
    fn visit_item_trait_mut(&mut self, trait_item: &mut ItemTrait) {
        let mut expanded_items = Vec::new();
        let mut items_to_keep = Vec::new();

        for item in trait_item.items.drain(..) {
            match item {
                TraitItem::Macro(macro_item) => {
                    // 尝试展开已知宏
                    if let Some(expanded) = try_expand_macro(&macro_item) {
                        expanded_items.extend(expanded);
                    } else {
                        // 保留未知宏不变
                        items_to_keep.push(TraitItem::Macro(macro_item));
                    }
                }
                other => {
                    items_to_keep.push(other);
                }
            }
        }

        // 用展开的内容重建项目列表，然后是原始项目
        trait_item.items = expanded_items;
        trait_item.items.extend(items_to_keep);

        // 继续访问
        visit_mut::visit_item_trait_mut(self, trait_item);
    }
}

fn try_expand_macro(macro_item: &syn::TraitItemMacro) -> Option<Vec<TraitItem>> {
    let path = &macro_item.mac.path;

    // 检查这是否是我们已知的样式宏之一
    let macro_name = path_to_string(path);

    // 处理已知宏，调用它们的实现
    match macro_name.as_str() {
        "rgpui_macros::style_helpers" | "style_helpers" => {
            let tokens = macro_item.mac.tokens.clone();
            let expanded = crate::styles::style_helpers(TokenStream::from(tokens));
            parse_expanded_items(expanded)
        }
        "rgpui_macros::visibility_style_methods" | "visibility_style_methods" => {
            let tokens = macro_item.mac.tokens.clone();
            let expanded = crate::styles::visibility_style_methods(TokenStream::from(tokens));
            parse_expanded_items(expanded)
        }
        "rgpui_macros::margin_style_methods" | "margin_style_methods" => {
            let tokens = macro_item.mac.tokens.clone();
            let expanded = crate::styles::margin_style_methods(TokenStream::from(tokens));
            parse_expanded_items(expanded)
        }
        "rgpui_macros::padding_style_methods" | "padding_style_methods" => {
            let tokens = macro_item.mac.tokens.clone();
            let expanded = crate::styles::padding_style_methods(TokenStream::from(tokens));
            parse_expanded_items(expanded)
        }
        "rgpui_macros::position_style_methods" | "position_style_methods" => {
            let tokens = macro_item.mac.tokens.clone();
            let expanded = crate::styles::position_style_methods(TokenStream::from(tokens));
            parse_expanded_items(expanded)
        }
        "rgpui_macros::overflow_style_methods" | "overflow_style_methods" => {
            let tokens = macro_item.mac.tokens.clone();
            let expanded = crate::styles::overflow_style_methods(TokenStream::from(tokens));
            parse_expanded_items(expanded)
        }
        "rgpui_macros::cursor_style_methods" | "cursor_style_methods" => {
            let tokens = macro_item.mac.tokens.clone();
            let expanded = crate::styles::cursor_style_methods(TokenStream::from(tokens));
            parse_expanded_items(expanded)
        }
        "rgpui_macros::border_style_methods" | "border_style_methods" => {
            let tokens = macro_item.mac.tokens.clone();
            let expanded = crate::styles::border_style_methods(TokenStream::from(tokens));
            parse_expanded_items(expanded)
        }
        "rgpui_macros::box_shadow_style_methods" | "box_shadow_style_methods" => {
            let tokens = macro_item.mac.tokens.clone();
            let expanded = crate::styles::box_shadow_style_methods(TokenStream::from(tokens));
            parse_expanded_items(expanded)
        }
        _ => None,
    }
}

fn path_to_string(path: &Path) -> String {
    path.segments
        .iter()
        .map(|seg| seg.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn parse_expanded_items(expanded: TokenStream) -> Option<Vec<TraitItem>> {
    let tokens = TokenStream2::from(expanded);

    // 尝试将展开后的 token 解析为特质项目
    // 我们需要将它们包装在一个虚拟特质中才能正确解析
    let dummy_trait: ItemTrait = parse_quote! {
        trait Dummy {
            #tokens
        }
    };

    Some(dummy_trait.items)
}
