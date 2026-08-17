//! 表格子系统 - 虚拟化数据表格、列定义、表头与表体组件

use crate::App;

/// 列定义模块 - 定义表格列、列组、排序与拖拽调整列宽等能力
mod column;

/// 数据表格模块 - DataTable 元素与表格选项、快捷键绑定
mod data_table;

/// 数据委托模块 - 定义表格数据来源与行为接口
mod delegate;

/// 加载状态模块 - 表格加载骨架屏展示
mod loading;

/// 表格状态模块 - TableState、选择模式与行渲染逻辑
mod state;

/// 表格组件模块 - Table、表头、表体、表行、表单元格等组件
mod table;

/// 重导出列定义相关类型
pub use column::*;
/// 重导出数据表格相关类型
pub use data_table::*;
/// 重导出数据委托相关类型
pub use delegate::*;
/// 重导出表格状态相关类型
pub use state::*;
/// 重导出表格组件相关类型
pub use table::*;

/// 初始化表格子系统，注册 DataTable 的默认快捷键绑定
pub fn init(cx: &mut App) {
    data_table::init(cx);
}
