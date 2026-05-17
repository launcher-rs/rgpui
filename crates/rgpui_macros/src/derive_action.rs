use crate::register_action::generate_register_action;
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{parse::ParseStream, Data, DeriveInput, LitStr, Token};

/// 为类型生成 `rgpui::Action` 特质实现的过程宏核心逻辑。
///
/// 该函数解析 `#[action(...)]` 属性，支持以下配置项：
/// - `name` - 指定动作名称
/// - `namespace` - 指定动作命名空间
/// - `no_json` - 禁用 JSON 序列化
/// - `no_register` - 禁用自动注册
/// - `deprecated_aliases` - 已弃用的别名列表
/// - `deprecated` - 弃用提示信息
///
/// 同时会收集类型的文档注释（`#[doc]` 属性），用于生成动作的文档说明。
///
/// # 生成的实现
///
/// 生成的 `Action` 特质实现包括：
/// - `name()` / `name_for_type()` - 返回动作的全限定名称
/// - `partial_eq()` - 动作相等性比较
/// - `boxed_clone()` - 克隆动作
/// - `build()` - 从 JSON 值构建动作（除非设置了 `no_json`）
/// - `action_json_schema()` - 生成 JSON Schema
/// - `deprecated_aliases()` - 返回已弃用别名列表
/// - `deprecation_message()` - 返回弃用提示信息
/// - `documentation()` - 返回文档说明
pub(crate) fn derive_action(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);

    let struct_name = &input.ident;
    let mut name_argument = None;
    let mut deprecated_aliases = Vec::new();
    let mut no_json = false;
    let mut no_register = false;
    let mut namespace = None;
    let mut deprecated = None;
    let mut doc_str: Option<String> = None;

