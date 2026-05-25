use crate::animation::AnimationState;
use crate::behavior::BehaviorState;
use crate::render::RenderCommand;

/// 二维向量，用于表达位置、速度和缩放。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    /// 水平方向分量。
    pub x: f32,
    /// 垂直方向分量。
    pub y: f32,
}

impl Vec2 {
    /// 创建一个新的二维向量。
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// 返回零向量。
    pub const fn zero() -> Self {
        Self::new(0.0, 0.0)
    }

    /// 返回单位缩放向量。
    pub const fn one() -> Self {
        Self::new(1.0, 1.0)
    }

    /// 按标量缩放当前向量。
    pub fn scale(self, value: f32) -> Self {
        Self::new(self.x * value, self.y * value)
    }
}

impl std::ops::Add for Vec2 {
    type Output = Self;

    /// 将两个二维向量相加。
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl std::ops::AddAssign for Vec2 {
    /// 将另一个二维向量累加到当前向量。
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Self;

    /// 将二维向量乘以标量。
    fn mul(self, rhs: f32) -> Self::Output {
        self.scale(rhs)
    }
}

impl std::ops::MulAssign<f32> for Vec2 {
    /// 将当前二维向量按标量原地缩放。
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

/// 矩形区域，用于表达桌宠活动边界。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// 左上角横坐标。
    pub x: f32,
    /// 左上角纵坐标。
    pub y: f32,
    /// 矩形宽度。
    pub width: f32,
    /// 矩形高度。
    pub height: f32,
}

impl Rect {
    /// 创建一个新的矩形区域。
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 返回矩形右边界坐标。
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    /// 返回矩形下边界坐标。
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    /// 将点限制在矩形范围内。
    pub fn clamp_point(self, point: Vec2) -> Vec2 {
        Vec2::new(
            point.x.clamp(self.x, self.right()),
            point.y.clamp(self.y, self.bottom()),
        )
    }
}

/// 纹理资源标识，由宿主渲染层解释为真实图片资源。
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TextureId(pub String);

impl TextureId {
    /// 创建一个新的纹理资源标识。
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// 角色当前行为阶段。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterState {
    /// 空闲状态。
    Idle,
    /// 行走状态。
    Walk,
    /// 睡眠状态。
    Sleep,
    /// 鼠标拖拽状态。
    Dragging,
    /// 自定义状态。
    Custom(String),
}

impl Default for CharacterState {
    /// 返回默认的空闲状态。
    fn default() -> Self {
        Self::Idle
    }
}

/// 单个 UI 角色的核心数据模型。
#[derive(Clone, Debug)]
pub struct Character {
    /// 角色唯一标识。
    pub id: String,
    /// 角色当前状态。
    pub state: CharacterState,
    /// 角色当前位置。
    pub position: Vec2,
    /// 角色当前速度。
    pub velocity: Vec2,
    /// 角色渲染缩放（负 x 表示水平翻转）。
    pub scale: Vec2,
    /// 角色渲染旋转角度，单位为弧度。
    pub rotation: f32,
    /// 角色是否朝左（用于方向判断）。
    pub facing_left: bool,
    /// 角色动画状态。
    pub animation: AnimationState,
    /// 角色行为状态。
    pub behavior: BehaviorState,
}

impl Character {
    /// 创建一个新的角色。
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state: CharacterState::Idle,
            position: Vec2::zero(),
            velocity: Vec2::zero(),
            scale: Vec2::one(),
            rotation: 0.0,
            facing_left: false,
            animation: AnimationState::default(),
            behavior: BehaviorState::default(),
        }
    }

    /// 设置角色水平朝向，自动调整 scale.x 实现翻转。
    pub fn set_facing(&mut self, left: bool) {
        self.facing_left = left;
        if left {
            self.scale.x = -self.scale.x.abs();
        } else {
            self.scale.x = self.scale.x.abs();
        }
    }

    /// 翻转角色朝向。
    pub fn flip_facing(&mut self) {
        self.set_facing(!self.facing_left);
    }

    /// 根据当前角色动画和位置构建渲染命令。
    pub fn build_render(&self) -> Option<RenderCommand> {
        self.animation
            .current_texture()
            .cloned()
            .map(|texture| RenderCommand::DrawSprite {
                texture,
                position: self.position,
                scale: self.scale,
                rotation: self.rotation,
            })
    }
}
