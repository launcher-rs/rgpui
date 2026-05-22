mod derive_action;
mod derive_app_context;
mod derive_into_element;
mod derive_refineable;
mod derive_render;
mod derive_visual_context;
mod property_test;
mod register_action;
mod styles;
mod test;

#[cfg(any(feature = "inspector", debug_assertions))]
mod derive_inspector_reflection;

use proc_macro::TokenStream;
use quote::quote;
use syn::LitStr;
use syn::parse_macro_input;
use syn::{DeriveInput, Ident};

/// `Action` 派生宏 - 用于为结构体自动实现 `rgpui::Action` 特质。
///
/// 该宏会为标记的类型生成 `Action` 特质的实现，包括动作名称、JSON 序列化/反序列化、
/// 克隆等功能。支持以下属性配置：
///
/// - `name = "..."` - 指定动作的名称
/// - `namespace = ...` - 指定动作的命名空间
/// - `no_json` - 禁用 JSON 构建功能
/// - `no_register` - 禁用自动注册
/// - `deprecated_aliases = [...]` - 指定已弃用的别名
/// - `deprecated = "..."` - 标记为已弃用并提供提示信息
///
/// # 示例
///
/// ```ignore
/// #[derive(Action)]
/// struct MyAction {
///     value: String,
/// }
/// ```
#[proc_macro_derive(Action, attributes(action))]
pub fn derive_action(input: TokenStream) -> TokenStream {
    derive_action::derive_action(input)
}

/// 用于向 GPUI 运行时注册动作的过程宏。
///
/// 当你想要手动实现 `Action` 特质时，可以使用此宏来注册动作。
/// 通常情况下，你应该使用 `Action` 派生宏或 `actions!` 宏来代替。
///
/// # 示例
///
/// ```ignore
/// impl rgpui::Action for MyAction {
///     // 手动实现...
/// }
///
/// rgpui::register_action!(MyAction);
/// ```
#[proc_macro]
pub fn register_action(ident: TokenStream) -> TokenStream {
    register_action::register_action(ident)
}

/// `IntoElement` 派生宏 - 用于将实现了 `RenderOnce` 特质的类型转换为 UI 组件。
///
/// 该宏会为标记的类型生成 `rgpui::IntoElement` 特质的实现，使其可以作为
/// UI 元素在渲染树中使用。
///
/// # 示例
///
/// ```ignore
/// #[derive(IntoElement)]
/// struct MyComponent {
///     text: String,
/// }
/// ```
#[proc_macro_derive(IntoElement)]
pub fn derive_into_element(input: TokenStream) -> TokenStream {
    derive_into_element::derive_into_element(input)
}

#[proc_macro_derive(Render)]
#[doc(hidden)]
pub fn derive_render(input: TokenStream) -> TokenStream {
    derive_render::derive_render(input)
}

/// `AppContext` 派生宏 - 用于为持有 `&mut App` 的结构体生成应用上下文实现。
///
/// 该宏会为标记的类型生成 `rgpui::AppContext` 特质的实现，使其能够访问和操作
/// GPUI 应用状态，包括创建实体、更新实体、读取全局状态等。
///
/// 注意：必须使用 `#[app]` 属性标记持有 `&mut App` 的字段。
///
/// 缺少该属性会导致编译错误：
///
/// ```compile_fail
/// # #[macro_use] extern crate rgpui_macros;
/// # #[macro_use] extern crate gpui;
/// #[derive(AppContext)]
/// struct MyContext<'a> {
///     app: &'a mut rgpui::App
/// }
/// ```
///
/// # 示例
///
/// ```ignore
/// #[derive(AppContext)]
/// struct MyContext<'a> {
///     #[app]
///     app: &'a mut rgpui::App,
/// }
/// ```
#[proc_macro_derive(AppContext, attributes(app))]
pub fn derive_app_context(input: TokenStream) -> TokenStream {
    derive_app_context::derive_app_context(input)
}

