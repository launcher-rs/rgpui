use crate::asset::AssetManager;
use crate::behavior::{Behavior, BehaviorAction, BehaviorContext};
use crate::core::Character;
use crate::physics::{PhysicsConfig, update_physics};
use crate::render::RenderCommand;

/// 角色交互事件。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterEvent {
    /// 点击角色。
    Click,
    /// 悬停角色。
    Hover,
    /// 开始拖拽角色。
    DragStart,
    /// 结束拖拽角色。
    DragEnd,
    /// 自定义事件。
    Custom(String),
}

/// 多角色运行时调度器。
#[derive(Clone, Debug, Default)]
pub struct CharacterRuntime {
    /// 运行时内的角色列表。
    pub characters: Vec<Character>,
    /// 运行时资源管理器。
    pub assets: AssetManager,
    /// 运行时物理配置。
    pub physics: PhysicsConfig,
}

impl CharacterRuntime {
    /// 创建空角色运行时。
    pub fn new() -> Self {
        Self::default()
    }

    /// 向运行时添加角色。
    pub fn add_character(&mut self, character: Character) {
        self.characters.push(character);
    }

    /// 派发角色事件。
    pub fn dispatch_event(&mut self, character_id: &str, event: CharacterEvent) {
        if let Some(character) = self
            .characters
            .iter_mut()
            .find(|character| character.id == character_id)
        {
            match event {
                CharacterEvent::DragStart => {
                    character.state = crate::core::CharacterState::Dragging
                }
                CharacterEvent::DragEnd => character.state = crate::core::CharacterState::Idle,
                CharacterEvent::Click | CharacterEvent::Hover | CharacterEvent::Custom(_) => {}
            }
        }
    }

    /// 使用同一个行为更新所有角色，并返回渲染命令。
    pub fn update<B>(&mut self, dt: f32, behavior: &mut B) -> Vec<RenderCommand>
    where
        B: Behavior,
    {
        let mut commands = Vec::with_capacity(self.characters.len());

        for character in &mut self.characters {
            let mut ctx = BehaviorContext {
                state: character.state.clone(),
                position: character.position,
                velocity: character.velocity,
                facing_left: character.facing_left,
                dt,
            };
            let action = behavior.update(&mut ctx);
            Self::apply_action(character, action);

            // 根据速度方向自动更新角色朝向
            if character.velocity.x.abs() > 1.0 {
                character.set_facing(character.velocity.x < 0.0);
            }

            update_physics(
                &mut character.position,
                &mut character.velocity,
                dt,
                self.physics,
            );

            let clip = character
                .animation
                .current_clip
                .as_deref()
                .and_then(|name| self.assets.animations.get(name));
            character.animation.update(dt, clip);

            if let Some(command) = character.build_render() {
                commands.push(command);
            }
        }

        commands
    }

    /// 将行为动作应用到角色数据上。
    fn apply_action(character: &mut Character, action: BehaviorAction) {
        match action {
            BehaviorAction::Move(impulse) => character.velocity += impulse,
            BehaviorAction::ChangeState(state) => character.state = state,
            BehaviorAction::PlayAnimation(name) => {
                character.behavior.requested_animation = Some(name.clone());
                character.animation.play(name);
            }
            BehaviorAction::Idle => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::animation::AnimationClip;
    use crate::behavior::ConstantMoveBehavior;
    use crate::core::{TextureId, Vec2};

    use super::*;

    #[test]
    /// 验证运行时会调度行为、物理、动画并输出渲染命令。
    fn runtime_updates_character_and_builds_render_command() {
        let mut runtime = CharacterRuntime::new();
        runtime.physics.friction = 1.0;
        runtime.assets.register_animation(AnimationClip::new(
            "idle",
            vec![TextureId::new("idle-0")],
            1.0,
            true,
        ));

        let mut character = Character::new("pet");
        character.animation.play("idle");
        runtime.add_character(character);

        let mut behavior = ConstantMoveBehavior::new(Vec2::new(10.0, 0.0));
        let commands = runtime.update(1.0, &mut behavior);

        assert_eq!(runtime.characters[0].position, Vec2::new(10.0, 0.0));
        assert_eq!(commands.len(), 1);
    }
}
