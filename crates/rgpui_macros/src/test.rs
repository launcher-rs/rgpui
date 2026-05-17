use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use std::mem;
use syn::{
    self, Expr, ExprLit, FnArg, ItemFn, Lit, Meta, MetaList, PathSegment, Token, Type,
    parse::{Parse, ParseStream},
    parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
};

/// 测试宏的参数结构体。
///
/// 存储从 `#[rgpui::test(...)]` 属性中解析出的配置项：
/// - `seeds` - 随机种子列表
/// - `max_retries` - 最大重试次数
/// - `max_iterations` - 最大迭代次数
/// - `on_failure_fn_name` - 失败时调用的回调函数
struct Args {
    seeds: Vec<u64>,
    max_retries: usize,
    max_iterations: usize,
    on_failure_fn_name: proc_macro2::TokenStream,
}

/// 为 `Args` 实现语法解析。
///
/// 解析以下参数格式：
/// - `retries = <usize>` - 最大重试次数
/// - `iterations = <usize>` - 最大迭代次数
/// - `on_failure = "path::to::fn"` - 失败回调函数路径
/// - `seed = <u64>` - 单个种子值
/// - `seeds(u64, u64, ...)` - 种子值列表
impl Parse for Args {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        let mut seeds = Vec::<u64>::new();
        let mut max_retries = 0;
        let mut max_iterations = 1;
        let mut on_failure_fn_name = quote!(None);

        let metas = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;

