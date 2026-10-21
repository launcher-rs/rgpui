//! 动画时间线示例
//!
//! 演示 `anim_timeline`：多轨关键帧时间线（标尺刻度 + 值曲线 + 关键帧方块 + 播放头）。
//! 拖拽标尺移动播放头，拖拽关键帧改时间、上下拖拽改值，双击轨道空白处新增关键帧
//! （值取当前插值），`Delete` 删除选中关键帧，`Ctrl/Cmd + 滚轮` 缩放、
//! `Shift + 滚轮` 纵向滚动轨道、滚轮平移时间轴，点轨道名前的箭头折叠 / 展开轨道。
//! 三轨的值实时驱动右侧预览方块（位置 / 缩放 / 透明度），播放头播放时按 `1/60s` 步进推进。
//!
//! 运行：`cargo run -p showcase --bin timeline`

use std::time::Duration;

use rgpui::anim_timeline::{
    Interpolation, Keyframe, Timeline, TimelineEvent, TimelineState, TimelineView, Track,
};
use rgpui::{
    AnyElement, App, Bounds, Button, Context, Entity, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, hsla,
    prelude::*, px, size, v_flex,
};
use rgpui_platform::application;

/// 播放时每帧推进的秒数。
const FRAME_STEP: f32 = 1.0 / 60.0;

/// 构造演示用时间线：三条轨道。
fn demo_timeline() -> Timeline {
    Timeline::from_tracks(vec![
        Track::with_keyframes(
            "位置 X",
            vec![Keyframe::new(0.0, 0.0), Keyframe::new(2.0, 240.0)],
        ),
        Track::with_keyframes(
            "缩放",
            vec![Keyframe::new(0.0, 0.6), Keyframe::new(2.0, 1.4)],
        )
        .with_interpolation(Interpolation::Smooth),
        Track::with_keyframes(
            "透明度",
            vec![
                Keyframe::new(0.0, 0.2),
                Keyframe::new(1.0, 1.0),
                Keyframe::new(2.0, 0.6),
            ],
        ),
        // 后三条只用来把轨道区撑到需要纵向滚动，预览方块不读取。
        Track::with_keyframes(
            "旋转",
            vec![Keyframe::new(0.0, 0.0), Keyframe::new(2.0, 90.0)],
        ),
        Track::with_keyframes(
            "模糊",
            vec![Keyframe::new(0.0, 0.0), Keyframe::new(2.0, 8.0)],
        ),
        Track::new("颜色"),
    ])
}

/// 时间线示例应用。
struct TimelineApp {
    /// 时间线状态。
    state: Entity<TimelineState>,
    /// 是否播放中（与时间线状态同步）。
    playing: bool,
    /// 状态栏文案。
    status: SharedString,
}

impl TimelineApp {
    /// 创建示例应用并订阅时间线事件。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| TimelineState::new(demo_timeline(), cx));

        cx.subscribe_in(
            &state,
            window,
            |this: &mut Self, state, event: &TimelineEvent, _window, cx| match event {
                TimelineEvent::PlaybackChanged(playing) => {
                    this.playing = *playing;
                    this.status = if *playing {
                        "播放中…".into()
                    } else {
                        "已暂停".into()
                    };
                }
                TimelineEvent::KeyframeRequested { track, time } => {
                    state.update(cx, |state, cx| {
                        if let Some(model) = state.timeline_mut().track_mut(*track) {
                            let value = model.value_at(*time);
                            model.set_keyframe(*time, value);
                        }
                        cx.notify();
                    });
                    this.status = format!("在第 {:.2} 秒新增关键帧", time).into();
                }
                TimelineEvent::KeyframeMoved {
                    track, time, value, ..
                } => {
                    this.status = format!(
                        "轨道 {} 的关键帧 → {:.2} 秒 / 值 {:.2}",
                        track + 1,
                        time,
                        value
                    )
                    .into();
                }
                TimelineEvent::KeyframeRemoved { track, .. } => {
                    this.status = format!("删除轨道 {} 的一个关键帧", track + 1).into();
                }
                TimelineEvent::TimeChanged(time) => {
                    this.status = format!("播放头 {:.2} 秒", time).into();
                }
                TimelineEvent::SelectionChanged { .. } => {}
                TimelineEvent::TrackMoved { from, to } => {
                    this.status = format!("轨道 {} 移到第 {} 位", from + 1, to + 1).into();
                }
                TimelineEvent::TrackCollapsed { index, collapsed } => {
                    this.status = if *collapsed {
                        format!("轨道 {} 已折叠", index + 1)
                    } else {
                        format!("轨道 {} 已展开", index + 1)
                    }
                    .into();
                }
                TimelineEvent::TrackInterpolation {
                    index,
                    interpolation,
                } => {
                    this.status =
                        format!("轨道 {} 的插值改为「{}」", index + 1, interpolation.label())
                            .into();
                }
            },
        )
        .detach();

