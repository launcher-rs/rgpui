use crate::{App, SharedString, SharedUri};
use futures::{Future, TryFutureExt};

use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 表示资源位置的枚举
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Resource {
    /// 此资源位于给定的 URI 处
    Uri(SharedUri),
    /// 此资源位于文件系统中的给定路径
    Path(Arc<Path>),
    /// 此资源嵌入在应用程序二进制文件中
    Embedded(SharedString),
}

impl From<SharedUri> for Resource {
    fn from(value: SharedUri) -> Self {
        Self::Uri(value)
    }
}

impl From<PathBuf> for Resource {
    fn from(value: PathBuf) -> Self {
        Self::Path(value.into())
    }
}

impl From<Arc<Path>> for Resource {
    fn from(value: Arc<Path>) -> Self {
        Self::Path(value)
    }
}

/// 用于异步加载资源的 trait。
pub trait Asset: 'static {
    /// 资源的来源。
    type Source: Clone + Hash + Send;

    /// 已加载的资源
    type Output: Clone + Send;

    /// 异步加载资源
    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static;
}

/// 一个资源加载器，在加载期间记录 [`Result`] 的 [`Err`] 变体
pub enum AssetLogger<T> {
    #[doc(hidden)]
    _Phantom(PhantomData<T>, &'static dyn crate::seal::Sealed),
}

impl<T, R, E> Asset for AssetLogger<T>
where
    T: Asset<Output = Result<R, E>>,
    R: Clone + Send,
    E: Clone + Send + std::fmt::Display,
{
    type Source = T::Source;

    type Output = T::Output;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let load = T::load(source, cx);
        load.inspect_err(|e| log::error!("Failed to load asset: {}", e))
    }
}

/// 使用快速、非密码学安全的哈希函数从数据中获取标识符
pub fn hash<T: Hash>(data: &T) -> u64 {
    let mut hasher = crate::collections::FxHasher::default();
    data.hash(&mut hasher);
    hasher.finish()
}
