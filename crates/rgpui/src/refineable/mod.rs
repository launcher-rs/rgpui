#![allow(missing_docs)]

pub use rgpui_macros::Refineable;

/// 可使用部分更新进行细化的类型特征。
///
/// `Refineable` 特征支持分层配置模式，其中基础配置
/// 可以被细化选择性地覆盖。这对于样式和设置以及主题层次结构特别有用。
///
/// # 派生宏
///
/// `#[derive(Refineable)]` 宏自动生成一个配套的细化类型并
/// 实现此特征。对于结构体 `Style`，它会创建 `StyleRefinement`，其中每个字段都
/// 被适当包装：
///
/// - **Refineable 字段**（标记为 `#[refineable]`）：变为对应的细化类型
///   （例如 `Bar` 变为 `BarRefinement`，或 `BarRefinement` 保持为 `BarRefinement`）
/// - **可选字段**（`Option<T>`）：保持为 `Option<T>`
/// - **普通字段**：变为 `Option<T>`
///
/// ## 属性
///
/// 派生宏支持结构体上的这些属性：
/// - `#[refineable(Debug)]`：为细化类型实现 `Debug`
/// - `#[refineable(Serialize)]`：派生 `Serialize`，跳过序列化 `None`
/// - `#[refineable(OtherTrait)]`：为细化类型派生额外特征
///
/// 字段可以标记为：
/// - `#[refineable]`：字段本身是可细化的（使用嵌套细化类型）
pub trait Refineable: Clone {
    type Refinement: Refineable<Refinement = Self::Refinement> + IsEmpty + Default;

    /// 将给定细化应用到此实例，就地修改。
    ///
    /// 仅应用细化中的非空值。
    ///
    /// * 对于可细化字段，递归调用 `refine`。
    /// * 对于其他字段，如果细化中存在该值则替换。
    fn refine(&mut self, refinement: &Self::Refinement);

    /// 返回应用了细化的新实例，等价于克隆 `self` 并在其上调用 `refine`
    fn refined(self, refinement: Self::Refinement) -> Self;

    /// 通过将所有细化合并到默认值之上来从级联创建实例
    fn from_cascade(cascade: &Cascade<Self>) -> Self
    where
        Self: Default + Sized,
    {
        Self::default().refined(cascade.merged())
    }

    /// 如果此实例包含细化中的所有值，则返回 `true`
    ///
    /// 对于可细化字段，递归检查 `is_superset_of`。对于其他字段，检查
    /// 细化中的 `Some` 值是否与此实例的值匹配。
    fn is_superset_of(&self, refinement: &Self::Refinement) -> bool;

    /// 返回表示此实例与给定细化之间差异的细化
    ///
    /// 对于可细化字段，递归调用 `subtract`。对于其他字段，如果
    /// 字段值等于细化中的值，则该字段为 `None`。
    fn subtract(&self, refinement: &Self::Refinement) -> Self::Refinement;
}

/// 表示应用此细化不会产生任何效果的特征
pub trait IsEmpty {
    /// 如果应用此细化不会产生任何效果，则返回 `true`
    fn is_empty(&self) -> bool;
}

/// 可以按优先级顺序合并的细化级联。
///
/// 级联维护可选细化的序列，其中后面的条目
/// 优先于前面的条目。第一个槽位（索引 0）始终是
/// 基础细化并保证存在。
///
/// 这对于实现配置层次结构（如 CSS 级联）很有用，
/// 其中来自不同来源（用户代理、用户、作者）的样式以特定优先级规则组合。
pub struct Cascade<S: Refineable>(Vec<Option<S::Refinement>>);

impl<S: Refineable + Default> Default for Cascade<S> {
    fn default() -> Self {
        Self(vec![Some(Default::default())])
    }
}

/// 级联中特定槽位的句柄。
///
/// 槽位用于标识级联中的特定位置，
/// 细化可以在这些位置设置或更新。
#[derive(Copy, Clone)]
pub struct CascadeSlot(usize);

impl<S: Refineable + Default> Cascade<S> {
    /// 在级联中预留新槽位并返回其句柄。
    ///
    /// 新槽位初始为空（`None`），可稍后使用 `set()` 填充。
    pub fn reserve(&mut self) -> CascadeSlot {
        self.0.push(None);
        CascadeSlot(self.0.len() - 1)
    }

    /// 返回基础细化（槽位 0）的可变引用。
    ///
    /// 基础细化始终存在，并作为级联的基础。
    pub fn base(&mut self) -> &mut S::Refinement {
        self.0[0].as_mut().unwrap()
    }

    /// 设置级联中特定槽位的细化。
    ///
    /// 将槽位设置为 `None` 实际上会在合并期间将其从考虑中移除。
    pub fn set(&mut self, slot: CascadeSlot, refinement: Option<S::Refinement>) {
        self.0[slot.0] = refinement
    }

    /// 将级联中的所有细化合并为单个细化。
    ///
    /// 细化按顺序应用，后面的槽位优先。
    /// 空槽位（`None`）在合并期间被跳过。
    pub fn merged(&self) -> S::Refinement {
        let mut merged = self.0[0].clone().unwrap();
        for refinement in self.0.iter().skip(1).flatten() {
            merged.refine(refinement);
        }
        merged
    }
}