        Self {
            state,
            playing: false,
            status: "拖拽标尺移动播放头 · 拖拽关键帧改时间 / 改值 · 双击轨道空白新增".into(),
        }
    }

    /// 开始播放（后台逐帧推进播放头）。
    fn play(&mut self, cx: &mut Context<Self>) {
        if self.playing {
            return;
        }
        self.playing = true;
        self.state.update(cx, |state, cx| state.play(cx));

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let continues = this.update(cx, |this, cx| {
                    if !this.playing {
                        return false;
                    }
                    this.state
                        .update(cx, |state, cx| state.advance(FRAME_STEP, cx));
                    true
                });
                if !matches!(continues, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    /// 暂停播放。
    fn pause(&mut self, cx: &mut Context<Self>) {
        self.playing = false;
        self.state.update(cx, |state, cx| state.pause(cx));
    }

    /// 回到开头。
    fn rewind(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.set_time(0.0, cx));
        self.status = "回到开头".into();
    }

    /// 在播放头处给第一条轨道新增关键帧。
    fn add_keyframe(&mut self, cx: &mut Context<Self>) {
        let (time, index) = self.state.update(cx, |state, cx| {
            let time = state.time();
            let Some(model) = state.timeline_mut().track_mut(0) else {
                return (0.0, None);
            };
            let value = model.value_at(time);
            model.set_keyframe(time, value);
            let index = model.keyframe_index_at(time, 1e-3);
            cx.notify();
            (time, index)
        });
        if let Some(index) = index {
            self.state
                .update(cx, |state, cx| state.select_keyframe(0, index, cx));
        }
        self.status = format!("在 {time:.2} 秒新增关键帧").into();
    }

    /// 删除选中的关键帧。
    fn remove_keyframe(&mut self, cx: &mut Context<Self>) {
        let selected = self.state.read(cx).selection().1.is_some();
        self.state
            .update(cx, |state, cx| state.remove_selected_keyframe(cx));
        self.status = if selected {
            "删除选中的关键帧".into()
        } else {
            "没有选中关键帧".into()
        };
    }

    /// 时间线总时长。
    fn duration(&self, cx: &Context<Self>) -> f32 {
        self.state.read(cx).duration()
    }

    /// 上移 / 下移选中的轨道（未选中时移动第一条）。
    fn move_track(&mut self, delta: isize, cx: &mut Context<Self>) {
        let (from, count) = {
            let state = self.state.read(cx);
            (state.selection().0.unwrap_or(0), state.timeline().len())
        };
        let target = from as isize + delta;
        if target < 0 || target >= count as isize {
            self.status = "轨道已在边界".into();
            return;
        }
        self.state
            .update(cx, |state, cx| state.move_track(from, target as usize, cx));
    }

    /// 折叠 / 展开选中的轨道（未选中时处理第一条）。
    fn toggle_collapse(&mut self, cx: &mut Context<Self>) {
        let index = self.state.read(cx).selection().0.unwrap_or(0);
        self.state
            .update(cx, |state, cx| state.toggle_track_collapsed(index, cx));
    }

    /// 播放头处三条轨道的值（位置 / 缩放 / 透明度）。
    fn values(&self, cx: &Context<Self>) -> (f32, f32, f32) {
        let state = self.state.read(cx);
        let values = state.timeline().values_at(state.time());
        (
            values.first().copied().unwrap_or(0.0),
            values.get(1).copied().unwrap_or(1.0),
            values.get(2).copied().unwrap_or(1.0),
        )
    }
}

