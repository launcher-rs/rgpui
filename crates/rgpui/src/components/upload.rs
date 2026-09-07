//! 文件上传组件。
//!
//! 选择区（点击打开平台文件对话框）+ 文件列表（含进度条与移除按钮）。
//! 对话框走平台已有的 [`App::prompt_for_paths`]（桌面三端原生实现），
//! Web 端平台明确不支持，`pick` 调用无效果，调用方需自行降级。

use super::AnimatedProgress;
use crate::{prelude::FluentBuilder as _, *};
use std::path::PathBuf;
use std::rc::Rc;

/// 单个待上传文件。
#[derive(Clone)]
pub struct UploadFile {
    /// 文件路径。
    pub path: PathBuf,
    /// 显示名（默认取文件名）。
    pub name: SharedString,
    /// 字节数（读不到时为 0）。
    pub size: u64,
    /// 上传进度 0.0–1.0（选择后默认 0，由调用方经 `set_progress` 推进）。
    pub progress: f32,
}

impl UploadFile {
    /// 由路径构造（自动取文件名与大小，失败时大小为 0）。
    pub fn from_path(path: PathBuf) -> Self {
        let name: SharedString = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned().into())
            .unwrap_or_default();
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        Self {
            path,
            name,
            size,
            progress: 0.0,
        }
    }

    /// 人类可读的大小（B/KB/MB）。
    pub fn size_text(&self) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        if self.size >= MB {
            format!("{:.1} MB", self.size as f64 / MB as f64)
        } else if self.size >= KB {
            format!("{:.1} KB", self.size as f64 / KB as f64)
        } else {
            format!("{} B", self.size)
        }
    }
}

/// 文件上传状态实体（`Render`，父组件 `cx.new` 持有）。
pub struct UploadState {
    /// 已选文件。
    files: Vec<UploadFile>,
    /// 是否多选（透传给对话框）。
    multiple: bool,
    /// 是否允许选择目录（透传给对话框）。
    directories: bool,
    /// 对话框提示文本。
    prompt: Option<SharedString>,
    /// 选择回调（订阅回调无 Window，延后到 render 里触发）。
    on_select: Option<Rc<dyn Fn(&[PathBuf], &mut Window, &mut App)>>,
    /// 待触发的选择（render 里消费）。
    pending_select: bool,
    /// 用户样式。
    style: StyleRefinement,
}

impl UploadState {
    /// 创建空上传状态（默认多选文件）。
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            files: Vec::new(),
            multiple: true,
            directories: false,
            prompt: None,
            on_select: None,
            pending_select: false,
            style: StyleRefinement::default(),
        }
    }

    /// 设置是否多选。
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    /// 设置是否允许选择目录。
    pub fn directories(mut self, directories: bool) -> Self {
        self.directories = directories;
        self
    }

    /// 设置对话框提示文本。
    pub fn prompt(mut self, prompt: impl Into<SharedString>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// 设置选择回调（用户选完文件后触发，参数为本次选中的路径）。
    pub fn on_select<F>(mut self, handler: F) -> Self
    where
        F: Fn(&[PathBuf], &mut Window, &mut App) + 'static,
    {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// 已选文件（只读）。
    pub fn files(&self) -> &[UploadFile] {
        &self.files
    }

    /// 打开平台文件对话框选择文件（Web 端平台不支持，调用无效果）。
    pub fn pick(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: self.directories,
            multiple: self.multiple,
            prompt: self.prompt.clone(),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                _ = this.update(cx, |state, cx| {
                    state.push_paths(paths);
                    state.pending_select = true;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// 直接加入一批路径（拖拽到应用等场景由调用方处理拖放后调用）。
    pub fn add_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        self.push_paths(paths);
        cx.notify();
    }

    /// 加入路径（不通知，由调用方负责 `notify`）。
    fn push_paths(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            if !self.files.iter().any(|f| f.path == path) {
                self.files.push(UploadFile::from_path(path));
            }
        }
    }

    /// 移除第 `index` 个文件。
    pub fn remove_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.files.len() {
            self.files.remove(index);
            cx.notify();
        }
    }

    /// 更新第 `index` 个文件的上传进度（0.0–1.0）。
    pub fn set_progress(&mut self, index: usize, progress: f32, cx: &mut Context<Self>) {
        if let Some(file) = self.files.get_mut(index) {
            file.progress = progress.clamp(0.0, 1.0);
            cx.notify();
        }
    }

    /// 清空文件列表。
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.files.clear();
        cx.notify();
    }
}

impl Styled for UploadState {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Render for UploadState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 延后触发选择回调（订阅上下文无 Window）。
        if self.pending_select {
            self.pending_select = false;
            if let Some(ref cb) = self.on_select {
                let paths: Vec<PathBuf> = self.files.iter().map(|f| f.path.clone()).collect();
                // render 中借用规则：先取回调与数据再调用，避免重入借用。
                let cb = cb.clone();
                cb(&paths, window, cx);
            }
        }

        let theme = cx.theme();
        let border = theme.tokens.border;
        let muted_foreground = theme.tokens.muted_foreground;
        let accent = theme.tokens.accent.color;
        let user_style = self.style.clone();

        let mut root =
            div().flex().flex_col().gap(px(8.0)).child(
                div()
                    .id("upload-pick")
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .py(px(20.0))
                    .rounded_md()
                    .border(px(1.0))
                    .border_color(border)
                    .cursor_pointer()
                    .hover(|this| this.bg(accent.opacity(0.08)))
                    .child(Icon::new(IconName::File).text_color(muted_foreground))
                    .child(div().text_sm().text_color(muted_foreground.color).child(
                        if self.multiple {
                            "点击选择文件（可多选）"
                        } else {
                            "点击选择文件"
                        },
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.pick(cx);
                    })),
            );

        for (ix, file) in self.files.iter().enumerate() {
            let name = file.name.clone();
            let size_text = file.size_text();
            let progress = file.progress;
            root = root.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded_md()
                    .bg(accent.opacity(0.05))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(div().flex_1().text_sm().child(name))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted_foreground.color)
                                    .child(size_text),
                            )
                            .child(
                                Button::new(ElementId::named_usize("upload-remove", ix))
                                    .ghost()
                                    .small()
                                    .icon(IconName::Close)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.remove_file(ix, cx);
                                    })),
                            ),
                    )
                    .child(
                        AnimatedProgress::new(ElementId::named_usize("upload-progress", ix))
                            .value(progress),
                    ),
            );
        }

        root.map(|mut this| {
            this.style().refine(&user_style);
            this
        })
    }
}