/// `VisualContext` 派生宏 - 用于为持有 `&mut Window` 并实现了 `AppContext` 的类型生成可视化上下文实现。
///
/// 该宏会为标记的类型生成 `rgpui::VisualContext` 特质的实现，使其能够管理窗口级别的
/// 操作，如创建窗口实体、替换根视图、设置焦点等。
///
/// 注意：必须同时使用 `#[app]` 和 `#[window]` 属性分别标记持有 `&mut App` 和 `&mut Window` 的字段。
///
/// 缺少任一属性都会导致编译错误：
///
/// ```compile_fail
/// # #[macro_use] extern crate rgpui_macros;
/// # #[macro_use] extern crate gpui;
/// #[derive(VisualContext)]
/// struct MyContext<'a, 'b> {
///     #[app]
///     app: &'a mut rgpui::App,
///     window: &'b mut rgpui::Window
/// }
/// ```
///
/// ```compile_fail
/// # #[macro_use] extern crate rgpui_macros;
/// # #[macro_use] extern crate gpui;
/// #[derive(VisualContext)]
/// struct MyContext<'a, 'b> {
///     app: &'a mut rgpui::App,
///     #[window]
///     window: &'b mut rgpui::Window
/// }
/// ```
///
/// # 示例
///
/// ```ignore
/// #[derive(VisualContext)]
/// struct MyContext<'a, 'b> {
///     #[app]
///     app: &'a mut rgpui::App,
///     #[window]
///     window: &'b mut rgpui::Window,
/// }
/// ```
#[proc_macro_derive(VisualContext, attributes(window, app))]
pub fn derive_visual_context(input: TokenStream) -> TokenStream {
    derive_visual_context::derive_visual_context(input)
}

/// 用于创建细化类型的派生宏。
///
/// 为复杂的结构体生成 `Refinement` 结构体，支持部分初始化。
#[proc_macro_derive(Refineable, attributes(refineable))]
pub fn derive_refineable(input: TokenStream) -> TokenStream {
    derive_refineable::derive_refineable(input)
}

/// 用于生成样式辅助函数的过程宏。
///
/// 该宏由 GPUI 内部使用，用于生成样式相关的基础辅助函数，
/// 包括边距、内边距、圆角等样式属性的方法生成器。
#[proc_macro]
#[doc(hidden)]
pub fn style_helpers(input: TokenStream) -> TokenStream {
    styles::style_helpers(input)
}

/// 生成可见性样式相关的方法。
///
/// 该宏会生成 `visible()` 和 `invisible()` 方法，用于控制元素的可见性。
/// 参考 [Tailwind CSS 文档](https://tailwindcss.com/docs/visibility)
#[proc_macro]
pub fn visibility_style_methods(input: TokenStream) -> TokenStream {
    styles::visibility_style_methods(input)
}

/// 生成外边距（margin）样式相关的方法。
///
/// 该宏会生成一系列方法，如 `m()`、`mt()`、`mb()`、`mx()`、`my()` 等，
/// 用于设置元素的外边距。参考 [Tailwind CSS 文档](https://tailwindcss.com/docs/margin)
#[proc_macro]
pub fn margin_style_methods(input: TokenStream) -> TokenStream {
    styles::margin_style_methods(input)
}

/// 生成内边距（padding）样式相关的方法。
///
/// 该宏会生成一系列方法，如 `p()`、`pt()`、`pb()`、`px()`、`py()` 等，
/// 用于设置元素的内边距。参考 [Tailwind CSS 文档](https://tailwindcss.com/docs/padding)
#[proc_macro]
pub fn padding_style_methods(input: TokenStream) -> TokenStream {
    styles::padding_style_methods(input)
}

/// 生成定位（position）样式相关的方法。
///
/// 该宏会生成 `relative()`、`absolute()` 以及 `inset`、`top`、`bottom`、`left`、`right`
/// 等方法，用于控制元素的定位行为。参考 [Tailwind CSS 文档](https://tailwindcss.com/docs/position)
#[proc_macro]
pub fn position_style_methods(input: TokenStream) -> TokenStream {
    styles::position_style_methods(input)
}

