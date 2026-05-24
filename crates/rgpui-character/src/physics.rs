use crate::core::{Rect, Vec2};

/// 轻量物理系统配置。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsConfig {
    /// 每帧速度保留比例。
    pub friction: f32,
    /// 角色活动边界。
    pub bounds: Rect,
    /// 碰到边界时是否反弹。
    pub bounce: bool,
}

impl Default for PhysicsConfig {
    /// 返回桌宠原型使用的默认物理配置。
    fn default() -> Self {
        Self {
            friction: 0.92,
            bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
            bounce: true,
        }
    }
}

/// 根据速度、摩擦和边界更新角色位置。
pub fn update_physics(position: &mut Vec2, velocity: &mut Vec2, dt: f32, config: PhysicsConfig) {
    *position += *velocity * dt.max(0.0);
    velocity.x *= config.friction;
    velocity.y *= config.friction;

    if position.x < config.bounds.x || position.x > config.bounds.right() {
        position.x = position.x.clamp(config.bounds.x, config.bounds.right());
        if config.bounce {
            velocity.x = -velocity.x;
        }
    }

    if position.y < config.bounds.y || position.y > config.bounds.bottom() {
        position.y = position.y.clamp(config.bounds.y, config.bounds.bottom());
        if config.bounce {
            velocity.y = -velocity.y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// 验证角色越过右边界时会被限制并反弹。
    fn clamps_and_bounces_at_bounds() {
        let config = PhysicsConfig {
            friction: 1.0,
            bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
            bounce: true,
        };
        let mut position = Vec2::new(9.0, 5.0);
        let mut velocity = Vec2::new(4.0, 0.0);

        update_physics(&mut position, &mut velocity, 1.0, config);

        assert_eq!(position, Vec2::new(10.0, 5.0));
        assert_eq!(velocity, Vec2::new(-4.0, 0.0));
    }
}
