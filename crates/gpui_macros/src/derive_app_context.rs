use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

use crate::get_simple_attribute_field;

/// 为持有 `&mut App` 的结构体生成 `gpui::AppContext` 特质实现。
///
/// 该函数通过查找标记了 `#[app]` 属性的字段，将该字段作为代理，
/// 将所有 `AppContext` 特质方法转发到该字段上。
///
/// 生成的实现包括：
/// - `new()` - 创建新实体
/// - `reserve_entity()` / `insert_entity()` - 预留和插入实体
/// - `update_entity()` - 更新实体
/// - `as_mut()` - 获取实体的可变引用
/// - `read_entity()` - 读取实体
/// - `update_window()` / `with_window()` - 更新窗口
/// - `read_window()` - 读取窗口
/// - `background_spawn()` - 在后台生成异步任务
/// - `read_global()` - 读取全局状态
///
/// 如果未找到 `#[app]` 属性，则返回编译错误。
pub fn derive_app_context(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    // 查找标记了 #[app] 属性的字段，如果未找到则返回编译错误
    let Some(app_variable) = get_simple_attribute_field(&ast, "app") else {
        return quote! {
            compile_error!("Derive must have an #[app] attribute to detect the &mut App field");
        }
        .into();
    };

    let type_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();

    let r#gen = quote! {
        impl #impl_generics gpui::AppContext for #type_name #type_generics
        #where_clause
        {
            fn new<T: 'static>(
                &mut self,
                build_entity: impl FnOnce(&mut gpui::Context<'_, T>) -> T,
            ) -> gpui::Entity<T> {
                self.#app_variable.new(build_entity)
            }

            fn reserve_entity<T: 'static>(&mut self) -> gpui::Reservation<T> {
                self.#app_variable.reserve_entity()
            }

            fn insert_entity<T: 'static>(
                &mut self,
                reservation: gpui::Reservation<T>,
                build_entity: impl FnOnce(&mut gpui::Context<'_, T>) -> T,
            ) -> gpui::Entity<T> {
                self.#app_variable.insert_entity(reservation, build_entity)
            }

            fn update_entity<T, R>(
                &mut self,
                handle: &gpui::Entity<T>,
                update: impl FnOnce(&mut T, &mut gpui::Context<'_, T>) -> R,
            ) -> R
            where
                T: 'static,
            {
                self.#app_variable.update_entity(handle, update)
            }

            fn as_mut<'y, 'z, T>(
                &'y mut self,
                handle: &'z gpui::Entity<T>,
            ) -> gpui::GpuiBorrow<'y, T>
            where
                T: 'static,
            {
                self.#app_variable.as_mut(handle)
            }

            fn read_entity<T, R>(
                &self,
                handle: &gpui::Entity<T>,
                read: impl FnOnce(&T, &gpui::App) -> R,
            ) -> R
            where
                T: 'static,
            {
                self.#app_variable.read_entity(handle, read)
            }

            fn update_window<T, F>(&mut self, window: gpui::AnyWindowHandle, f: F) -> gpui::Result<T>
            where
                F: FnOnce(gpui::AnyView, &mut gpui::Window, &mut gpui::App) -> T,
            {
                self.#app_variable.update_window(window, f)
            }

            fn with_window<R>(
                &mut self,
                entity_id: gpui::EntityId,
                f: impl FnOnce(&mut gpui::Window, &mut gpui::App) -> R,
            ) -> Option<R>
            {
                self.#app_variable.with_window(entity_id, f)
            }

            fn read_window<T, R>(
                &self,
                window: &gpui::WindowHandle<T>,
                read: impl FnOnce(gpui::Entity<T>, &gpui::App) -> R,
            ) -> gpui::Result<R>
            where
                T: 'static,
            {
                self.#app_variable.read_window(window, read)
            }

            fn background_spawn<R>(&self, future: impl std::future::Future<Output = R> + Send + 'static) -> gpui::Task<R>
            where
                R: Send + 'static,
            {
                self.#app_variable.background_spawn(future)
            }

            fn read_global<G, R>(&self, callback: impl FnOnce(&G, &gpui::App) -> R) -> R
            where
                G: gpui::Global,
            {
                self.#app_variable.read_global(callback)
            }
        }
    };

    r#gen.into()
}
