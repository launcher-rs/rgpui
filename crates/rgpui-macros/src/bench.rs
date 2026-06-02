use proc_macro::TokenStream;
use quote::quote;
use syn::{FnArg, ItemFn, Token, parse_macro_input, punctuated::Punctuated};

/// `#[rgpui::bench]` 基准测试宏 - 用于注解需要 GPUI 支持的 Criterion 基准测试函数。
///
/// 该宏将普通函数转换为 Criterion 基准测试，自动管理 `BenchAppContext` 的生命周期。
///
/// # 用法
///
/// ```ignore
/// #[rgpui::bench]
/// fn bench_my_feature(cx: &mut BenchAppContext) {
///     // 基准测试代码
/// }
/// ```
pub(crate) fn bench(args: TokenStream, function: TokenStream) -> TokenStream {
    let func = parse_macro_input!(function as ItemFn);
    let ident = &func.sig.ident;
    let vis = &func.vis;

    let bench_name = ident.to_string();

    let result = quote! {
        #vis fn #ident() -> criterion::Criterion {
            let mut group = criterion::Criterion::default();
            let mut group = group.benchmark_group(stringify!(#ident));
            group.bench_function(#bench_name, |bencher| {
                let mut cx = rgpui::app::BenchAppContext::new(Some(#bench_name));
                bencher.iter(|| {
                    cx.run_until_idle();
                });
                cx.teardown();
            });
            group.finalize();
            group
        }
    };

    TokenStream::from(result)
}
