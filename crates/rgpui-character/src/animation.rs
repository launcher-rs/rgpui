use crate::core::TextureId;

/// 一段基于精灵图帧序列的动画片段。
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationClip {
    /// 动画名称。
    pub name: String,
    /// 动画帧纹理序列。
    pub frames: Vec<TextureId>,
    /// 每秒播放帧数。
    pub fps: f32,
    /// 是否循环播放。
    pub looped: bool,
}

impl AnimationClip {
    /// 创建一个新的动画片段。
    pub fn new(name: impl Into<String>, frames: Vec<TextureId>, fps: f32, looped: bool) -> Self {
        Self {
            name: name.into(),
            frames,
            fps,
            looped,
        }
    }
}

/// 动画播放器，记录当前播放时间和帧索引。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimationPlayer {
    /// 当前播放时间，单位为秒。
    pub current_time: f32,
    /// 当前帧索引。
    pub current_frame: usize,
}

impl AnimationPlayer {
    /// 重置播放状态到第一帧。
    pub fn reset(&mut self) {
        self.current_time = 0.0;
        self.current_frame = 0;
    }

    /// 按时间增量推进动画帧。
    pub fn update(&mut self, dt: f32, clip: &AnimationClip) {
        let frame_count = clip.frames.len();
        if frame_count == 0 || clip.fps <= 0.0 {
            self.current_frame = 0;
            return;
        }

        self.current_time += dt.max(0.0);
        let frame = (self.current_time * clip.fps) as usize;
        self.current_frame = if clip.looped {
            frame % frame_count
        } else {
            frame.min(frame_count.saturating_sub(1))
        };
    }
}

/// 角色当前动画状态。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimationState {
    /// 当前动画名称。
    pub current_clip: Option<String>,
    /// 当前动画播放器。
    pub player: AnimationPlayer,
    /// 当前帧纹理。
    current_texture: Option<TextureId>,
}

impl AnimationState {
    /// 切换到指定动画片段。
    pub fn play(&mut self, clip_name: impl Into<String>) {
        let clip_name = clip_name.into();
        if self.current_clip.as_deref() != Some(clip_name.as_str()) {
            self.current_clip = Some(clip_name);
            self.player.reset();
            self.current_texture = None;
        }
    }

    /// 使用动画片段推进当前动画状态。
    pub fn update(&mut self, dt: f32, clip: Option<&AnimationClip>) {
        let Some(clip) = clip else {
            self.current_texture = None;
            return;
        };

        self.player.update(dt, clip);
        self.current_texture = clip.frames.get(self.player.current_frame).cloned();
    }

    /// 返回当前帧纹理。
    pub fn current_texture(&self) -> Option<&TextureId> {
        self.current_texture.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造测试用纹理标识。
    fn texture(id: &str) -> TextureId {
        TextureId::new(id)
    }

    #[test]
    /// 验证循环动画会按帧率推进并回绕。
    fn looped_animation_wraps_frame_index() {
        let clip = AnimationClip::new(
            "idle",
            vec![texture("idle-0"), texture("idle-1"), texture("idle-2")],
            10.0,
            true,
        );
        let mut player = AnimationPlayer::default();

        player.update(0.2, &clip);
        assert_eq!(player.current_frame, 2);

        player.update(0.2, &clip);
        assert_eq!(player.current_frame, 1);
    }

    #[test]
    /// 验证非循环动画会停在最后一帧。
    fn non_looped_animation_stops_at_last_frame() {
        let clip = AnimationClip::new(
            "sleep",
            vec![texture("sleep-0"), texture("sleep-1")],
            8.0,
            false,
        );
        let mut player = AnimationPlayer::default();

        player.update(10.0, &clip);

        assert_eq!(player.current_frame, 1);
    }
}