/// 生成溢出（overflow）样式相关的方法。
///
/// 该宏会生成 `overflow_hidden()`、`overflow_x_hidden()`、`overflow_y_hidden()` 等方法，
/// 用于控制内容溢出容器时的行为。参考 [Tailwind CSS 文档](https://tailwindcss.com/docs/overflow)
#[proc_macro]
pub fn overflow_style_methods(input: TokenStream) -> TokenStream {
    styles::overflow_style_methods(input)
}

/// 生成光标（cursor）样式相关的方法。
///
/// 该宏会生成一系列方法，如 `cursor_default()`、`cursor_pointer()`、`cursor_text()` 等，
/// 用于设置鼠标悬停在元素上时的光标样式。参考 [Tailwind CSS 文档](https://tailwindcss.com/docs/cursor)
#[proc_macro]
pub fn cursor_style_methods(input: TokenStream) -> TokenStream {
    styles::cursor_style_methods(input)
}

/// 生成边框（border）样式相关的方法。
///
/// 该宏会生成 `border_color()` 以及 `border()`、`border_t()`、`border_b()` 等方法，
/// 用于设置元素的边框颜色和宽度。参考 [Tailwind CSS 文档](https://tailwindcss.com/docs/border-width)
#[proc_macro]
pub fn border_style_methods(input: TokenStream) -> TokenStream {
    styles::border_style_methods(input)
}

/// 生成盒子阴影（box shadow）样式相关的方法。
///
/// 该宏会生成 `shadow()`、`shadow_none()` 以及 `shadow_sm()`、`shadow_md()`、`shadow_lg()` 等方法，
/// 用于为元素添加预定义的盒子阴影效果。参考 [Tailwind CSS 文档](https://tailwindcss.com/docs/box-shadow)
#[proc_macro]
pub fn box_shadow_style_methods(input: TokenStream) -> TokenStream {
    styles::box_shadow_style_methods(input)
}

/// `#[rgpui::test]` 测试属性宏 - 用于注解需要 GPUI 支持的测试函数。
///
/// 该宏支持同步和异步测试，并可以提供任意数量的 `TestAppContext` 实例。
/// 生成的代码包含 `#[test]` 注解，因此可以与任何现有测试框架配合使用
/// （`cargo test` 或 `cargo-nextest`）。
///
/// ```ignore
/// #[rgpui::test]
/// async fn test_foo(mut cx: &TestAppContext) { }
/// ```
///
/// 除了 `TestAppContext`，你还可以请求 `StdRng` 实例。
/// 该实例将通过 `SEED` 环境变量进行种子设定，GPUI 内部的 ForegroundExecutor 和
/// BackgroundExecutor 会使用它在测试中确定性地运行任务。
/// 使用相同的 `StdRng` 可以通过仅改变种子来复现各种场景和执行交错。
///
/// # 参数
///
/// - `#[rgpui::test]` - 无参数时使用种子 `0` 或 `SEED` 环境变量（如果已设置）运行一次。
/// - `#[rgpui::test(seed = 10)]` - 使用种子 `10` 运行一次。
/// - `#[rgpui::test(seeds(10, 20, 30))]` - 使用种子 `10`、`20` 和 `30` 分别运行三次。
/// - `#[rgpui::test(iterations = 5)]` - 运行五次，种子值为 `0..5` 范围。
/// - `#[rgpui::test(retries = 3)]` - 如果测试失败，最多重试四次以尝试使其通过。
/// - `#[rgpui::test(on_failure = "crate::test::report_failure")]` - 测试失败时调用指定函数，
///   以便输出更详细的失败信息。
///
/// 可以组合 `iterations = ...` 和 `seeds(...)`：
/// - `#[rgpui::test(iterations = 5, seed = 10)]` 等价于 `#[rgpui::test(seeds(0, 1, 2, 3, 4, 10))]`。
/// - `#[rgpui::test(iterations = 5, seeds(10, 20, 30)]` 等价于 `#[rgpui::test(seeds(0, 1, 2, 3, 4, 10, 20, 30))]`。
/// - `#[rgpui::test(seeds(10, 20, 30), iterations = 5]` 等价于 `#[rgpui::test(seeds(0, 1, 2, 3, 4, 10, 20, 30))]`。
///
/// # 环境变量
///
/// - `SEED` - 设置首次运行的种子值
/// - `ITERATIONS` - 强制设置 `iterations` 参数的值
#[proc_macro_attribute]
pub fn test(args: TokenStream, function: TokenStream) -> TokenStream {
    test::test(args, function)
}

