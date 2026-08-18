//! 庆祝彩带粒子爆发组件：配置好的彩带粒子阵，通过 canvas 绘制。

use crate::*;
use std::time::Duration;

const DEFAULT_CONFETTI_COLORS: [u32; 6] =
    [0xFF6B6B, 0x4ECDC4, 0x45B7D1, 0xFFA07A, 0x98D8C8, 0xF7DC6F];

/// 单个彩带粒子的状态。
#[derive(Clone)]
pub struct ConfettiParticle {
    /// 归一化位置（0~1，相对容器）。
    pub position: Point<f32>,
    /// 速度。
    pub velocity: Point<f32>,
    /// 旋转速度。
    pub rotation_speed: f32,
    /// 粒子尺寸。
    pub size: f32,
    /// 颜色。
    pub color: Hsla,
    /// 已存活时长。
    pub age: f32,
    /// 寿命。
    pub lifetime: f32,
}

/// 彩带粒子系统状态。
pub struct ConfettiState {
    /// 是否正在播放。
    is_active: bool,
    /// 粒子数组。
    particles: Vec<ConfettiParticle>,
    /// 粒子数量。
    particle_count: usize,
    /// 颜色列表。
    colors: Vec<Hsla>,
    /// 重力加速度。
    gravity: f32,
    /// 爆发原点（归一化）。
    origin: Point<f32>,
    /// 爆发扩散速度。
    spread: f32,
}

