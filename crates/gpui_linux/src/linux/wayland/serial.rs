use gpui::collections::HashMap;

/// 序列号类型枚举
#[derive(Debug, Hash, PartialEq, Eq)]
pub(crate) enum SerialKind {
    /// 数据设备序列号
    DataDevice,
    /// 输入法序列号
    InputMethod,
    /// 鼠标进入序列号
    MouseEnter,
    /// 鼠标按下序列号
    MousePress,
    /// 按键按下序列号
    KeyPress,
}

/// 序列号数据包装
struct SerialData {
    serial: u32,
}

impl SerialData {
    fn new(value: u32) -> Self {
        Self { serial: value }
    }
}

/// 用于跟踪不同序列号类型的辅助结构
pub(crate) struct SerialTracker {
    serials: HashMap<SerialKind, SerialData>,
}

impl SerialTracker {
    /// 创建新的序列号跟踪器
    pub fn new() -> Self {
        Self {
            serials: HashMap::default(),
        }
    }

    /// 更新指定类型的序列号
    pub fn update(&mut self, kind: SerialKind, value: u32) {
        self.serials.insert(kind, SerialData::new(value));
    }

    /// 返回指定 [`SerialKind`] 的最新序列号
    ///
    /// 如果未跟踪则返回 0
    pub fn get(&self, kind: SerialKind) -> u32 {
        self.serials
            .get(&kind)
            .map(|serial_data| serial_data.serial)
            .unwrap_or(0)
    }

    /// 返回指定 [`SerialKind`] 列表中的最新序列号
    ///
    /// 如果未跟踪则返回 0
    pub fn latest_of(&self, kinds: &[SerialKind]) -> u32 {
        kinds
            .iter()
            .filter_map(|kind| self.serials.get(kind))
            .max_by_key(|serial_data| serial_data.serial)
            .map(|serial_data| serial_data.serial)
            .unwrap_or(0)
    }
}
