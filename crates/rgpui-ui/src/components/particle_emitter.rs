//! 轻量粒子系统：通过 canvas 绘制持续喷射的粒子流。

use rgpui::*;
use std::time::Duration;

/// 单个粒子。
#[derive(Clone)]
pub struct Particle {
    /// 位置。
    pub position: Point<f32>,
    /// 速度。
    pub velocity: Point<f32>,
    /// 已存活时长。
    pub age: f32,
    /// 寿命。
    pub lifetime: f32,
    /// 粒子尺寸。
    pub size: f32,
    /// 颜色。
    pub color: Hsla,
}

/// 粒子发射器配置。
#[derive(Clone)]
pub struct ParticleEmitterConfig {
    /// 每秒生成粒子数。
    pub spawn_rate: f32,
    /// 粒子寿命。
    pub lifetime: Duration,
    /// 速度范围（最小、最大）。
    pub velocity_range: (f32, f32),
    /// 尺寸范围（最小、最大）。
    pub size_range: (f32, f32),
    /// 起始颜色。
    pub color_start: Hsla,
    /// 结束颜色。
    pub color_end: Hsla,
    /// 重力加速度。
    pub gravity: f32,
    /// 喷射角度扩散范围。
    pub spread_angle: f32,
    /// 最大粒子数。
    pub max_particles: usize,
    /// 喷射原点。
    pub origin: Point<f32>,
}

impl Default for ParticleEmitterConfig {
    fn default() -> Self {
        Self {
            spawn_rate: 10.0,
            lifetime: Duration::from_millis(1500),
            velocity_range: (50.0, 150.0),
            size_range: (2.0, 6.0),
            color_start: hsla(0.55, 0.8, 0.6, 1.0),
            color_end: hsla(0.55, 0.8, 0.6, 0.0),
            gravity: 80.0,
            spread_angle: std::f32::consts::PI,
            max_particles: 200,
            origin: Point { x: 0.0, y: 0.0 },
        }
    }
}

/// 粒子发射器状态。
pub struct ParticleEmitterState {
    particles: Vec<Particle>,
    config: ParticleEmitterConfig,
    accumulator: f32,
    running: bool,
}