/// `#[rgpui::property_test]` 属性测试宏 - 支持基于属性的测试（property-based testing）。
///
/// 属性测试类似于 GPUI 随机测试，但允许测试形如"对于任何可能的 X，Y 应该成立"的断言。
/// 例如：
/// ```ignore
/// #[rgpui::property_test]
/// fn test_arithmetic(x: i32, y: i32) {
///     assert!(x == y || x < y || x > y);
/// }
/// ```
///
/// 标准 GPUI 随机测试提供 `StdRng` 实例以受控方式生成随机数据。
/// 属性测试具有以下额外优势：
/// - **收缩（Shrinking）** - 测试框架理解值的"复杂度"概念，能够找到导致测试失败的"最简单值"。
/// - **易用性/清晰度** - 测试框架会自动生成值，无需在测试体中编写生成逻辑。
/// - **失败持久化** - 如果找到失败的种子，会存储在文件中，后续运行会优先检查这些已知失败的情况。
///
/// 当所有输入都可以预先生成并保存在简单数据结构中时，属性测试效果最佳。
/// 某些情况下这可能不可行——例如，测试需要根据当前结构状态做出随机决策。
/// 在这种情况下，标准 GPUI 随机测试可能更合适。
///
/// ## 自定义随机值
///
/// 该宏基于 [`#[proptest::property_test]`] 宏，但处理了一些 GPUI 特有的参数。
/// 具体来说，`&{mut,} TestAppContext` 和 `BackgroundExecutor` 正常工作。
/// `StdRng` 参数被**明确禁止**，因为它们会破坏收缩机制，是常见的陷阱。
///
/// 所有其他参数都会转发到底层的 proptest 宏。
///
/// 类型为 `T` 的随机值由 `Strategy<Value = T>` 对象生成。
/// 某些类型有标准的 `Strategy`——这些类型也实现了 `Arbitrary`。
/// `#[rgpui::property_test]` 的参数默认使用类型的 `Arbitrary` 实现。
/// 如果想要提供自定义策略，可以在参数上使用 `#[strategy = ...]`：
/// ```ignore
/// #[rgpui::property_test]
/// fn int_test(#[strategy = 1..10] x: i32, #[strategy = "[a-zA-Z0-9]{20}"] s: String) {
///   assert!(s.len() > (x as usize));
/// }
/// ```
///
/// ## 调度器
///
/// 与 `#[rgpui::test]` 类似，该宏会为测试调度器选择随机种子。
/// 它使用 `.no_shrink()` 告诉 proptest 所有种子在"复杂度"方面大致等价。
/// 如果设置了 `$SEED`，它只会影响传递给调度器的种子。要控制其他值，请使用自定义 `Strategy`。
///
/// [`#[proptest::property_test]`]: https://docs.rs/proptest/latest/proptest/attr.property_test.html
/// [book]: https://proptest-rs.github.io/proptest/intro.html
/// [`Strategy`]: https://docs.rs/proptest/latest/proptest/strategy/trait.Strategy.html
#[proc_macro_attribute]
pub fn property_test(args: TokenStream, function: TokenStream) -> TokenStream {
    property_test::test(args.into(), function.into()).into()
}

/// `#[derive_inspector_reflection]` 属性宏 - 为特质生成检查器反射模块。
///
/// 当添加到特质上时，该宏会生成一个模块，提供对所有形如 `fn method(self) -> Self`
/// 的方法的枚举和按名称查找功能。检查器（inspector）使用此功能来调用 `Styled` 和
/// `StyledExt` 中的构建器方法。
///
/// 生成的模块名称为 `<snake_case_trait_name>_reflection`，包含以下函数：
///
/// ```ignore
/// pub fn methods::<T: TheTrait + 'static>() -> Vec<rgpui::inspector_reflection::FunctionReflection<T>>;
///
/// pub fn find_method::<T: TheTrait + 'static>() -> Option<rgpui::inspector_reflection::FunctionReflection<T>>;
/// ```
///
/// `FunctionReflection` 的 `invoke` 方法会运行对应的方法。`FunctionReflection` 还提供方法的文档说明。
#[cfg(any(feature = "inspector", debug_assertions))]
#[proc_macro_attribute]
pub fn derive_inspector_reflection(_args: TokenStream, input: TokenStream) -> TokenStream {
    derive_inspector_reflection::derive_inspector_reflection(_args, input)
}

