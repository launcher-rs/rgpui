//! 滚动物理模型：处理滚动惯性（动量）、减速度与越界回弹（overscroll）。

/// 滚动物理状态机。
///
/// 提供滚动位置跟踪、惯性滑动（fling）衰减、边界钳制与越界回弹能力。
/// 以帧为单位的 `tick` 驱动，返回是否仍在运动。
#[derive(Clone, Debug)]
pub struct ScrollPhysics {
    /// 当前速度（像素/帧的等效比例）。
    velocity: f32,
    /// 当前滚动位置。
    position: f32,
    /// 滚动下边界。
    min_bound: f32,
    /// 滚动上边界。
    max_bound: f32,
    /// 惯性衰减系数（0.8~0.99，越小减速越快）。
    deceleration: f32,
    /// 越界回弹阻力（0.0~1.0，越大回弹越快）。
    overscroll_resistance: f32,
    /// 是否启用惯性滑动。
    momentum_enabled: bool,
    /// 是否启用越界回弹。
    overscroll_enabled: bool,
}

impl Default for ScrollPhysics {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollPhysics {
    /// 创建默认滚动物理模型。
    pub fn new() -> Self {
        Self {
            velocity: 0.0,
            position: 0.0,
            min_bound: 0.0,
            max_bound: f32::MAX,
            deceleration: 0.95,
            overscroll_resistance: 0.3,
            momentum_enabled: true,
            overscroll_enabled: true,
        }
    }

    /// 设置滚动边界。
    pub fn with_bounds(mut self, min: f32, max: f32) -> Self {
        self.min_bound = min;
        self.max_bound = max;
        self
    }

    /// 设置惯性衰减系数（自动钳制在 0.8~0.99）。
    pub fn with_deceleration(mut self, deceleration: f32) -> Self {
        self.deceleration = deceleration.clamp(0.8, 0.99);
        self
    }

    /// 设置越界回弹阻力（自动钳制在 0.0~1.0）。
    pub fn with_overscroll_resistance(mut self, resistance: f32) -> Self {
        self.overscroll_resistance = resistance.clamp(0.0, 1.0);
        self
    }

    /// 设置是否启用惯性滑动。
    pub fn momentum(mut self, enabled: bool) -> Self {
        self.momentum_enabled = enabled;
        self
    }

    /// 设置是否启用越界回弹。
    pub fn overscroll(mut self, enabled: bool) -> Self {
        self.overscroll_enabled = enabled;
        self
    }

    /// 设置滚动边界。
    pub fn set_bounds(&mut self, min: f32, max: f32) {
        self.min_bound = min;
        self.max_bound = max;
    }

    /// 应用一次手指/滚轮位移，并更新速度。
    pub fn apply_delta(&mut self, delta: f32) {
        if self.momentum_enabled {
            self.velocity = delta * 0.8 + self.velocity * 0.2;
        } else {
            self.velocity = 0.0;
        }
        self.position += delta;

        if !self.overscroll_enabled {
            self.position = self.position.clamp(self.min_bound, self.max_bound);
        }
    }

    /// 推进一帧物理模拟，返回是否仍在运动。
    ///
    /// 处理速度衰减、位移累计以及越界回弹或边界钳制。
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.momentum_enabled && !self.is_overscrolled() {
            return false;
        }

        self.velocity *= self.deceleration;
        self.position += self.velocity * dt * 60.0;

        if self.overscroll_enabled {
            if self.position < self.min_bound {
                let overshoot = self.min_bound - self.position;
                // 回弹位移接近边界时直接钳制到位，避免几何级数衰减永不收敛。
                if overshoot < 0.5 {
                    self.position = self.min_bound;
                    self.velocity = 0.0;
                } else {
                    self.position += overshoot * self.overscroll_resistance;
                    self.velocity *= 0.5;
                }
            }
            if self.position > self.max_bound {
                let overshoot = self.position - self.max_bound;
                if overshoot < 0.5 {
                    self.position = self.max_bound;
                    self.velocity = 0.0;
                } else {
                    self.position -= overshoot * self.overscroll_resistance;
                    self.velocity *= 0.5;
                }
            }
        } else {
            self.position = self.position.clamp(self.min_bound, self.max_bound);
            if self.position <= self.min_bound || self.position >= self.max_bound {
                self.velocity = 0.0;
            }
        }

        self.velocity.abs() > 0.5 || self.is_overscrolled()
    }

    /// 返回当前滚动位置。
    pub fn position(&self) -> f32 {
        self.position
    }

    /// 返回当前速度。
    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    /// 是否仍在运动（速度大于阈值）。
    pub fn is_moving(&self) -> bool {
        self.velocity.abs() > 0.5
    }

    /// 是否处于越界状态。
    pub fn is_overscrolled(&self) -> bool {
        self.position < self.min_bound || self.position > self.max_bound
    }

    /// 停止运动（速度清零）。
    pub fn stop(&mut self) {
        self.velocity = 0.0;
    }

    /// 重置到边界起点。
    pub fn reset(&mut self) {
        self.velocity = 0.0;
        self.position = self.min_bound;
    }

    /// 直接设置滚动位置。
    pub fn set_position(&mut self, position: f32) {
        self.position = position;
    }

    /// 平滑滚动到指定位置（自动钳制在边界内并停止运动）。
    pub fn scroll_to(&mut self, position: f32) {
        self.position = position.clamp(self.min_bound, self.max_bound);
        self.velocity = 0.0;
    }

    /// 以给定速度开始惯性滑动（fling）。
    pub fn fling(&mut self, velocity: f32) {
        self.velocity = velocity;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_delta_clamps_within_bounds() {
        let mut physics = ScrollPhysics::new()
            .with_bounds(0.0, 100.0)
            .momentum(false)
            .overscroll(false);
        physics.apply_delta(150.0);
        assert_eq!(physics.position(), 100.0);
        physics.apply_delta(-200.0);
        assert_eq!(physics.position(), 0.0);
    }

    #[test]
    fn test_tick_stops_when_velocity_decays() {
        let mut physics = ScrollPhysics::new()
            .with_bounds(0.0, 100.0)
            .overscroll(false);
        physics.fling(50.0);
        let mut frames = 0;
        while physics.tick(1.0 / 60.0) {
            frames += 1;
            assert!(frames < 1000, "惯性滑动应最终停止");
        }
        assert_eq!(physics.velocity(), 0.0);
    }

    #[test]
    fn test_overscroll_springs_back() {
        let mut physics = ScrollPhysics::new().with_bounds(0.0, 100.0);
        physics.set_position(-20.0);
        assert!(physics.is_overscrolled());
        let mut frames = 0;
        while physics.tick(1.0 / 60.0) {
            frames += 1;
            assert!(frames < 1000, "回弹应最终回到边界内");
        }
        assert!(!physics.is_overscrolled());
    }

    #[test]
    fn test_scroll_to_clamps() {
        let mut physics = ScrollPhysics::new().with_bounds(0.0, 100.0);
        physics.scroll_to(500.0);
        assert_eq!(physics.position(), 100.0);
        assert_eq!(physics.velocity(), 0.0);
    }
}
