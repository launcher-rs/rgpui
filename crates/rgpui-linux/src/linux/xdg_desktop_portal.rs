//! 从 [XDG 桌面门户] 事件提供 [calloop] 事件源
//!
//! 本模块使用 [ashpd] crate

use ashpd::desktop::settings::{ColorScheme, Settings};
use calloop::channel::Channel;
use calloop::{EventSource, Poll, PostAction, Readiness, Token, TokenFactory};
use smol::stream::StreamExt;

use rgpui::{BackgroundExecutor, WindowAppearance};

/// XDG 桌面门户事件类型
pub enum Event {
    /// 窗口外观变化
    WindowAppearance(WindowAppearance),
    /// 光标主题变化
    CursorTheme(String),
    /// 光标大小变化
    CursorSize(u32),
    /// 窗口按钮布局变化
    ButtonLayout(String),
    /// 系统「减少动态效果」无障碍设置变化
    ReduceMotion(bool),
}

/// XDG 桌面门户事件源
pub struct XDPEventSource {
    channel: Channel<Event>,
}

impl XDPEventSource {
    /// 创建新的 XDP 事件源
    ///
    /// 订阅桌面门户设置变化，包括颜色方案、光标主题、光标大小、按钮布局和减少动态效果
    ///
    /// # 参数
    ///
    /// * `executor` - 后台执行器
    pub fn new(executor: &BackgroundExecutor) -> Self {
        let (sender, channel) = calloop::channel::channel();

        let background = executor.clone();

        executor
            .spawn(async move {
                let settings = Settings::new().await?;

                if let Ok(initial_appearance) = settings.color_scheme().await {
                    sender.send(Event::WindowAppearance(
                        window_appearance_from_color_scheme(initial_appearance),
                    ))?;
                }
                if let Ok(initial_theme) = settings
                    .read::<String>("org.gnome.desktop.interface", "cursor-theme")
                    .await
                {
                    sender.send(Event::CursorTheme(initial_theme))?;
                }

                // 如果这里使用 u32，会抛出无效类型错误
                if let Ok(initial_size) = settings
                    .read::<i32>("org.gnome.desktop.interface", "cursor-size")
                    .await
                {
                    sender.send(Event::CursorSize(initial_size as u32))?;
                }

                if let Ok(initial_layout) = settings
                    .read::<String>("org.gnome.desktop.wm.preferences", "button-layout")
                    .await
                {
                    sender.send(Event::ButtonLayout(initial_layout))?;
                }

                if let Some(reduce_motion) = read_reduce_motion(&settings).await {
                    sender.send(Event::ReduceMotion(reduce_motion))?;
                }

                if let Ok(mut cursor_theme_changed) = settings
                    .receive_setting_changed_with_args(
                        "org.gnome.desktop.interface",
                        "cursor-theme",
                    )
                    .await
                {
                    let sender = sender.clone();
                    background
                        .spawn(async move {
                            while let Some(theme) = cursor_theme_changed.next().await {
                                let theme = theme?;
                                sender.send(Event::CursorTheme(theme))?;
                            }
                            anyhow::Ok(())
                        })
                        .detach();
                }

                if let Ok(mut cursor_size_changed) = settings
                    .receive_setting_changed_with_args::<i32>(
                        "org.gnome.desktop.interface",
                        "cursor-size",
                    )
                    .await
                {
                    let sender = sender.clone();
                    background
                        .spawn(async move {
                            while let Some(size) = cursor_size_changed.next().await {
                                let size = size?;
                                sender.send(Event::CursorSize(size as u32))?;
                            }
                            anyhow::Ok(())
                        })
                        .detach();
                }

                if let Ok(mut button_layout_changed) = settings
                    .receive_setting_changed_with_args(
                        "org.gnome.desktop.wm.preferences",
                        "button-layout",
                    )
                    .await
                {
                    let sender = sender.clone();
                    background
                        .spawn(async move {
                            while let Some(layout) = button_layout_changed.next().await {
                                let layout = layout?;
                                sender.send(Event::ButtonLayout(layout))?;
                            }
                            anyhow::Ok(())
                        })
                        .detach();
                }

                if let Ok(mut animations_changed) = settings
                    .receive_setting_changed_with_args::<bool>(
                        "org.freedesktop.appearance",
                        "animations-disabled",
                    )
                    .await
                {
                    let sender = sender.clone();
                    background
                        .spawn(async move {
                            while let Some(disabled) = animations_changed.next().await {
                                sender.send(Event::ReduceMotion(disabled?))?;
                            }
                            anyhow::Ok(())
                        })
                        .detach();
                }

                let mut appearance_changed = settings.receive_color_scheme_changed().await?;
                while let Some(scheme) = appearance_changed.next().await {
                    sender.send(Event::WindowAppearance(
                        window_appearance_from_color_scheme(scheme),
                    ))?;
                }

                anyhow::Ok(())
            })
            .detach();

        Self { channel }
    }
}

impl EventSource for XDPEventSource {
    type Event = Event;
    type Metadata = ();
    type Ret = ();
    type Error = anyhow::Error;

    fn process_events<F>(
        &mut self,
        readiness: Readiness,
        token: Token,
        mut callback: F,
    ) -> Result<PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        self.channel.process_events(readiness, token, |evt, _| {
            if let calloop::channel::Event::Msg(msg) = evt {
                (callback)(msg, &mut ())
            }
        })?;

        Ok(PostAction::Continue)
    }

    fn register(
        &mut self,
        poll: &mut Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        self.channel.register(poll, token_factory)?;

        Ok(())
    }

    fn reregister(
        &mut self,
        poll: &mut Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        self.channel.reregister(poll, token_factory)?;

        Ok(())
    }

    fn unregister(&mut self, poll: &mut Poll) -> calloop::Result<()> {
        self.channel.unregister(poll)?;

        Ok(())
    }
}

fn window_appearance_from_color_scheme(cs: ColorScheme) -> WindowAppearance {
    match cs {
        ColorScheme::PreferDark => WindowAppearance::Dark,
        ColorScheme::PreferLight => WindowAppearance::Light,
        ColorScheme::NoPreference => WindowAppearance::Light,
    }
}

/// 读取「减少动态效果」设置。
///
/// 优先取门户标准键 `org.freedesktop.appearance` 的 `animations-disabled`；
/// 桌面环境没实现该键时回退 GNOME 的 `enable-animations`（语义相反）。
/// 两者都读不到时返回 `None`，由调用方保持平台侧默认值（动画开启）。
async fn read_reduce_motion(settings: &Settings) -> Option<bool> {
    if let Ok(disabled) = settings
        .read::<bool>("org.freedesktop.appearance", "animations-disabled")
        .await
    {
        return Some(disabled);
    }

    settings
        .read::<bool>("org.gnome.desktop.interface", "enable-animations")
        .await
        .ok()
        .map(|animations| !animations)
}