/*
*
* #[action()]
* struct Foo {
*  bar: bool // bar 是否被视为属性
}
*/
    // 解析 #[action(...)] 属性中的各个配置项
    for attr in &input.attrs {
        if attr.path().is_ident("action") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    if name_argument.is_some() {
                        return Err(meta.error("'name' argument specified multiple times"));
                    }
                    meta.input.parse::<Token![=]>()?;
                    let lit: LitStr = meta.input.parse()?;
                    name_argument = Some(lit.value());
                } else if meta.path.is_ident("namespace") {
                    if namespace.is_some() {
                        return Err(meta.error("'namespace' argument specified multiple times"));
                    }
                    meta.input.parse::<Token![=]>()?;
                    let ident: Ident = meta.input.parse()?;
                    namespace = Some(ident.to_string());
                } else if meta.path.is_ident("no_json") {
                    if no_json {
                        return Err(meta.error("'no_json' argument specified multiple times"));
                    }
                    no_json = true;
                } else if meta.path.is_ident("no_register") {
                    if no_register {
                        return Err(meta.error("'no_register' argument specified multiple times"));
                    }
                    no_register = true;
                } else if meta.path.is_ident("deprecated_aliases") {
                    if !deprecated_aliases.is_empty() {
                        return Err(
                            meta.error("'deprecated_aliases' argument specified multiple times")
                        );
                    }
                    meta.input.parse::<Token![=]>()?;
                    // 解析字符串字面量数组
                    let content;
                    syn::bracketed!(content in meta.input);
                    let aliases = content.parse_terminated(
                        |input: ParseStream| input.parse::<LitStr>(),
                        Token![,],
                    )?;
                    deprecated_aliases.extend(aliases.into_iter().map(|lit| lit.value()));
                } else if meta.path.is_ident("deprecated") {
                    if deprecated.is_some() {
                        return Err(meta.error("'deprecated' argument specified multiple times"));
                    }
                    meta.input.parse::<Token![=]>()?;
                    let lit: LitStr = meta.input.parse()?;
                    deprecated = Some(lit.value());
                } else {
                    return Err(meta.error(format!(
                        "'{:?}' argument not recognized, expected \
                        'namespace', 'no_json', 'no_register, 'deprecated_aliases', or 'deprecated'",
                        meta.path
                    )));
                }
                Ok(())
            })
            .unwrap_or_else(|e| panic!("in #[action] attribute: {}", e));
        } else if attr.path().is_ident("doc") {
            // 收集文档注释，用于生成动作的文档说明
            use syn::{Expr::Lit, ExprLit, Lit::Str, Meta, MetaNameValue};
            if let Meta::NameValue(MetaNameValue {
                value:
                    Lit(ExprLit {
                        lit: Str(ref lit_str),
                        ..
                    }),
                ..
            }) = attr.meta
            {
                let doc = lit_str.value();
                let doc_str = doc_str.get_or_insert_default();
                doc_str.push_str(doc.trim());
                doc_str.push('\n');
            }
        }
    }

    // 如果未指定名称，则使用结构体名称
    let name = name_argument.unwrap_or_else(|| struct_name.to_string());

    // 名称不能包含 "::"，应使用 namespace 属性代替

    if name.contains("::") {
        panic!(
            "in #[action] attribute: `name = \"{name}\"` must not contain `::`, \
            also specify `namespace` instead"
        );
    }

    // 构建完整名称（包含命名空间）
    let full_name = if let Some(namespace) = namespace {
        format!("{namespace}::{name}")
    } else {
        name
    };

    // 检查是否为单元结构体（无字段），单元结构体的 JSON 构建逻辑不同
    let is_unit_struct = matches!(&input.data, Data::Struct(data) if data.fields.is_empty());

    // 根据 no_json 和是否为单元结构体生成不同的 build 函数体

    let build_fn_body = if no_json {
        let error_msg = format!("{} cannot be built from JSON", full_name);
        quote! { Err(rgpui::private::anyhow::anyhow!(#error_msg)) }
    } else if is_unit_struct {
        quote! { Ok(Box::new(Self)) }
    } else {
        quote! { Ok(Box::new(rgpui::private::serde_json::from_value::<Self>(_value)?)) }
    };

    // 生成 JSON Schema 函数体（单元结构体和 no_json 不生成 schema）
    let json_schema_fn_body = if no_json || is_unit_struct {
        quote! { None }
    } else {
        quote! { Some(<Self as rgpui::private::schemars::JsonSchema>::json_schema(_generator)) }
    };

    // 生成已弃用别名列表的函数体
    let deprecated_aliases_fn_body = if deprecated_aliases.is_empty() {
        quote! { &[] }
    } else {
        let aliases = deprecated_aliases.iter();
        quote! { &[#(#aliases),*] }
    };

    // 生成弃用提示信息函数体
    let deprecation_fn_body = if let Some(message) = deprecated {
        quote! { Some(#message) }
    } else {
        quote! { None }
    };

    // 生成文档说明函数体（如果有文档注释则返回，否则返回 None）
    let documentation_fn_body = if let Some(doc) = doc_str {
        let doc = doc.trim();
        quote! { Some(#doc) }
    } else {
        quote! { None }
    };

    // 根据 no_register 决定是否生成动作注册代码
    let registration = if no_register {
        quote! {}
    } else {
        generate_register_action(struct_name)
    };

    TokenStream::from(quote! {
        #registration

        impl rgpui::Action for #struct_name {
            fn name(&self) -> &'static str {
                #full_name
            }

            fn name_for_type() -> &'static str
            where
                Self: Sized
            {
                #full_name
            }

            fn partial_eq(&self, action: &dyn rgpui::Action) -> bool {
                action
                    .as_any()
                    .downcast_ref::<Self>()
                    .map_or(false, |a| self == a)
            }

            fn boxed_clone(&self) -> Box<dyn rgpui::Action> {
                Box::new(self.clone())
            }

            fn build(_value: rgpui::private::serde_json::Value) -> rgpui::Result<Box<dyn rgpui::Action>> {
                #build_fn_body
            }

            fn action_json_schema(
                _generator: &mut rgpui::private::schemars::SchemaGenerator,
            ) -> Option<rgpui::private::schemars::Schema> {
                #json_schema_fn_body
            }

            fn deprecated_aliases() -> &'static [&'static str] {
                #deprecated_aliases_fn_body
            }

            fn deprecation_message() -> Option<&'static str> {
                #deprecation_fn_body
            }

            fn documentation() -> Option<&'static str> {
                #documentation_fn_body
            }
        }
    })
}
