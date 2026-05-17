use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

use super::get_simple_attribute_field;

/// 为持有 `&mut Window` 和 `&mut App` 的结构体生成 `rgpui::VisualContext` 特质实现。
///
/// 该函数通过查找标记了 `#[window]` 和 `#[app]` 属性的字段，将可视化上下文操作
/// 转发到对应的字段上。
///
/// 生成的实现包括：
/// - `window_handle()` - 获取窗口句柄
/// - `update_window_entity()` - 更新窗口实体
/// - `new_window_entity()` - 创建新窗口实体
/// - `replace_root_view()` - 替换根视图
/// - `focus()` - 设置焦点到可聚焦实体
///
/// 如果未找到 `#[window]` 或 `#[app]` 属性，则返回相应的编译错误。
pub fn derive_visual_context(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    // 查找标记了 #[window] 属性的字段，如果未找到则返回编译错误
    let Some(window_variable) = get_simple_attribute_field(&ast, "window") else {
        return quote! {
            compile_error!("Derive must have a #[window] attribute to detect the &mut Window field");
        }
        .into();
    };

    // 查找标记了 #[app] 属性的字段，如果未找到则返回编译错误
    let Some(app_variable) = get_simple_attribute_field(&ast, "app") else {
        return quote! {
            compile_error!("Derive must have a #[app] attribute to detect the &mut App field");
        }
        .into();
    };

    let type_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();

    let r#gen = quote! {
        impl #impl_generics rgpui::VisualContext for #type_name #type_generics
        #where_clause
        {
            type Result<T> = T;

            fn window_handle(&self) -> rgpui::AnyWindowHandle {
                self.#window_variable.window_handle()
            }

            fn update_window_entity<T: 'static, R>(
                &mut self,
                entity: &rgpui::Entity<T>,
                update: impl FnOnce(&mut T, &mut rgpui::Window, &mut rgpui::Context<T>) -> R,
            ) -> R {
                rgpui::AppContext::update_entity(self.#app_variable, entity, |entity, cx| update(entity, self.#window_variable, cx))
            }

            fn new_window_entity<T: 'static>(
                &mut self,
                build_entity: impl FnOnce(&mut rgpui::Window, &mut rgpui::Context<'_, T>) -> T,
            ) -> rgpui::Entity<T> {
                rgpui::AppContext::new(self.#app_variable, |cx| build_entity(self.#window_variable, cx))
            }

            fn replace_root_view<V>(
                &mut self,
                build_view: impl FnOnce(&mut rgpui::Window, &mut rgpui::Context<V>) -> V,
            ) -> rgpui::Entity<V>
            where
                V: 'static + rgpui::Render,
            {
                self.#window_variable.replace_root(self.#app_variable, build_view)
            }

            fn focus<V>(&mut self, entity: &rgpui::Entity<V>)
            where
                V: rgpui::Focusable,
            {
                let focus_handle = rgpui::Focusable::focus_handle(entity, self.#app_variable);
                self.#window_variable.focus(&focus_handle, self.#app_variable);
            }
        }
    };

    r#gen.into()
}