        for meta in metas {
            let ident = {
                let meta_path = match &meta {
                    Meta::NameValue(meta) => &meta.path,
                    Meta::List(list) => &list.path,
                    Meta::Path(path) => {
                        return Err(syn::Error::new(path.span(), "invalid path argument"));
                    }
                };
                let Some(ident) = meta_path.get_ident() else {
                    return Err(syn::Error::new(meta_path.span(), "unexpected path"));
                };
                ident.to_string()
            };

            match (&meta, ident.as_str()) {
                (Meta::NameValue(meta), "retries") => {
                    max_retries = parse_usize_from_expr(&meta.value)?
                }
                (Meta::NameValue(meta), "iterations") => {
                    max_iterations = parse_usize_from_expr(&meta.value)?
                }
                (Meta::NameValue(meta), "on_failure") => {
                    let Expr::Lit(ExprLit {
                        lit: Lit::Str(name),
                        ..
                    }) = &meta.value
                    else {
                        return Err(syn::Error::new(
                            meta.value.span(),
                            "on_failure argument must be a string",
                        ));
                    };
                    let segments = name
                        .value()
                        .split("::")
                        .map(|part| PathSegment::from(Ident::new(part, name.span())))
                        .collect();
                    let path = syn::Path {
                        leading_colon: None,
                        segments,
                    };
                    on_failure_fn_name = quote!(Some(#path));
                }
                (Meta::NameValue(meta), "seed") => {
                    seeds = vec![parse_usize_from_expr(&meta.value)? as u64]
                }
                (Meta::List(list), "seeds") => seeds = parse_u64_array(list)?,
                (Meta::Path(_), _) => {
                    return Err(syn::Error::new(meta.span(), "invalid path argument"));
                }
                (_, _) => {
                    return Err(syn::Error::new(meta.span(), "invalid argument name"));
                }
            }
        }

        Ok(Args {
            seeds,
            max_retries,
            max_iterations,
            on_failure_fn_name,
        })
    }
}

/// `#[rgpui::test]` 过程宏的入口函数。
///
/// 该函数解析宏参数和测试函数，然后调用 `generate_test_function` 生成包装后的测试代码。
/// 生成的代码包含 `#[test]` 注解，可以与标准测试框架配合使用。
pub fn test(args: TokenStream, function: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(args as Args);
    let mut inner_fn = match syn::parse::<ItemFn>(function) {
        Ok(f) => f,
        Err(err) => return error_to_stream(err),
    };

    let inner_fn_attributes = mem::take(&mut inner_fn.attrs);
    let inner_fn_name = format_ident!("__{}", inner_fn.sig.ident);
    let outer_fn_name = mem::replace(&mut inner_fn.sig.ident, inner_fn_name.clone());

    let result = generate_test_function(
        args,
        inner_fn,
        inner_fn_attributes,
        inner_fn_name,
        outer_fn_name,
    );
    match result {
        Ok(tokens) => tokens,
        Err(tokens) => tokens,
    }
}

/// 生成测试函数的核心逻辑。
///
/// 该函数根据测试函数是否为异步函数，生成不同的包装代码：
///
/// **异步测试**：使用 `ForegroundExecutor::block_test` 阻塞执行异步函数。
/// **同步测试**：直接调用测试函数。
///
/// 对于测试函数的参数，该函数会识别以下类型并自动注入：
/// - `StdRng` - 注入基于种子初始化的随机数生成器
/// - `BackgroundExecutor` - 注入后台执行器
/// - `&mut TestAppContext` - 注入测试应用上下文
/// - `&mut App` - 注入应用的可变引用
///
/// # 参数
///
/// * `args` - 解析后的宏参数
/// * `inner_fn` - 原始测试函数
/// * `inner_fn_attributes` - 原始测试函数的属性
/// * `inner_fn_name` - 内部函数名称（以 `__` 为前缀）
/// * `outer_fn_name` - 外部函数名称（原始函数名）
fn generate_test_function(
    args: Args,
    inner_fn: ItemFn,
    inner_fn_attributes: Vec<syn::Attribute>,
    inner_fn_name: Ident,
    outer_fn_name: Ident,
) -> Result<TokenStream, TokenStream> {
    let seeds = &args.seeds;
    let max_retries = args.max_retries;
    let num_iterations = args.max_iterations;
    let on_failure_fn_name = &args.on_failure_fn_name;
    let seeds = quote!( #(#seeds),* );

    // 根据测试函数是否为异步生成不同的包装代码
    let mut outer_fn: ItemFn = if inner_fn.sig.asyncness.is_some() {
        // 根据参数列表为测试函数注入所需的应用上下文
        let mut cx_vars = proc_macro2::TokenStream::new();
        let mut cx_teardowns = proc_macro2::TokenStream::new();
        let mut inner_fn_args = proc_macro2::TokenStream::new();
        // 遍历测试函数参数，根据类型注入相应的测试上下文
        for (ix, arg) in inner_fn.sig.inputs.iter().enumerate() {
            if let FnArg::Typed(arg) = arg {
                if let Type::Path(ty) = &*arg.ty {
                    let last_segment = ty.path.segments.last();
                    match last_segment.map(|s| s.ident.to_string()).as_deref() {
                        Some("StdRng") => {
                            inner_fn_args.extend(quote!(rand::SeedableRng::seed_from_u64(_seed),));
                            continue;
                        }
                        Some("BackgroundExecutor") => {
                            inner_fn_args.extend(quote!(rgpui::BackgroundExecutor::new(
                                std::sync::Arc::new(dispatcher.clone()),
                            ),));
                            continue;
                        }
                        _ => {}
                    }
                } else if let Type::Reference(ty) = &*arg.ty
                    && let Type::Path(ty) = &*ty.elem
                {
                    let last_segment = ty.path.segments.last();
                    if let Some("TestAppContext") =
                        last_segment.map(|s| s.ident.to_string()).as_deref()
                    {
                        let cx_varname = format_ident!("cx_{}", ix);
                        cx_vars.extend(quote!(
                            let mut #cx_varname = rgpui::TestAppContext::build(
                                dispatcher.clone(),
                                Some(stringify!(#outer_fn_name)),
                            );
                            let _entity_refcounts = #cx_varname.app.borrow().ref_counts_drop_handle();
                        ));
                        cx_teardowns.extend(quote!(
                            #cx_varname.run_until_parked();
                            #cx_varname.update(|cx| { cx.background_executor().forbid_parking(); cx.quit(); });
                            #cx_varname.run_until_parked();
                            drop(#cx_varname);
                        ));
                        inner_fn_args.extend(quote!(&mut #cx_varname,));
                        continue;
                    }
                }
            }

            return Err(error_with_message("invalid function signature", arg));
        }

        parse_quote! {
            #[test]
            fn #outer_fn_name() {
                #inner_fn

                rgpui::run_test(
                    #num_iterations,
                    &[#seeds],
                    #max_retries,
                    &mut |dispatcher, _seed| {
                        let exec = std::sync::Arc::new(dispatcher.clone());
                        #cx_vars
                        rgpui::ForegroundExecutor::new(exec.clone()).block_test(#inner_fn_name(#inner_fn_args));
                        drop(exec);
                        #cx_teardowns
                        // Ideally we would only drop cancelled tasks, that way we could detect leaks due to task <-> entity
                        // cycles as cancelled tasks will be dropped properly once the runnable gets run again
                        //
                        // async-task does not give us the power to do this just yet though
                        dispatcher.drain_tasks();
                        drop(dispatcher);
                    },
                    #on_failure_fn_name
                );
            }
        }
    } else {
        // 同步测试：根据参数列表为测试函数注入所需的应用上下文
        let mut cx_vars = proc_macro2::TokenStream::new();
        let mut cx_teardowns = proc_macro2::TokenStream::new();
        let mut inner_fn_args = proc_macro2::TokenStream::new();
        // 遍历同步测试函数参数，根据类型注入相应的测试上下文
        for (ix, arg) in inner_fn.sig.inputs.iter().enumerate() {
            if let FnArg::Typed(arg) = arg {
                if let Type::Path(ty) = &*arg.ty {
                    let last_segment = ty.path.segments.last();

                    if let Some("StdRng") = last_segment.map(|s| s.ident.to_string()).as_deref() {
                        inner_fn_args.extend(quote!(rand::SeedableRng::seed_from_u64(_seed),));
                        continue;
                    }
                } else if let Type::Reference(ty) = &*arg.ty
                    && let Type::Path(ty) = &*ty.elem
                {
                    let last_segment = ty.path.segments.last();
                    match last_segment.map(|s| s.ident.to_string()).as_deref() {
                        Some("App") => {
                            let cx_varname = format_ident!("cx_{}", ix);
                            let cx_varname_lock = format_ident!("cx_{}_lock", ix);
                            cx_vars.extend(quote!(
                                let mut #cx_varname = rgpui::TestAppContext::build(
                                   dispatcher.clone(),
                                   Some(stringify!(#outer_fn_name))
                                );
                                let mut #cx_varname_lock = #cx_varname.app.borrow_mut();
                                let _entity_refcounts = #cx_varname_lock.ref_counts_drop_handle();
                            ));
                            inner_fn_args.extend(quote!(&mut #cx_varname_lock,));
                            cx_teardowns.extend(quote!(
                                    drop(#cx_varname_lock);
                                    #cx_varname.run_until_parked();
                                    #cx_varname.update(|cx| { cx.background_executor().forbid_parking(); cx.quit(); });
                                    #cx_varname.run_until_parked();
                                    drop(#cx_varname);
                                ));
                            continue;
                        }
                        Some("TestAppContext") => {
                            let cx_varname = format_ident!("cx_{}", ix);
                            cx_vars.extend(quote!(
                                let mut #cx_varname = rgpui::TestAppContext::build(
                                    dispatcher.clone(),
                                    Some(stringify!(#outer_fn_name))
                                );
                                let _entity_refcounts = #cx_varname.app.borrow().ref_counts_drop_handle();
                            ));
                            cx_teardowns.extend(quote!(
                                #cx_varname.run_until_parked();
                                #cx_varname.update(|cx| { cx.background_executor().forbid_parking(); cx.quit(); });
                                #cx_varname.run_until_parked();
                                drop(#cx_varname);
                            ));
                            inner_fn_args.extend(quote!(&mut #cx_varname,));
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            return Err(error_with_message("invalid function signature", arg));
        }

        parse_quote! {
            #[test]
            fn #outer_fn_name() {
                #inner_fn

                rgpui::run_test(
                    #num_iterations,
                    &[#seeds],
                    #max_retries,
                    &mut |dispatcher, _seed| {
                        #cx_vars
                        #inner_fn_name(#inner_fn_args);
                        #cx_teardowns
                        // Ideally we would only drop cancelled tasks, that way we could detect leaks due to task <-> entity
                        // cycles as cancelled tasks will be dropped properly once they runnable gets run again
                        //
                        // async-task does not give us the power to do this just yet though
                        dispatcher.drain_tasks();
                        drop(dispatcher);
                    },
                    #on_failure_fn_name,
                );
            }
        }
    };
    outer_fn.attrs.extend(inner_fn_attributes);

    Ok(TokenStream::from(quote!(#outer_fn)))
}

/// 从表达式中解析 `usize` 类型的整数值。
///
/// # 错误
///
/// 如果表达式不是整数字面量或解析失败，则返回相应的语法错误。
fn parse_usize_from_expr(expr: &Expr) -> Result<usize, syn::Error> {
    let Expr::Lit(ExprLit {
        lit: Lit::Int(int), ..
    }) = expr
    else {
        return Err(syn::Error::new(expr.span(), "expected an integer"));
    };
    int.base10_parse()
        .map_err(|_| syn::Error::new(int.span(), "failed to parse integer"))
}

/// 从元列表（如 `seeds(1, 2, 3)`）中解析 `u64` 数组。
///
/// 该函数用于解析 `seeds(...)` 参数中的种子值列表。
fn parse_u64_array(meta_list: &MetaList) -> Result<Vec<u64>, syn::Error> {
    let mut result = Vec::new();
    let tokens = &meta_list.tokens;
    let parser = |input: ParseStream| {
        let exprs = Punctuated::<Expr, Token![,]>::parse_terminated(input)?;
        for expr in exprs {
            if let Expr::Lit(ExprLit {
                lit: Lit::Int(int), ..
            }) = expr
            {
                let value: usize = int.base10_parse()?;
                result.push(value as u64);
            } else {
                return Err(syn::Error::new(expr.span(), "expected an integer"));
            }
        }
        Ok(())
    };
    syn::parse::Parser::parse2(parser, tokens.clone())?;
    Ok(result)
}

/// 将错误消息转换为编译错误 TokenStream。
///
/// 该函数用于在宏展开期间生成用户友好的编译错误。
fn error_with_message(message: &str, spanned: impl Spanned) -> TokenStream {
    error_to_stream(syn::Error::new(spanned.span(), message))
}

/// 将 syn 错误转换为编译错误 TokenStream。
///
/// 该函数使用 `into_compile_error()` 将语法错误转换为可以在编译时显示的错误信息。
fn error_to_stream(err: syn::Error) -> TokenStream {
    TokenStream::from(err.into_compile_error())
}
