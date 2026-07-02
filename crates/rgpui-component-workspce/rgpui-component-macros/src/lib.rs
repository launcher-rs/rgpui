use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};

mod derive_into_plot;

/// `icon_named!` 宏的输入结构：枚举名称、路径、可选的额外派生
struct IconNameInput {
    /// 枚举名称
    enum_name: syn::Ident,
    /// 逗号分隔符
    _comma: syn::Token![,],
    /// SVG 图标目录路径
    path: syn::LitStr,
    /// 可选的额外派生 trait 列表
    derives: Option<(
        syn::Token![,],
        syn::punctuated::Punctuated<syn::Path, syn::Token![,]>,
    )>,
}

impl Parse for IconNameInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let enum_name = input.parse()?;
        let _comma = input.parse()?;
        let path = input.parse()?;

        // 解析可选的额外派生列表
        let derives = if input.peek(syn::Token![,]) {
            let comma = input.parse()?;
            let content;
            syn::bracketed!(content in input);
            let derives = content.parse_terminated(syn::Path::parse, syn::Token![,])?;
            Some((comma, derives))
        } else {
            None
        };

        Ok(IconNameInput {
            enum_name,
            _comma,
            path,
            derives,
        })
    }
}

/// 为图表类型派生 `IntoPlot` trait
#[proc_macro_derive(IntoPlot)]
pub fn derive_into_plot(input: TokenStream) -> TokenStream {
    derive_into_plot::derive_into_plot(input)
}

/// 将 SVG 文件名转换为 PascalCase 标识符。
///
/// 移除 `.svg` 扩展名，按分隔符（`-`、`_`、`.`）分割，
/// 并将每个单词首字母大写，遵循 Rust 命名规范。
///
/// # 示例
///
/// ```ignore
/// assert_eq!(pascal_case("arrow-right.svg"), "ArrowRight");
/// assert_eq!(pascal_case("some_icon_name.svg"), "SomeIconName");
/// assert_eq!(pascal_case("icon-123.svg"), "Icon123");
/// ```
fn pascal_case(filename: &str) -> String {
    filename
        .strip_suffix(".svg")
        .unwrap_or(filename)
        .split(|c: char| c == '-' || c == '_' || c == '.')
        .filter(|part| !part.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) if first.is_ascii_digit() => word.to_string(),
                Some(first) => {
                    let mut result = String::with_capacity(word.len());
                    result.extend(first.to_uppercase());
                    result.push_str(&chars.as_str().to_lowercase());
                    result
                }
            }
        })
        .collect()
}

/// 通过扫描 SVG 文件目录生成自定义图标枚举及其 `IconNamed` 实现。
///
/// 接受枚举名称、相对于调用 crate 的 `CARGO_MANIFEST_DIR` 的路径，
/// 以及可选的额外派生 trait 列表。
/// 每个 `.svg` 文件都会使用 PascalCase 转换成为枚举变体。
///
/// # 示例
///
/// ```ignore
/// // 基本用法（默认派生 IntoElement、Clone）
/// icon_named!(IconName, "../assets/assets/icons");
///
/// // 带自定义派生
/// icon_named!(IconName, "../assets/assets/icons", [Debug, Copy, PartialEq, Eq]);
/// ```
#[proc_macro]
pub fn icon_named(input: TokenStream) -> TokenStream {
    let IconNameInput {
        enum_name,
        path,
        derives,
        ..
    } = syn::parse_macro_input!(input as IconNameInput);

    let relative_path = path.value();

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let icons_dir = std::path::Path::new(&manifest_dir).join(&relative_path);

    let mut entries: Vec<(String, String)> = Vec::new();

    let dir = std::fs::read_dir(&icons_dir).unwrap_or_else(|e| {
        panic!(
            "generate_icon_enum: 无法读取目录 '{}': {}",
            icons_dir.display(),
            e
        )
    });

    for entry in dir {
        let entry = entry.expect("无法读取目录条目");
        let filename = entry.file_name().to_string_lossy().to_string();
        if filename.ends_with(".svg") {
            let variant_name = pascal_case(&filename);
            let path = format!("icons/{}", filename);
            entries.push((variant_name, path));
        }
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let variants: Vec<proc_macro2::Ident> = entries
        .iter()
        .map(|(name, _)| proc_macro2::Ident::new(name, proc_macro2::Span::call_site()))
        .collect();
    let paths: Vec<&str> = entries.iter().map(|(_, p)| p.as_str()).collect();

    // 构建派生列表：始终包含 IntoElement 和 Clone，然后添加自定义派生
    let derive_attrs = if let Some((_, custom_derives)) = derives {
        let derives_vec: Vec<_> = custom_derives.iter().collect();
        quote! {
            #[derive(IntoElement, Clone, #(#derives_vec),*)]
        }
    } else {
        quote! {
            #[derive(IntoElement, Clone)]
        }
    };

    let expanded = quote! {
        #derive_attrs

        pub enum #enum_name {
            #(#variants,)*
        }

        impl IconNamed for #enum_name {
            fn path(self) -> SharedString {
                match self {
                    #(Self::#variants => #paths,)*
                }
                .into()
            }
        }
    };

    TokenStream::from(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pascal_case_basic() {
        assert_eq!(pascal_case("arrow-right.svg"), "ArrowRight");
        assert_eq!(pascal_case("home.svg"), "Home");
        assert_eq!(pascal_case("x-circle.svg"), "XCircle");

        assert_eq!(pascal_case("some_icon_name.svg"), "SomeIconName");
        assert_eq!(pascal_case("arrow_up_down.svg"), "ArrowUpDown");

        assert_eq!(pascal_case("kebab-case_mixed.svg"), "KebabCaseMixed");
        assert_eq!(pascal_case("icon-with_under.svg"), "IconWithUnder");

        assert_eq!(pascal_case("icon-123.svg"), "Icon123");
        assert_eq!(pascal_case("arrow-2x.svg"), "Arrow2x");
        assert_eq!(pascal_case("24-hour.svg"), "24Hour");

        assert_eq!(pascal_case("arrow--right.svg"), "ArrowRight");
        assert_eq!(pascal_case("icon__name.svg"), "IconName");
        assert_eq!(pascal_case("multiple---dash.svg"), "MultipleDash");

        assert_eq!(pascal_case("a.svg"), "A");
        assert_eq!(pascal_case("-leading.svg"), "Leading");
        assert_eq!(pascal_case("trailing-.svg"), "Trailing");
        assert_eq!(pascal_case("-.svg"), "");

        assert_eq!(pascal_case("arrow-right"), "ArrowRight");
        assert_eq!(pascal_case("home"), "Home");

        assert_eq!(pascal_case("hello.svg"), "Hello");
        assert_eq!(pascal_case("WORLD.svg"), "World");
        assert_eq!(pascal_case("iOS-icon.svg"), "IosIcon");
        assert_eq!(pascal_case("API-key.svg"), "ApiKey");
    }
}