impl ConfettiState {
    /// 创建彩带系统，默认 80 粒子、重力 120、扩散 300。
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            is_active: false,
            particles: Vec::new(),
            particle_count: 80,
            colors: DEFAULT_CONFETTI_COLORS
                .iter()
                .map(|&c| {
                    let color: Rgba = rgb(c);
                    Hsla::from(color)
                })
                .collect(),
            gravity: 120.0,
            origin: Point { x: 0.5, y: 0.5 },
            spread: 300.0,
        }
    }

    /// 设置粒子数量。
    pub fn set_particle_count(&mut self, count: usize) {
        self.particle_count = count;
    }

    /// 设置颜色列表，空列表忽略。
    pub fn set_colors(&mut self, colors: Vec<Hsla>) {
        if !colors.is_empty() {
            self.colors = colors;
        }
    }

    /// 设置重力加速度。
    pub fn set_gravity(&mut self, gravity: f32) {
        self.gravity = gravity;
    }

    /// 设置爆发原点（归一化坐标）。
    pub fn set_origin(&mut self, origin: Point<f32>) {
        self.origin = origin;
    }

    /// 是否正在播放。
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// 触发一次彩带爆发。
    pub fn burst(&mut self, cx: &mut Context<Self>) {
        self.particles.clear();
        self.is_active = true;

        let count = self.particle_count;
        let color_count = self.colors.len().max(1);

        for i in 0..count {
            let seed = i as u32;
            let angle = pseudo_random_f32(seed) * std::f32::consts::TAU;
            let speed = self.spread * (0.3 + pseudo_random_f32(seed + 3) * 0.7);

            let vx = angle.cos() * speed;
            let vy = angle.sin() * speed - self.spread * 0.5;

            let color_idx =
                (pseudo_random_f32(seed + 7) * color_count as f32) as usize % color_count;
            let particle_size = 4.0 + pseudo_random_f32(seed + 11) * 6.0;
            let rotation_spd = (pseudo_random_f32(seed + 17) - 0.5) * 10.0;
            let lifetime = 1.5 + pseudo_random_f32(seed + 23) * 1.5;

            self.particles.push(ConfettiParticle {
                position: Point {
                    x: self.origin.x,
                    y: self.origin.y,
                },
                velocity: Point { x: vx, y: vy },
                rotation_speed: rotation_spd,
                size: particle_size,
                color: self.colors[color_idx],
                age: 0.0,
                lifetime,
            });
        }

        self.schedule_tick(cx);
        cx.notify();
    }

    /// 按时间步推进粒子物理。
    fn update_particles(&mut self, dt: f32) {
        let gravity = self.gravity;

        for particle in &mut self.particles {
            particle.age += dt;
            particle.velocity.y += gravity * dt;
            particle.velocity.x *= 0.99;
            particle.position.x += particle.velocity.x * dt;
            particle.position.y += particle.velocity.y * dt;
        }

        self.particles.retain(|p| p.age < p.lifetime);

        if self.particles.is_empty() {
            self.is_active = false;
        }
    }

    /// 获取当前粒子数组。
    pub fn particles(&self) -> &[ConfettiParticle] {
        &self.particles
    }

    /// 定时推进粒子模拟。
    fn schedule_tick(&self, cx: &mut Context<Self>) {
        if !self.is_active {
            return;
        }

        cx.spawn(async |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;

            _ = this.update(cx, |state, cx| {
                if !state.is_active {
                    return;
                }

                let dt = 1.0 / 60.0;
                state.update_particles(dt);

                if state.is_active {
                    state.schedule_tick(cx);
                }

                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for ConfettiState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// 绘制用的粒子快照数据。
struct ConfettiPaintData {
    particles: Vec<ConfettiParticle>,
}

/// 彩带粒子组件。
#[derive(IntoElement)]
pub struct Confetti {
    id: ElementId,
    state: Entity<ConfettiState>,
    style: StyleRefinement,
}

impl Confetti {
    /// 创建彩带组件，绑定粒子系统实体。
    pub fn new(id: impl Into<ElementId>, state: Entity<ConfettiState>) -> Self {
        Self {
            id: id.into(),
            state,
            style: StyleRefinement::default(),
        }
    }

    /// 设置粒子数量。
    pub fn particle_count(self, count: usize, cx: &mut App) -> Self {
        self.state.update(cx, |s, _| s.particle_count = count);
        self
    }

    /// 设置颜色列表。
    pub fn colors(self, colors: Vec<Hsla>, cx: &mut App) -> Self {
        self.state.update(cx, |s, _| s.set_colors(colors));
        self
    }

    /// 设置重力加速度。
    pub fn gravity(self, gravity: f32, cx: &mut App) -> Self {
        self.state.update(cx, |s, _| s.gravity = gravity);
        self
    }
}

impl Styled for Confetti {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Confetti {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let user_style = self.style;
        let state = self.state.read(cx);
        let paint_data = ConfettiPaintData {
            particles: state.particles().to_vec(),
        };

        let mut root = div().id(self.id).relative().size_full().child(
            canvas(
                move |_bounds, _window, _cx| paint_data,
                move |bounds, data, window, _cx| {
                    paint_confetti(bounds, &data, window);
                },
            )
            .absolute()
            .inset_0()
            .size_full(),
        );
        root.style().refine(&user_style);
        root
    }
}

/// 在窗口上绘制所有彩带粒子。
fn paint_confetti(bounds: Bounds<Pixels>, data: &ConfettiPaintData, window: &mut Window) {
    if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
        return;
    }

    let bw = bounds.size.width / px(1.0);
    let bh = bounds.size.height / px(1.0);

    for particle in &data.particles {
        let x = bounds.left() + px(particle.position.x * bw);
        let y = bounds.top() + px(particle.position.y * bh);
        let half = particle.size * 0.5;

        if x + px(half) < bounds.left()
            || x - px(half) > bounds.right()
            || y + px(half) < bounds.top()
            || y - px(half) > bounds.bottom()
        {
            continue;
        }

        let fade = 1.0 - (particle.age / particle.lifetime).clamp(0.0, 1.0);
        let alpha = particle.color.a * fade;

        let wobble = (particle.age * particle.rotation_speed).sin().abs();
        let w = particle.size * (0.5 + wobble * 0.5);
        let h = particle.size;

        window.paint_quad(PaintQuad {
            bounds: Bounds {
                origin: point(x - px(w * 0.5), y - px(h * 0.5)),
                size: size(px(w), px(h)),
            },
            corner_radii: Corners::all(px(1.0)),
            background: hsla(particle.color.h, particle.color.s, particle.color.l, alpha).into(),
            border_widths: Edges::default(),
            border_color: transparent_black(),
            border_style: BorderStyle::default(),
        });
    }
}

/// 简单的伪随机数生成器（0~1）。
fn pseudo_random_f32(seed: u32) -> f32 {
    let mut x = seed.wrapping_add(0x9E3779B9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x45D9F3B);
    x ^= x >> 16;
    (x & 0xFFFF) as f32 / 65535.0
}