impl ParticleEmitterState {
    /// 创建粒子发射器，使用默认配置。
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            particles: Vec::with_capacity(200),
            config: ParticleEmitterConfig::default(),
            accumulator: 0.0,
            running: false,
        }
    }

    /// 以指定配置创建粒子发射器。
    pub fn with_config(config: ParticleEmitterConfig, _cx: &mut Context<Self>) -> Self {
        let cap = config.max_particles;
        Self {
            particles: Vec::with_capacity(cap),
            config,
            accumulator: 0.0,
            running: false,
        }
    }

    /// 更新发射器配置。
    pub fn set_config(&mut self, config: ParticleEmitterConfig, cx: &mut Context<Self>) {
        self.config = config;
        cx.notify();
    }

    /// 设置喷射原点。
    pub fn set_origin(&mut self, origin: Point<f32>, cx: &mut Context<Self>) {
        self.config.origin = origin;
        cx.notify();
    }

    /// 开始发射。
    pub fn start(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        self.running = true;
        self.schedule_tick(cx);
        cx.notify();
    }

    /// 停止发射。
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.running = false;
        cx.notify();
    }

    /// 是否正在发射。
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// 清空所有粒子。
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.particles.clear();
        cx.notify();
    }

    /// 立即生成指定数量的粒子。
    pub fn emit(&mut self, count: usize) {
        let config = &self.config;
        let lifetime_secs = config.lifetime.as_secs_f32();

        for _ in 0..count {
            if self.particles.len() >= config.max_particles {
                break;
            }

            let angle_offset =
                (pseudo_random_f32(self.particles.len() as u32) - 0.5) * config.spread_angle;
            let speed = config.velocity_range.0
                + pseudo_random_f32(self.particles.len() as u32 + 7)
                    * (config.velocity_range.1 - config.velocity_range.0);
            let particle_size = config.size_range.0
                + pseudo_random_f32(self.particles.len() as u32 + 13)
                    * (config.size_range.1 - config.size_range.0);

            let vx = angle_offset.sin() * speed;
            let vy = -angle_offset.cos() * speed;

            self.particles.push(Particle {
                position: config.origin,
                velocity: Point { x: vx, y: vy },
                age: 0.0,
                lifetime: lifetime_secs,
                size: particle_size,
                color: config.color_start,
            });
        }
    }

    /// 按时间步推进粒子物理与颜色渐变。
    pub fn update(&mut self, dt: f32) {
        let gravity = self.config.gravity;
        let color_start = self.config.color_start;
        let color_end = self.config.color_end;

        for particle in &mut self.particles {
            particle.age += dt;
            particle.velocity.y += gravity * dt;
            particle.position.x += particle.velocity.x * dt;
            particle.position.y += particle.velocity.y * dt;

            let t = (particle.age / particle.lifetime).clamp(0.0, 1.0);
            particle.color = lerp_hsla(color_start, color_end, t);
        }

        self.particles.retain(|p| p.age < p.lifetime);
    }

    /// 获取当前粒子数组。
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    /// 定时推进粒子模拟。
    fn schedule_tick(&self, cx: &mut Context<Self>) {
        if !self.running {
            return;
        }

        cx.spawn(async |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;

            _ = this.update(cx, |state, cx| {
                if !state.running {
                    return;
                }

                let dt = 1.0 / 60.0;
                state.accumulator += state.config.spawn_rate * dt;

                let to_spawn = state.accumulator as usize;
                state.accumulator -= to_spawn as f32;
                state.emit(to_spawn);
                state.update(dt);

                state.schedule_tick(cx);
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for ParticleEmitterState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// 绘制用的粒子快照数据。
struct EmitterPaintData {
    particles: Vec<Particle>,
}

/// 粒子发射器组件。
#[derive(IntoElement)]
pub struct ParticleEmitter {
    id: ElementId,
    state: Entity<ParticleEmitterState>,
    style: StyleRefinement,
}

impl ParticleEmitter {
    /// 创建粒子发射器组件，绑定粒子系统实体。
    pub fn new(id: impl Into<ElementId>, state: Entity<ParticleEmitterState>) -> Self {
        Self {
            id: id.into(),
            state,
            style: StyleRefinement::default(),
        }
    }

    /// 设置每秒生成粒子数。
    pub fn spawn_rate(self, rate: f32, cx: &mut App) -> Self {
        self.state.update(cx, |s, _| s.config.spawn_rate = rate);
        self
    }

    /// 设置粒子寿命。
    pub fn lifetime(self, lifetime: Duration, cx: &mut App) -> Self {
        self.state.update(cx, |s, _| s.config.lifetime = lifetime);
        self
    }

    /// 设置速度范围。
    pub fn velocity_range(self, range: (f32, f32), cx: &mut App) -> Self {
        self.state
            .update(cx, |s, _| s.config.velocity_range = range);
        self
    }

    /// 设置尺寸范围。
    pub fn size_range(self, range: (f32, f32), cx: &mut App) -> Self {
        self.state.update(cx, |s, _| s.config.size_range = range);
        self
    }

    /// 设置起始颜色。
    pub fn color_start(self, color: Hsla, cx: &mut App) -> Self {
        self.state.update(cx, |s, _| s.config.color_start = color);
        self
    }

    /// 设置结束颜色。
    pub fn color_end(self, color: Hsla, cx: &mut App) -> Self {
        self.state.update(cx, |s, _| s.config.color_end = color);
        self
    }

    /// 设置重力加速度。
    pub fn gravity(self, gravity: f32, cx: &mut App) -> Self {
        self.state.update(cx, |s, _| s.config.gravity = gravity);
        self
    }

    /// 设置喷射角度扩散范围。
    pub fn spread_angle(self, angle: f32, cx: &mut App) -> Self {
        self.state.update(cx, |s, _| s.config.spread_angle = angle);
        self
    }

    /// 设置最大粒子数。
    pub fn max_particles(self, max: usize, cx: &mut App) -> Self {
        self.state.update(cx, |s, _| s.config.max_particles = max);
        self
    }

    /// 设置喷射原点。
    pub fn origin(self, origin: Point<f32>, cx: &mut App) -> Self {
        self.state.update(cx, |s, _| s.config.origin = origin);
        self
    }
}

impl Styled for ParticleEmitter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ParticleEmitter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let user_style = self.style;
        let state = self.state.read(cx);
        let paint_data = EmitterPaintData {
            particles: state.particles().to_vec(),
        };

        let mut root = div().id(self.id).relative().size_full().child(
            canvas(
                move |_bounds, _window, _cx| paint_data,
                move |bounds, data, window, _cx| {
                    paint_particles(bounds, &data, window);
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

/// 在窗口上绘制所有粒子。
fn paint_particles(bounds: Bounds<Pixels>, data: &EmitterPaintData, window: &mut Window) {
    if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
        return;
    }

    for particle in &data.particles {
        let x = bounds.left() + px(particle.position.x);
        let y = bounds.top() + px(particle.position.y);
        let half = particle.size * 0.5;

        if x + px(half) < bounds.left()
            || x - px(half) > bounds.right()
            || y + px(half) < bounds.top()
            || y - px(half) > bounds.bottom()
        {
            continue;
        }

        window.paint_quad(PaintQuad {
            bounds: Bounds {
                origin: point(x - px(half), y - px(half)),
                size: size(px(particle.size), px(particle.size)),
            },
            corner_radii: Corners::all(px(half)),
            background: particle.color.into(),
            border_widths: Edges::default(),
            border_color: transparent_black(),
            border_style: BorderStyle::default(),
        });
    }
}

/// 在 HSLA 颜色之间线性插值。
fn lerp_hsla(a: Hsla, b: Hsla, t: f32) -> Hsla {
    hsla(
        a.h + (b.h - a.h) * t,
        a.s + (b.s - a.s) * t,
        a.l + (b.l - a.l) * t,
        a.a + (b.a - a.a) * t,
    )
}

/// 简单的伪随机数生成器（0~1）。
fn pseudo_random_f32(seed: u32) -> f32 {
    let mut x = seed.wrapping_add(0x9E3779B9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x45D9F3B);
    x ^= x >> 16;
    (x & 0xFFFF) as f32 / 65535.0
}
