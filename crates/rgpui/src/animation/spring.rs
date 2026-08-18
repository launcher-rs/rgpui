use std::time::Duration;

/// 弹簧物理模拟（值驱动动画原语）。
///
/// 核心 `crate::Animation` 是时间驱动的（0~1 进度），而弹簧是**值驱动**的：
/// 通过刚度、阻尼、质量三参数模拟真实的回弹运动，适合按钮按压、面板展开、
/// 卡片拖拽等需要惯性回弹的手感。
///
/// 本类型为纯 Rust 实现、不依赖任何 UI 类型，必要时可自由抽出为独立库。
#[derive(Clone, Debug)]
pub struct Spring {
    /// 当前位置。
    pub position: f32,
    /// 当前速度。
    pub velocity: f32,
    /// 目标位置。
    pub target: f32,
    /// 刚度（越大回弹越快）。
    stiffness: f32,
    /// 阻尼（越大越不易振荡）。
    damping: f32,
    /// 质量（越大运动越迟滞）。
    mass: f32,
    /// 静止判定阈值，位移与速度均小于该值时视为到位。
    rest_threshold: f32,
}

impl Default for Spring {
    fn default() -> Self {
        Self::gentle()
    }
}

impl Spring {
    /// 以指定参数创建弹簧。
    ///
    /// - `stiffness`：刚度，会被钳制到至少 0.1。
    /// - `damping`：阻尼，会被钳制到非负。
    /// - `mass`：质量，会被钳制到至少 0.01。
    pub fn new(stiffness: f32, damping: f32, mass: f32) -> Self {
        Self {
            position: 0.0,
            velocity: 0.0,
            target: 0.0,
            stiffness: stiffness.max(0.1),
            damping: damping.max(0.0),
            mass: mass.max(0.01),
            rest_threshold: 0.001,
        }
    }

    /// 柔和预设（标准回弹）。
    pub fn gentle() -> Self {
        Self::new(120.0, 14.0, 1.0)
    }

    /// 晃动预设（较强回弹）。
    pub fn wobbly() -> Self {
        Self::new(180.0, 12.0, 1.0)
    }

    /// 硬朗预设（快速到位，轻微回弹）。
    pub fn stiff() -> Self {
        Self::new(210.0, 20.0, 1.0)
    }

    /// 缓慢预设（平稳无振荡）。
    pub fn slow() -> Self {
        Self::new(280.0, 60.0, 1.0)
    }

    /// 敏捷预设（快速响应）。
    pub fn snappy() -> Self {
        Self::new(400.0, 30.0, 1.0)
    }

    /// 设置初始位置。
    pub fn with_position(mut self, position: f32) -> Self {
        self.position = position;
        self
    }

    /// 设置目标位置。
    pub fn with_target(mut self, target: f32) -> Self {
        self.target = target;
        self
    }

    /// 设置初始速度。
    pub fn with_velocity(mut self, velocity: f32) -> Self {
        self.velocity = velocity;
        self
    }

    /// 设置静止判定阈值，至少为 0.0001。
    pub fn with_rest_threshold(mut self, threshold: f32) -> Self {
        self.rest_threshold = threshold.max(0.0001);
        self
    }

    /// 更新目标位置。
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// 更新当前位置。
    pub fn set_position(&mut self, position: f32) {
        self.position = position;
    }

    /// 施加一个瞬时速度冲量。
    pub fn impulse(&mut self, velocity: f32) {
        self.velocity += velocity;
    }

    /// 按时间步长推进弹簧模拟，返回是否仍在运动中（未到静止）。
    ///
    /// 单步时间会被钳制到 64ms，避免大帧间隔导致数值发散。
    pub fn tick(&mut self, dt: f32) -> bool {
        let dt = dt.min(0.064);

        let displacement = self.position - self.target;
        let spring_force = -self.stiffness * displacement;
        let damping_force = -self.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.mass;

        self.velocity += acceleration * dt;
        self.position += self.velocity * dt;

        let is_moving = self.velocity.abs() > self.rest_threshold
            || (self.position - self.target).abs() > self.rest_threshold;

        if !is_moving {
            self.position = self.target;
            self.velocity = 0.0;
        }

        is_moving
    }

    /// 按 `Duration` 推进弹簧模拟（等价于 `tick(dt.as_secs_f32())`）。
    pub fn tick_duration(&mut self, duration: Duration) -> bool {
        self.tick(duration.as_secs_f32())
    }

    /// 判断弹簧是否已静止（速度与位移均小于静止阈值）。
    pub fn is_at_rest(&self) -> bool {
        self.velocity.abs() <= self.rest_threshold
            && (self.position - self.target).abs() <= self.rest_threshold
    }

    /// 计算动画进度（0~1.5），用于把弹簧位置映射为渲染进度。
    ///
    /// 目标为 0 时进度为 1.0；进度超过 1.0 表示越过目标（回弹阶段）。
    pub fn progress(&self) -> f32 {
        if (self.target - self.position).abs() < self.rest_threshold {
            return 1.0;
        }
        let start = 0.0_f32;
        let total = self.target - start;
        if total.abs() < f32::EPSILON {
            return 1.0;
        }
        ((self.position - start) / total).clamp(0.0, 1.5)
    }

    /// 重置到初始状态（位置、速度归零）。
    pub fn reset(&mut self) {
        self.position = 0.0;
        self.velocity = 0.0;
    }

    /// 直接吸附到目标位置（瞬间到位，无动画）。
    pub fn snap_to_target(&mut self) {
        self.position = self.target;
        self.velocity = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证弹簧最终会稳定收敛到目标位置。
    #[test]
    fn test_spring_reaches_target() {
        let mut spring = Spring::gentle().with_target(100.0);
        // 模拟 2 秒（超过单步钳制，按步推进）
        for _ in 0..1000 {
            spring.tick(0.002);
        }
        assert!(spring.is_at_rest());
        assert!((spring.position - 100.0).abs() < 0.01);
    }

    /// 验证冲量会产生越位回弹（进度超过 1.0）。
    #[test]
    fn test_spring_impulse_overshoots() {
        let mut spring = Spring::stiff().with_target(50.0);
        spring.impulse(400.0);
        let mut max_progress = 0.0_f32;
        for _ in 0..600 {
            spring.tick(0.002);
            max_progress = max_progress.max(spring.progress());
        }
        // 冲量足够大时应越过目标再回弹
        assert!(max_progress > 1.0, "max_progress = {max_progress}");
    }

    /// 验证快照直接到位。
    #[test]
    fn test_snap_to_target() {
        let mut spring = Spring::gentle().with_target(42.0);
        spring.snap_to_target();
        assert!(spring.is_at_rest());
        assert_eq!(spring.position, 42.0);
    }
}
