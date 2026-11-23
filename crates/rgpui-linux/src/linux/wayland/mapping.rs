//! 普通 Wayland 窗口的隐藏/重新映射状态，不持有协议或 GPU 资源。

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Visible,
    Hidden,
    WaitingForSync,
    WaitingForConfigure,
    WaitingForFrame,
}

#[derive(Debug, Default)]
pub(super) struct WindowMapping {
    phase: Phase,
    /// 隐藏/恢复周期编号
    generation: u64,
}

impl WindowMapping {
    /// 隐藏使已排队的帧回调和恢复请求失效；重复隐藏不重复取消映射。
    pub fn hide(&mut self) -> bool {
        if self.phase == Phase::Hidden {
            return false;
        }
        self.generation = self.generation.wrapping_add(1);
        self.phase = Phase::Hidden;
        true
    }

    /// 只为首次恢复安排同步屏障，连续点击共享同一次恢复。
    pub fn show(&mut self) -> Option<u64> {
        if self.phase != Phase::Hidden {
            return None;
        }
        self.phase = Phase::WaitingForSync;
        Some(self.generation)
    }

    /// 旧事件排空后才允许发送初始空提交，并接收新一轮 configure。
    pub fn synced(&mut self, generation: u64) -> bool {
        if generation != self.generation || self.phase != Phase::WaitingForSync {
            return false;
        }
        self.phase = Phase::WaitingForConfigure;
        true
    }

    /// ack_configure 之后才能绘制；首帧提交之前暂不消费激活令牌。
    pub fn configured(&mut self) {
        if self.phase == Phase::WaitingForConfigure {
            self.phase = Phase::WaitingForFrame;
        }
    }

    /// 只在恢复后的第一帧成功提交时请求激活。
    pub fn presented(&mut self) -> bool {
        if self.phase != Phase::WaitingForFrame {
            return false;
        }
        self.phase = Phase::Visible;
        true
    }

    /// 隐藏和排空旧事件阶段不得处理旧配置。
    pub fn accepts_configure(&self) -> bool {
        !matches!(self.phase, Phase::Hidden | Phase::WaitingForSync)
    }

    /// 等待新配置期间禁止提交缓冲区，避免 unconfigured_buffer 协议错误。
    pub fn can_render(&self) -> bool {
        matches!(self.phase, Phase::Visible | Phase::WaitingForFrame)
    }

    /// 新激活请求在恢复首帧完成之前需要延期。
    pub fn is_visible(&self) -> bool {
        self.phase == Phase::Visible
    }

    /// 完全隐藏时丢弃迟到的激活令牌，避免窗口自行重现。
    pub fn is_hidden(&self) -> bool {
        self.phase == Phase::Hidden
    }

    /// 帧与同步回调携带代数，用于丢弃上一轮显示的回调。
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 隐藏期间的后台输出和旧配置都不能使窗口重新映射。
    #[test]
    fn hidden_window_cannot_render_or_accept_configure() {
        let mut mapping = WindowMapping::default();
        assert!(mapping.hide());
        assert!(!mapping.hide());
        mapping.configured();
        assert!(!mapping.can_render());
        assert!(!mapping.accepts_configure());
        assert!(!mapping.presented());
    }

    // 完整恢复必须按同步、配置、首帧的顺序完成，连续点击不会重复请求。
    #[test]
    fn restore_waits_for_configure_and_first_frame() {
        let mut mapping = WindowMapping::default();
        mapping.hide();
        let generation = mapping.show().unwrap();
        assert_eq!(mapping.show(), None);
        assert!(!mapping.can_render());
        assert!(!mapping.accepts_configure());
        assert!(mapping.synced(generation));
        assert!(!mapping.synced(generation));
        assert!(mapping.accepts_configure());
        assert!(!mapping.can_render());
        mapping.configured();
        assert!(mapping.can_render());
        assert!(!mapping.is_visible());
        assert!(mapping.presented());
        assert!(!mapping.presented());
        assert!(mapping.is_visible());
    }

    // 恢复过程中再次隐藏后，迟到的同步和帧回调不得恢复旧窗口状态。
    #[test]
    fn hide_cancels_every_restore_phase() {
        for phase in 0..3 {
            let mut mapping = WindowMapping::default();
            mapping.hide();
            let old = mapping.show().unwrap();
            if phase >= 1 {
                mapping.synced(old);
            }
            if phase >= 2 {
                mapping.configured();
            }
            mapping.hide();
            assert!(mapping.is_hidden());
            assert_ne!(mapping.generation(), old);
            let new = mapping.show().unwrap();
            assert!(!mapping.synced(old));
            assert!(!mapping.presented());
            assert!(mapping.synced(new));
            mapping.configured();
            assert!(mapping.presented());
        }
    }
}
