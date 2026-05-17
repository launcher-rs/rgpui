use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 指定字体可配置的备用字体。
/// 备用字体的字体族名称存储在此结构中。
#[derive(Default, Clone, Eq, PartialEq, Hash, Debug, Deserialize, Serialize, JsonSchema)]
pub struct FontFallbacks(pub Arc<Vec<String>>);

impl FontFallbacks {
    /// 获取备用字体的字体族名称列表
    pub fn fallback_list(&self) -> &[String] {
        self.0.as_slice()
    }

    /// 从字符串列表创建备用字体
    pub fn from_fonts(fonts: Vec<String>) -> Self {
        FontFallbacks(Arc::new(fonts))
    }
}
