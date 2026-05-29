use crate::core::{TextureId, Vec2};

/// 角色运行时输出的渲染命令。
#[derive(Clone, Debug, PartialEq)]
pub enum RenderCommand {
    /// 绘制一个 sprite 纹理。
    DrawSprite {
        /// 要绘制的纹理标识。
        texture: TextureId,
        /// 绘制位置。
        position: Vec2,
        /// 绘制缩放。
        scale: Vec2,
        /// 绘制旋转角度，单位为弧度。
        rotation: f32,
    },
}

/// 渲染后端桥接接口。
pub trait RenderBackend {
    /// 绘制一条角色渲染命令。
    fn draw_sprite(&mut self, cmd: &RenderCommand);
}
