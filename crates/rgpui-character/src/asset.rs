use std::collections::HashMap;

use crate::animation::AnimationClip;
use crate::core::TextureId;

/// 资源系统错误。
#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    /// 请求的纹理不存在。
    #[error("纹理不存在: {0}")]
    TextureMissing(String),
    /// 请求的动画不存在。
    #[error("动画不存在: {0}")]
    AnimationMissing(String),
}

/// 角色资源管理器，负责纹理和动画索引。
#[derive(Clone, Debug, Default)]
pub struct AssetManager {
    /// 已注册纹理。
    pub textures: HashMap<String, TextureId>,
    /// 已注册动画。
    pub animations: HashMap<String, AnimationClip>,
}

impl AssetManager {
    /// 创建空资源管理器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个纹理标识。
    pub fn register_texture(&mut self, name: impl Into<String>, texture: TextureId) {
        self.textures.insert(name.into(), texture);
    }

    /// 获取已注册纹理标识。
    pub fn texture(&self, name: &str) -> Result<&TextureId, AssetError> {
        self.textures
            .get(name)
            .ok_or_else(|| AssetError::TextureMissing(name.to_string()))
    }

    /// 注册一个动画片段。
    pub fn register_animation(&mut self, clip: AnimationClip) {
        self.animations.insert(clip.name.clone(), clip);
    }

    /// 获取已注册动画片段。
    pub fn animation(&self, name: &str) -> Result<&AnimationClip, AssetError> {
        self.animations
            .get(name)
            .ok_or_else(|| AssetError::AnimationMissing(name.to_string()))
    }
}