/// 辅助函数：从派生输入中查找带有指定简单属性的字段。
///
/// 该函数用于查找结构体中标记了特定属性（如 `#[app]`、`#[window]`）的字段，
/// 并返回该字段的标识符。仅支持结构体，对枚举和联合体返回 `None`。
///
/// # 参数
///
/// * `ast` - 派生输入的语法树
/// * `name` - 要查找的属性名称
pub(crate) fn get_simple_attribute_field(ast: &DeriveInput, name: &'static str) -> Option<Ident> {
    match &ast.data {
        syn::Data::Struct(data_struct) => data_struct
            .fields
            .iter()
            .find(|field| field.attrs.iter().any(|attr| attr.path().is_ident(name)))
            .map(|field| field.ident.clone().unwrap()),
        syn::Data::Enum(_) => None,
        syn::Data::Union(_) => None,
    }
}

/// 用于测试中跨平台路径字符串字面量的宏。在 Windows 上将 `/` 替换为 `\\` 并在绝对路径开头添加 `C:`。
/// 在其他平台上，路径保持不变。
///
/// # 示例
/// ```rust
/// use rgpui_macros::path;
///
/// let path = path!("/Users/user/file.txt");
/// #[cfg(target_os = "windows")]
/// assert_eq!(path, "C:\\Users\\user\\file.txt");
/// #[cfg(not(target_os = "windows"))]
/// assert_eq!(path, "/Users/user/file.txt");
/// ```
#[proc_macro]
pub fn path(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as LitStr);
    let path = path.value();

    #[cfg(target_os = "windows")]
    let path = {
        let path = path.replace("/", "\\");
        if path.starts_with("\\") {
            format!("C:{}", path)
        } else {
            path
        }
    };

    TokenStream::from(quote! {
        #path
    })
}

/// 此宏将路径前缀 `file:///` 替换为 `file:///C:/`（仅适用于 Windows）。
/// 如果目标操作系统不是 Windows，则 URI 保持不变。
///
/// # 示例
/// ```rust
/// use rgpui_macros::uri;
///
/// let uri = uri!("file:///path/to/file");
/// #[cfg(target_os = "windows")]
/// assert_eq!(uri, "file:///C:/path/to/file");
/// #[cfg(not(target_os = "windows"))]
/// assert_eq!(uri, "file:///path/to/file");
/// ```
#[proc_macro]
pub fn uri(input: TokenStream) -> TokenStream {
    let uri = parse_macro_input!(input as LitStr);
    let uri = uri.value();

    #[cfg(target_os = "windows")]
    let uri = uri.replace("file:///", "file:///C:/");

    TokenStream::from(quote! {
        #uri
    })
}

/// 此宏将行尾 `\n` 替换为 `\r\n`（仅适用于 Windows）。
/// 如果目标操作系统不是 Windows，则行尾保持不变。
///
/// # 示例
/// ```rust
/// use rgpui_macros::line_endings;
///
/// let text = line_endings!("Hello\nWorld");
/// #[cfg(target_os = "windows")]
/// assert_eq!(text, "Hello\r\nWorld");
/// #[cfg(not(target_os = "windows"))]
/// assert_eq!(text, "Hello\nWorld");
/// ```
#[proc_macro]
pub fn line_endings(input: TokenStream) -> TokenStream {
    let text = parse_macro_input!(input as LitStr);
    let text = text.value();

    #[cfg(target_os = "windows")]
    let text = text.replace("\n", "\r\n");

    TokenStream::from(quote! {
        #text
    })
}
