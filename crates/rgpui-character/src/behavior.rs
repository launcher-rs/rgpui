use crate::core::{CharacterState, Vec2};

/// 行为系统每帧可读取的上下文。
#[derive(Clone, Debug)]
pub struct BehaviorContext {
    /// 当前角色状态。
    pub state: CharacterState,
    /// 当前角色位置。
    pub position: Vec2,
    /// 当前角色速度。
    pub velocity: Vec2,
    /// 角色是否朝左。
    pub facing_left: bool,
    /// 本帧时间增量，单位为秒。
    pub dt: f32,
}

/// 行为系统输出的角色意图。
#[derive(Clone, Debug, PartialEq)]
pub enum BehaviorAction {
    /// 增加角色移动速度。
    Move(Vec2),
    /// 切换角色状态。
    ChangeState(CharacterState),
    /// 播放指定动画。
    PlayAnimation(String),
    /// 本帧不做行为变化。
    Idle,
}

/// 可插拔角色行为接口。
pub trait Behavior {
    /// 根据上下文更新行为，并返回本帧动作。
    fn update(&mut self, ctx: &mut BehaviorContext) -> BehaviorAction;
}

/// 角色行为运行状态。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BehaviorState {
    /// 最近一次请求播放的动画名称。
    pub requested_animation: Option<String>,
}

/// 始终保持空闲的基础行为。
#[derive(Clone, Debug, Default)]
pub struct IdleBehavior;

impl Behavior for IdleBehavior {
    /// 返回空闲动作。
    fn update(&mut self, _ctx: &mut BehaviorContext) -> BehaviorAction {
        BehaviorAction::Idle
    }
}

/// 简单的定向移动行为，用于桌宠原型调试。
#[derive(Clone, Debug)]
pub struct ConstantMoveBehavior {
    /// 每帧期望增加的速度。
    pub impulse: Vec2,
}

impl ConstantMoveBehavior {
    /// 创建一个定向移动行为。
    pub fn new(impulse: Vec2) -> Self {
        Self { impulse }
    }
}

impl Behavior for ConstantMoveBehavior {
    /// 返回定向移动动作。
    fn update(&mut self, _ctx: &mut BehaviorContext) -> BehaviorAction {
        BehaviorAction::Move(self.impulse)
    }
}