/// 插值选择按钮组：作用于选中轨道（未选中时按钮禁用）。
fn interpolation_buttons(
    cx: &mut Context<TimelineApp>,
    state: Entity<TimelineState>,
) -> Vec<AnyElement> {
    let selected = state.read(cx).selection().0;
    let mut out = Vec::new();
    for (index, interpolation) in [
        Interpolation::Linear,
        Interpolation::Smooth,
        Interpolation::EaseIn,
        Interpolation::EaseOut,
        Interpolation::EaseInOut,
        Interpolation::Hold,
    ]
    .into_iter()
    .enumerate()
    {
        let state = state.clone();
        let current = selected.and_then(|track| {
            state
                .read(cx)
                .timeline()
                .track(track)
                .map(|track| track.interpolation)
        });
        let active = current == Some(interpolation);
        let button = Button::new(format!("interp-{index}"))
            .label(format!("插值：{}", interpolation.label()))
            .on_click(move |_event, _window, cx| {
                state.update(cx, |state, cx| {
                    if let Some(track) = state.selection().0 {
                        state.set_track_interpolation(track, interpolation, cx);
                    }
                });
            });
        out.push(
            div()
                .child(button)
                .when(active, |this| this.text_color(cx.theme().primary))
                .into_any_element(),
        );
    }
    out
}
impl Render for TimelineApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (x, scale, opacity) = self.values(cx);
        let duration = self.duration(cx);
        let time = self.state.read(cx).time();
        let snapping = self.state.read(cx).is_snapping();
        let selected = self.state.read(cx).selection().1.is_some();

        let play_button = if self.playing {
            Button::new("pause")
                .label("暂停")
                .on_click(cx.listener(|this, _event, _window, cx| this.pause(cx)))
        } else {
            Button::new("play")
                .label("播放")
                .on_click(cx.listener(|this, _event, _window, cx| this.play(cx)))
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .p_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(play_button)
                    .child(
                        Button::new("rewind")
                            .label("回到开头")
                            .on_click(cx.listener(|this, _event, _window, cx| this.rewind(cx))),
                    )
                    .child(
                        Button::new("add-keyframe").label("加关键帧").on_click(
                            cx.listener(|this, _event, _window, cx| this.add_keyframe(cx)),
                        ),
                    )
                    .child(Button::new("remove-keyframe").label("删关键帧").on_click(
                        cx.listener(|this, _event, _window, cx| this.remove_keyframe(cx)),
                    ))
                    .child(Button::new("zoom-in").label("放大").on_click(cx.listener(
                        |this, _event, _window, cx| {
                            let state = this.state.clone();
                            state.update(cx, |state, cx| state.zoom_by(1.25, cx));
                        },
                    )))
                    .child(Button::new("zoom-out").label("缩小").on_click(cx.listener(
                        |this, _event, _window, cx| {
                            let state = this.state.clone();
                            state.update(cx, |state, cx| state.zoom_by(0.8, cx));
                        },
                    )))
                    .child(
                        Button::new("track-up").label("轨道上移").on_click(
                            cx.listener(|this, _event, _window, cx| this.move_track(-1, cx)),
                        ),
                    )
                    .child(
                        Button::new("track-down").label("轨道下移").on_click(
                            cx.listener(|this, _event, _window, cx| this.move_track(1, cx)),
                        ),
                    )
                    .child(Button::new("track-fold").label("折叠轨道").on_click(
                        cx.listener(|this, _event, _window, cx| this.toggle_collapse(cx)),
                    ))
                    .children(interpolation_buttons(cx, self.state.clone()))
                    .child(
                        Button::new("toggle-snap")
                            .label(if snapping {
                                "吸附：开"
                            } else {
                                "吸附：关"
                            })
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                let state = this.state.clone();
                                let next = if state.read(cx).is_snapping() {
                                    0.0
                                } else {
                                    30.0
                                };
                                state.update(cx, |state, cx| state.set_fps(next, cx));
                            })),
                    )
                    .child(div().text_sm().child(self.status.clone())),
            )
            .child(
                div()
                    .h(px(140.0))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(TimelineView::new(&self.state)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .p_4()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .child(format!("播放头 {time:.2}s / 总时长 {duration:.2}s")),
                    )
                    .child(div().text_xs().child(format!(
                        "位置 X {x:.1}px · 缩放 {scale:.2}× · 透明度 {opacity:.2}"
                    )))
                    .child(
                        div()
                            .relative()
                            .h(px(160.0))
                            .w_full()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .overflow_hidden()
                            // 预览方块：位置 / 缩放 / 透明度由时间线驱动。
                            .child(
                                div()
                                    .absolute()
                                    .top(px(40.0))
                                    .left(px(16.0 + x))
                                    .w(px(120.0 * scale))
                                    .h(px(80.0 * scale))
                                    .rounded_md()
                                    .bg(hsla(0.58, 0.6, 0.5, 1.0))
                                    .opacity(opacity),
                            ),
                    )
                    .child(div().text_xs().child(if selected {
                        "已选中关键帧：Delete 删除，水平拖拽改时间、上下拖拽改值"
                    } else {
                        "未选中关键帧"
                    })),
            )
    }
}

fn main() {
    rgpui::init_logging();
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1000.), px(620.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Animation Timeline Example".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| TimelineApp::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
