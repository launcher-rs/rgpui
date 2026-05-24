//! rgpui-character 桌面宠物完整示例。
//!
//! 这个示例展示一条完整链路：
//! - `AssetManager` 注册 sprite 动画资源
//! - `Behavior` 决定角色当前动作
//! - `CharacterRuntime` 推进行为、物理和动画
//! - `RenderCommand` 输出当前要绘制的纹理和变换
//! - rgpui 使用 `img()` 将纹理路径绘制到透明覆盖窗口中
//!
//! 运行：
//!
//! ```text
//! cargo run -p rgpui-character --example desktop_pet
//! ```

// 在 wasm 目标上禁用 main 函数入口
#![cfg_attr(target_family = "wasm", no_main)]

// 导入单例模块：防止重复启动
use rgpui::single_instance::{SingleInstance, send_activate_to_existing};
// 从 rgpui 核心库导入 UI 框架所需的基础类型
use rgpui::{
    App,                        // 应用程序上下文
    AssetSource,                // 资源加载器 trait
    Bounds,                     // 窗口边界
    Context,                    // 视图上下文
    Keystroke,                  // 按键描述（用于注册快捷键）
    Pixels,                     // 像素单位类型
    Point,                      // 点坐标
    Render,                     // 渲染 trait（视图必须实现）
    SharedString,               // 共享字符串（跨线程安全）
    TrayIconEvent,              // 托盘图标事件
    TrayMenuItem,               // 托盘菜单项
    Window,                     // 窗口引用
    WindowBackgroundAppearance, // 窗口背景外观
    WindowBounds,               // 窗口边界状态
    WindowKind,                 // 窗口类型（普通、弹窗、覆盖）
    WindowOptions,              // 窗口创建选项
    div,                        // div 布局元素
    img,                        // 图片元素
    point,                      // 创建 Point 的宏
    prelude::*,                 // 导入常用 trait 和类型
    px,                         // 创建 Pixels 的宏
    rgb,                        // 创建 RGB 颜色的宏
    rgba,                       // 创建 RGBA 颜色的宏
    size,                       // 创建 Size 的宏
};
// 从 rgpui-character 导入角色运行时和动画系统
use rgpui_character::{
    AnimationClip,    // 动画片段（包含多个帧纹理）
    Behavior,         // 行为 trait（决定角色动作）
    BehaviorAction,   // 行为动作（移动、播放动画、待机）
    BehaviorContext,  // 行为上下文（包含位置、速度、增量时间）
    Character,        // 角色结构体
    CharacterRuntime, // 角色运行时（推进物理、动画和行为）
    PhysicsConfig,    // 物理配置（摩擦力、边界、弹性）
    Rect,             // 矩形（用于物理边界）
    RenderCommand,    // 渲染命令（绘制精灵）
    TextureId,        // 纹理标识符
    Vec2,             // 二维向量
};
// 导入平台选择入口，根据编译目标选择对应平台实现
use rgpui_platform::application;
// 导入 Cow 用于借用或拥有数据
use std::borrow::Cow;
// 导入文件系统操作
use std::fs;
// 导入路径类型
use std::path::PathBuf;
// 导入 Arc（原子引用计数）和 Mutex（互斥锁），用于跨线程共享状态
use std::sync::{Arc, Mutex};
// 导入 Duration（持续时间）
use std::time::Duration;

/// 应用标识符，用于单例检测。
/// 当第二个实例启动时，检测到相同标识符则退出并向第一个实例发送信号。
const APP_ID: &str = "com.example.rgpui-character-desktop-pet";

/// 桌宠覆盖窗口尺寸。
/// 窗口为正方形，宽高均为 320 像素。
const WINDOW_SIZE: f32 = 320.0;

/// sprite（精灵）在窗口中的显示尺寸。
/// 精灵按此尺寸缩放显示在窗口中心。
const SPRITE_SIZE: f32 = 220.0;

/// 自动更新帧间隔，约等于 60 FPS。
/// 每 16 毫秒执行一次更新循环。
const FRAME_TIME: Duration = Duration::from_millis(16);

/// 暂停或恢复桌宠的全局快捷键编号。
/// 用于在注册快捷键时标识此快捷键。
const HOTKEY_TOGGLE_PAUSE: u32 = 1;

/// 示例资源加载器。
/// 从文件系统加载动画帧图片资源。
struct ExampleAssets {
    /// 资源根目录。
    /// 所有资源路径相对于此目录。
    base: PathBuf,
}

/// 实现 AssetSource trait，为 rgpui 提供资源加载能力。
impl AssetSource for ExampleAssets {
    /// 加载指定路径的资源文件。
    ///
    /// # 参数
    /// * `path` - 资源文件路径（相对于 base 目录）
    ///
    /// # 返回
    /// 返回加载的字节数据，文件不存在时返回 Ok(None)。
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        // 拼接完整路径并读取文件
        fs::read(self.base.join(path))
            // 成功时包装为 Cow::Owned
            .map(|data| Some(Cow::Owned(data)))
            // 转换错误类型
            .map_err(Into::into)
    }

    /// 列出指定目录下的所有资源文件。
    ///
    /// # 参数
    /// * `path` - 目录路径
    ///
    /// # 返回
    /// 返回目录中的文件名列表。
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        // 读取目录条目
        fs::read_dir(self.base.join(path))
            .map(|entries| {
                // 过滤并转换目录条目
                entries
                    .filter_map(|entry| {
                        entry
                            // 忽略 IO 错误
                            .ok()
                            // 获取文件名并转为字符串
                            .and_then(|entry| entry.file_name().into_string().ok())
                            // 转换为 SharedString
                            .map(SharedString::from)
                    })
                    // 收集到 Vec
                    .collect()
            })
            // 转换错误类型
            .map_err(Into::into)
    }
}

/// 小猫动作类型。
/// 定义了桌宠可能执行的全部动作。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CatActivity {
    /// 散步：猫在屏幕上随机走动
    Walk,
    /// 睡觉：猫原地休息
    Sleep,
    /// 吃东西：猫做进食动画
    Eat,
    /// 转圈圈：猫旋转动画
    Spin,
    /// 打哈欠：猫打哈欠动画
    Yawn,
    /// 转头：猫改变朝向（碰到边界时触发）
    Turn,
}

/// CatActivity 的方法实现。
impl CatActivity {
    /// 获取动作对应的动画名称（用于查找动画资源）。
    ///
    /// # 返回
    /// 返回动画名称字符串切片，如 "walk"、"sleep" 等。
    fn animation_name(self) -> &'static str {
        // 根据动作类型返回动画文件名前缀
        match self {
            Self::Walk => "walk",
            Self::Sleep => "sleep",
            Self::Eat => "eat",
            Self::Spin => "spin",
            Self::Yawn => "yawn",
            Self::Turn => "turn",
        }
    }

    /// 获取动作的中文显示标签。
    ///
    /// # 返回
    /// 返回动作的中文名称，用于控制面板按钮显示。
    fn label(self) -> &'static str {
        // 根据动作类型返回中文标签
        match self {
            Self::Walk => "散步",
            Self::Sleep => "睡觉",
            Self::Eat => "吃东西",
            Self::Spin => "转圈圈",
            Self::Yawn => "打哈欠",
            Self::Turn => "转头",
        }
    }

    /// 判断此动作是否移动角色位置。
    ///
    /// # 返回
    /// 只有 Walk（散步）会移动位置，其余动作均为原地行为。
    fn moves(self) -> bool {
        // 只有散步会移动
        matches!(self, Self::Walk)
    }
}

/// 跨线程共享的桌宠运行状态。
/// 通过 Arc<Mutex<>> 在 UI 主循环和按钮事件处理器之间共享。
struct PetSharedState {
    /// 角色运行时，负责推进物理、动画和行为决策
    runtime: CharacterRuntime,
    /// 是否处于自动运行模式（与 interactive 互斥）
    /// true = 猫自动散步/进食等；false = 交互模式或暂停
    running: bool,
    /// 渲染层鼠标穿透标志
    /// true = 不显示控制面板（穿透）；false = 显示控制面板（可点击）
    passthrough: bool,
    /// 是否处于交互模式（与 running 互斥）
    /// true = 控制面板可见，可点击按钮
    interactive: bool,
    /// 当前正在执行的动作（如 Walk、Eat 等）
    activity: CatActivity,
    /// 请求切换到指定动作（每帧在 tick 中消费）
    /// 由按钮点击或状态切换函数设置
    requested_activity: Option<CatActivity>,
    /// 上一帧的渲染命令，用于提取当前要绘制的纹理
    last_render: Option<RenderCommand>,
    /// 提示信息文本，显示在控制面板上
    message: SharedString,
}

/// PetSharedState 的方法实现。
impl PetSharedState {
    /// 创建初始状态。
    ///
    /// # 参数
    /// * `screen_width` - 屏幕宽度（用于计算物理边界）
    /// * `screen_height` - 屏幕高度（用于计算物理边界）
    ///
    /// # 返回
    /// 返回初始化的 PetSharedState，猫处于自动散步状态。
    fn new(screen_width: f32, screen_height: f32) -> Self {
        // 创建角色运行时
        let mut runtime = CharacterRuntime::new();
        // 配置物理参数
        runtime.physics = PhysicsConfig {
            // 摩擦力为零：每帧速度归零，由行为直接设置速度
            friction: 0.0,
            // 物理边界：扩展窗口边界 ±120px，允许猫部分移出窗口
            bounds: Rect::new(
                -120.0,
                -120.0,
                screen_width - WINDOW_SIZE + 240.0,
                screen_height - WINDOW_SIZE + 240.0,
            ),
            // 禁用弹跳
            bounce: false,
        };

        // 注册猫的所有动画资源到运行时
        register_cat_assets(&mut runtime);

        // 创建猫角色实例
        let mut pet = Character::new("desktop-cat");
        // 设置初始位置为屏幕中心
        pet.position = Vec2::new(
            (screen_width - WINDOW_SIZE) / 2.0,
            (screen_height - WINDOW_SIZE) / 2.0,
        );
        // 初始播放散步动画
        pet.animation.play(CatActivity::Walk.animation_name());
        // 将猫添加到运行时
        runtime.add_character(pet);

        // 返回初始状态
        Self {
            runtime,
            running: true,               // 自动运行
            passthrough: true,           // 初始穿透（面板隐藏）
            interactive: false,          // 非交互模式
            activity: CatActivity::Walk, // 初始动作：散步
            requested_activity: None,    // 无待处理请求
            last_render: None,           // 无渲染命令
            message: "Ctrl+Shift+P 暂停或恢复；按住 Ctrl 可拖动窗口".into(),
        }
    }

    /// 获取猫当前在屏幕上的位置（以像素为单位）。
    ///
    /// # 返回
    /// 返回 Point<Pixels> 类型的位置坐标。
    fn position(&self) -> Point<Pixels> {
        // 从运行时获取第一个角色的位置
        let position = self.runtime.characters[0].position;
        // 转换为 Point<Pixels>
        point(px(position.x), px(position.y))
    }

    /// 设置猫的位置（以像素为单位）。
    ///
    /// # 参数
    /// * `position` - 新的位置坐标（Point<Pixels>）
    fn set_position(&mut self, position: Point<Pixels>) {
        // 将 Point<Pixels> 转换为 Vec2 并设置到角色
        self.runtime.characters[0].position = Vec2::new(position.x.as_f32(), position.y.as_f32());
    }

    /// 切换运行/暂停状态。
    /// 由 Ctrl+Shift+P 快捷键触发。
    ///
    /// # 返回
    /// 返回切换后的 running 状态（true=运行中，false=已暂停）。
    fn toggle_running(&mut self) -> bool {
        // 翻转运行状态
        self.running = !self.running;
        // passthrough 与 running 同步：运行时穿透，暂停时关闭穿透
        self.passthrough = self.running;
        // interactive 与 running 相反：运行时非交互，暂停时交互
        self.interactive = !self.running;
        if self.running {
            // 恢复运行：请求散步动作
            self.requested_activity = Some(CatActivity::Walk);
            self.message = "自动运行中".into();
        } else {
            // 暂停：设置睡觉动作
            self.activity = CatActivity::Sleep;
            self.requested_activity = Some(CatActivity::Sleep);
            self.message = "已暂停，控制面板可点击".into();
        }
        // 返回当前运行状态
        self.running
    }

    /// 处理用户通过控制面板按钮请求的动作。
    ///
    /// # 参数
    /// * `activity` - 用户请求的动作（散步、吃东西等）
    fn request_activity(&mut self, activity: CatActivity) {
        // 睡眠时停止运行，其他动作保持运行
        self.running = !matches!(activity, CatActivity::Sleep);
        // 散步退出交互模式，其他动作保持交互
        self.interactive = !matches!(activity, CatActivity::Walk);
        // 交互模式时关闭穿透（显示面板），非交互时开启穿透
        self.passthrough = !self.interactive;
        // 记录待处理的动作请求
        self.requested_activity = Some(activity);
        // 更新提示信息
        self.message = format!("已请求动作：{}", activity.label()).into();
    }

    /// 进入交互模式（由 Ctrl 键触发）。
    /// 交互模式下打开控制面板，关闭鼠标穿透。
    fn enter_interactive(&mut self) {
        self.running = false; // 停止自动运行
        self.passthrough = false; // 关闭穿透（显示面板）
        self.interactive = true; // 进入交互模式
        self.activity = CatActivity::Sleep; // 猫暂时入睡
        // 请求睡觉动作让角色播放睡眠动画
        self.requested_activity = Some(CatActivity::Sleep);
        self.message = "交互模式：点击按钮或再次按 Ctrl 退出".into();
    }

    /// 退出交互模式，回到自动散步状态。
    /// 强制回到 Walk，确保鼠标穿透立刻开启。
    fn exit_interactive(&mut self) {
        self.running = true; // 恢复自动运行
        self.interactive = false; // 退出交互模式
        self.passthrough = true; // 开启穿透（面板隐藏）
        // 强制散步：退出交互时始终回到 Walk，不恢复上一次活动
        // 这样能确保鼠标穿透立刻生效（只有散步时穿透打开）
        self.requested_activity = Some(CatActivity::Walk);
        self.message = "自动运行中".into();
    }

    /// 更新猫的状态。
    /// 每次调用都会同步 passthrough 状态：
    /// - 非交互模式下：散步时开启鼠标穿透，非散步时关闭穿透（显示面板）
    /// - 交互模式下：由 enter_interactive / request_activity 控制
    ///
    /// # 参数
    /// * `behavior` - 猫的行为控制器（决定何时切换动作）
    fn tick(&mut self, behavior: &mut CatBehavior) {
        // 如果有待处理的动作请求，强制行为控制器立即切换
        if let Some(activity) = self.requested_activity.take() {
            behavior.force(activity);
        }

        // 根据运行状态选择行为控制器并更新运行时
        let commands = if self.running {
            // 自动运行：使用猫的默认行为（散步、进食等的随机切换）
            self.runtime.update(FRAME_TIME.as_secs_f32(), behavior)
        } else {
            // 暂停/交互：使用睡眠行为
            self.runtime
                .update(FRAME_TIME.as_secs_f32(), &mut SleepBehavior)
        };

        // 更新当前动作记录
        if !self.running {
            // 非运行状态（暂停或交互）时动作始终为睡觉
            self.activity = CatActivity::Sleep;
        } else {
            // 运行状态时从行为控制器获取当前动作
            self.activity = behavior.activity();
        }
        // 只取第一个渲染命令（只有一个角色）
        self.last_render = commands.into_iter().next();

        // 非交互模式下：根据当前活动动态决定鼠标穿透
        // 散步时开启穿透（面板隐藏），非散步时关闭穿透（面板可见可交互）
        if !self.interactive {
            self.passthrough = self.activity == CatActivity::Walk;
        }
    }

    /// 获取当前帧的要绘制的纹理路径。
    ///
    /// # 返回
    /// 返回纹理路径字符串。
    fn current_texture_path(&self) -> SharedString {
        // 从最后一次渲染命令中提取纹理路径
        let Some(RenderCommand::DrawSprite { texture, .. }) = &self.last_render else {
            // 没有渲染命令时返回默认的散步帧
            return frame_path(CatActivity::Walk, 0).into();
        };
        // 返回纹理路径
        texture.0.clone().into()
    }

    /// 从渲染命令中提取水平缩放（用于翻转）。
    /// 猫向右走时 scale_x = 1.0，向左走时 scale_x = -1.0。
    ///
    /// # 返回
    /// 返回水平缩放值。
    fn current_scale_x(&self) -> f32 {
        // 从最后一次渲染命令中提取缩放
        let Some(RenderCommand::DrawSprite { scale, .. }) = &self.last_render else {
            // 没有渲染命令时返回默认缩放
            return 1.0;
        };
        // 返回水平缩放
        scale.x
    }
}

/// 注册猫的所有动画资产到运行时。
///
/// # 参数
/// * `runtime` - 角色运行时
fn register_cat_assets(runtime: &mut CharacterRuntime) {
    // 为每种动作注册动画
    for activity in [
        CatActivity::Walk,
        CatActivity::Sleep,
        CatActivity::Eat,
        CatActivity::Spin,
        CatActivity::Yawn,
        CatActivity::Turn,
    ] {
        // 注册动画片段：包含名称、帧纹理、帧速率和是否循环
        runtime.assets.register_animation(AnimationClip::new(
            // 动画名称（如 "walk"）
            activity.animation_name(),
            // 帧纹理列表（0-3 共 4 帧）
            (0..4)
                .map(|frame| TextureId::new(frame_path(activity, frame)))
                .collect(),
            // 帧速率（每秒播放帧数），不同动作速度不同
            match activity {
                CatActivity::Walk | CatActivity::Spin | CatActivity::Turn => 8.0,
                CatActivity::Eat => 6.0,
                CatActivity::Yawn => 5.0,
                CatActivity::Sleep => 2.0,
            },
            // 是否循环播放
            true,
        ));
    }
}

/// 生成指定动作和帧序号的图片路径。
///
/// # 参数
/// * `activity` - 动作类型
/// * `frame` - 帧序号（0-3）
///
/// # 返回
/// 返回资源路径字符串，如 "assets/cat/walk_0.png"。
fn frame_path(activity: CatActivity, frame: usize) -> String {
    format!("assets/cat/{}_{}.png", activity.animation_name(), frame)
}

/// 小猫自动行为。
/// 控制猫的随机移动、动作切换和边界检测。
struct CatBehavior {
    /// 当前活动（如 Walk、Eat 等）
    activity: CatActivity,
    /// 移动方向角度（弧度）
    angle: f32,
    /// 移动速度（像素/秒）
    speed: f32,
    /// 方向变化计时器（累积时间，达到 interval 后改变方向）
    direction_timer: f32,
    /// 方向变化间隔（随机值，2-5 秒）
    direction_interval: f32,
    /// 当前活动持续时间计时器
    activity_timer: f32,
    /// 当前活动总持续时间
    activity_duration: f32,
    /// 屏幕宽度（用于边界检测）
    screen_width: f32,
    /// 屏幕高度（用于边界检测）
    screen_height: f32,
    /// 是否已请求播放当前动作的动画（首次进入动作时设为 true）
    play_requested: bool,
    /// 转头动画计时器
    turn_timer: f32,
    /// 转头动画持续时间
    turn_duration: f32,
    /// 转头后的新角度（朝向屏幕中心）
    turn_target_angle: f32,
}

/// CatBehavior 的方法实现。
impl CatBehavior {
    /// 创建新的猫行为控制器。
    ///
    /// # 参数
    /// * `screen_width` - 屏幕宽度
    /// * `screen_height` - 屏幕高度
    ///
    /// # 返回
    /// 返回初始化的行为控制器，猫从散步开始。
    fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            activity: CatActivity::Walk,                // 初始动作：散步
            angle: 0.0,                                 // 初始角度：向右
            speed: 86.0,                                // 速度：86像素/秒
            direction_timer: 0.0,                       // 方向计时器归零
            direction_interval: 2.0 + rand_f32() * 3.0, // 2-5秒后改变方向
            activity_timer: 0.0,                        // 活动计时器归零
            activity_duration: 6.0,                     // 初始活动持续6秒
            screen_width,                               // 屏幕宽度
            screen_height,                              // 屏幕高度
            play_requested: true,                       // 请求播放动画
            turn_timer: 0.0,                            // 转头计时器归零
            turn_duration: 0.0,                         // 转头持续时间
            turn_target_angle: 0.0,                     // 转头目标角度
        }
    }

    /// 获取当前活动。
    ///
    /// # 返回
    /// 返回当前 CatActivity。
    fn activity(&self) -> CatActivity {
        self.activity
    }

    /// 强制切换到指定活动（由外部请求触发，如按钮点击）。
    ///
    /// # 参数
    /// * `activity` - 要切换到的活动
    fn force(&mut self, activity: CatActivity) {
        // 设置活动类型
        self.activity = activity;
        // 重置活动计时器
        self.activity_timer = 0.0;
        // 根据活动类型设置持续时间
        self.activity_duration = match activity {
            CatActivity::Walk => 6.0 + rand_f32() * 4.0, // 散步：6-10秒
            CatActivity::Eat => 3.0,                     // 吃东西：3秒
            CatActivity::Spin => 2.2,                    // 转圈圈：2.2秒
            CatActivity::Yawn => 2.6,                    // 打哈欠：2.6秒
            CatActivity::Sleep => 4.0,                   // 睡觉：4秒
            CatActivity::Turn => 0.6,                    // 转头：0.6秒
        };
        // 重置转头计时器
        self.turn_timer = 0.0;
        // 标记需要播放动画
        self.play_requested = true;
    }

    /// 自动更新活动（根据计时器到期随机切换到新活动）。
    ///
    /// # 参数
    /// * `dt` - 帧间隔时间（秒）
    fn update_activity(&mut self, dt: f32) {
        // 累积活动持续时间
        self.activity_timer += dt;
        // 如果还未到切换时间，保持当前活动
        if self.activity_timer < self.activity_duration {
            return;
        }

        // 随机选择下一个活动（概率分布）
        let roll = rand_f32();
        let next = if roll < 0.55 {
            CatActivity::Walk // 55% 概率继续散步
        } else if roll < 0.72 {
            CatActivity::Eat // 17% 概率吃东西
        } else if roll < 0.88 {
            CatActivity::Yawn // 16% 概率打哈欠
        } else {
            CatActivity::Spin // 12% 概率转圈圈
        };
        // 切换到选中的活动
        self.force(next);
    }

    /// 检测是否碰到屏幕边界，需要转头。
    ///
    /// 注意：由于 physics friction = 0.0，每帧 velocity 都会被归零，
    /// 因此不能依赖 ctx.velocity 来判断移动方向，改用 angle 来推断。
    ///
    /// # 参数
    /// * `ctx` - 行为上下文（包含角色当前位置）
    ///
    /// # 返回
    /// 返回是否需要转头（true=需要转头以避开边界）。
    fn check_boundary_turn(&mut self, ctx: &BehaviorContext) -> bool {
        // 边距：在距离边界 20px 时触发转头
        let margin = 20.0;
        // 物理边界内的安全区域
        let max_x = self.screen_width - WINDOW_SIZE + 120.0 - margin;
        let min_x = -120.0 + margin;
        let max_y = self.screen_height - WINDOW_SIZE + 120.0 - margin;
        let min_y = -120.0 + margin;

        // 检查猫是否在边界且朝向边界外
        let at_left = ctx.position.x <= min_x && self.angle.cos() < -0.1;
        let at_right = ctx.position.x >= max_x && self.angle.cos() > 0.1;
        let at_top = ctx.position.y <= min_y && self.angle.sin() < -0.1;
        let at_bottom = ctx.position.y >= max_y && self.angle.sin() > 0.1;

        // 如果在任一边界且朝向边界外，触发转头
        if at_left || at_right || at_top || at_bottom {
            // 计算朝向屏幕中心的角度
            let center_x = self.screen_width / 2.0;
            let center_y = self.screen_height / 2.0;
            self.turn_target_angle = (center_y - ctx.position.y).atan2(center_x - ctx.position.x);
            return true;
        }
        // 不需要转头
        false
    }

    /// 更新移动方向（随机改变角度，模拟随机行走）。
    ///
    /// # 参数
    /// * `ctx` - 行为上下文（包含增量时间等）
    ///
    /// # 返回
    /// 返回新的速度向量。
    fn update_direction(&mut self, ctx: &BehaviorContext) -> Vec2 {
        // 紧急回正：如果角色已经超出边界（物理 clamp 后的位置），强制转向中心
        let phys_max_x = self.screen_width - WINDOW_SIZE + 120.0;
        let phys_min_x = -120.0;
        let phys_max_y = self.screen_height - WINDOW_SIZE + 120.0;
        let phys_min_y = -120.0;

        if ctx.position.x >= phys_max_x
            || ctx.position.x <= phys_min_x
            || ctx.position.y >= phys_max_y
            || ctx.position.y <= phys_min_y
        {
            // 计算朝向屏幕中心的角度
            let center_x = self.screen_width / 2.0;
            let center_y = self.screen_height / 2.0;
            self.angle = (center_y - ctx.position.y).atan2(center_x - ctx.position.x);
            // 重置方向计时器，防止马上转向
            self.direction_timer = 0.0;
            self.direction_interval = 1.5;
        }

        // 累积方向计时器
        self.direction_timer += ctx.dt;
        // 如果到达方向变化间隔，随机改变方向
        if self.direction_timer >= self.direction_interval {
            self.direction_timer = 0.0;
            // 设置下一次变化间隔（2-5 秒后）
            self.direction_interval = 2.0 + rand_f32() * 3.0;
            // 随机改变角度（±45度）
            let change = (rand_f32() - 0.5) * std::f32::consts::PI * 0.5;
            self.angle += change;
        }

        // 根据角度和速度计算速度向量
        Vec2::new(self.angle.cos() * self.speed, self.angle.sin() * self.speed)
    }
}

/// 为 CatBehavior 实现 Behavior trait，使其可用作角色行为控制器。
impl Behavior for CatBehavior {
    /// 每帧更新行为，决定角色的下一个动作。
    ///
    /// # 参数
    /// * `ctx` - 行为上下文（包含位置、增量时间等信息）
    ///
    /// # 返回
    /// 返回行为动作（播放动画、移动、待机等）。
    fn update(&mut self, ctx: &mut BehaviorContext) -> BehaviorAction {
        // 更新活动（计时器到期时随机切换）
        self.update_activity(ctx.dt);

        // 如果在转头动画中，等待转头完成
        if self.activity == CatActivity::Turn {
            // 累积转头计时器
            self.turn_timer += ctx.dt;
            // 转头完成后
            if self.turn_timer >= self.turn_duration {
                // 应用转头后的角度
                self.angle = self.turn_target_angle;
                // 重置方向计时器，转头后不立即变向
                self.direction_timer = 0.0;
                self.direction_interval = 2.0;
                // 转完头后继续散步
                self.force(CatActivity::Walk);
                // 播放散步动画
                return BehaviorAction::PlayAnimation(CatActivity::Walk.animation_name().into());
            }
            // 转头动画播放中，保持待机
            return BehaviorAction::Idle;
        }

        // 如果是第一次进入此动作，播放对应的动画
        if self.play_requested {
            self.play_requested = false;
            return BehaviorAction::PlayAnimation(self.activity.animation_name().into());
        }

        // 对于会移动的活动（散步），处理边界转头和方向更新
        if self.activity.moves() {
            // 检测边界转头（只在靠近边界时触发转头动画）
            if self.check_boundary_turn(ctx) {
                // 切换到转头活动
                self.activity = CatActivity::Turn;
                self.turn_timer = 0.0;
                self.turn_duration = 0.6;
                self.activity_timer = 0.0;
                // 播放转头动画
                return BehaviorAction::PlayAnimation(CatActivity::Turn.animation_name().into());
            }

            // 正常行走：更新方向并返回移动动作
            BehaviorAction::Move(self.update_direction(ctx))
        } else {
            // 非移动活动（吃东西、打哈欠等）：待机播放动画
            BehaviorAction::Idle
        }
    }
}

/// 睡眠行为。
/// 用于暂停状态，始终播放睡眠动画。
struct SleepBehavior;

/// 为 SleepBehavior 实现 Behavior trait。
impl Behavior for SleepBehavior {
    /// 每帧返回睡眠动画播放请求。
    fn update(&mut self, _ctx: &mut BehaviorContext) -> BehaviorAction {
        // 始终播放睡眠动画
        BehaviorAction::PlayAnimation(CatActivity::Sleep.animation_name().into())
    }
}

// Windows 平台：链接 user32.dll 以使用 GetAsyncKeyState API
#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    // 获取指定虚拟键的状态（是否正在按下）。
    // # 参数
    // * `v_key` - 虚拟键代码（如 VK_CONTROL=0x11）
    // # 返回
    // 返回 i16：高位（bit 15）表示当前是否按下
    fn GetAsyncKeyState(v_key: i32) -> i16;
}

/// Windows：Ctrl 键的虚拟键代码
#[cfg(target_os = "windows")]
const VK_CONTROL: i32 = 0x11;

/// Windows：Shift 键的虚拟键代码
#[cfg(target_os = "windows")]
const VK_SHIFT: i32 = 0x10;

/// 检查 Ctrl 键是否正在按下。
///
/// # 返回
/// true = Ctrl 键当前处于按下状态
#[cfg(target_os = "windows")]
fn is_ctrl_pressed() -> bool {
    // GetAsyncKeyState 返回值 < 0 表示高位被设置（键正在按下）
    unsafe { GetAsyncKeyState(VK_CONTROL) < 0 }
}

/// 检查 Shift 键是否按下。
/// 用于区分 Ctrl 单独按下 vs Ctrl+Shift 组合键（如 Ctrl+Shift+P 快捷键）。
///
/// # 返回
/// true = Shift 键当前处于按下状态
#[cfg(target_os = "windows")]
fn is_shift_pressed() -> bool {
    // GetAsyncKeyState 返回值 < 0 表示高位被设置（键正在按下）
    unsafe { GetAsyncKeyState(VK_SHIFT) < 0 }
}

/// 非 Windows 平台：Ctrl 键检测始终返回 false
#[cfg(not(target_os = "windows"))]
fn is_ctrl_pressed() -> bool {
    false
}

/// 非 Windows 平台：Shift 键检测始终返回 false
#[cfg(not(target_os = "windows"))]
fn is_shift_pressed() -> bool {
    false
}

/// 生成伪随机 f32 值（0.0 ~ 1.0）。
/// 使用简单的线性同余生成器，无需依赖 rand crate。
///
/// # 返回
/// 返回 [0.0, 1.0) 范围内的 f32 值。
fn rand_f32() -> f32 {
    // 导入原子类型
    use std::sync::atomic::{AtomicU64, Ordering};
    // 静态状态变量（线程安全）
    static STATE: AtomicU64 = AtomicU64::new(1);
    // 原子递增并获取
    let mut value = STATE.fetch_add(1, Ordering::Relaxed);
    // 线性同余生成器步骤
    value = value.wrapping_mul(6364136223846793005).wrapping_add(1);
    // 提取高位并归一化到 [0.0, 1.0)
    ((value >> 33) as f32) / (u32::MAX as f32)
}

/// 程序入口点。
fn main() {
    // 尝试获取单例实例（防止重复启动）
    let _instance = match SingleInstance::acquire(APP_ID) {
        Ok(instance) => instance, // 首次运行，获取成功
        Err(_) => {
            // 已有实例在运行
            eprintln!("Another instance is already running. Sending activation signal.");
            // 向已有实例发送激活信号
            let _ = send_activate_to_existing(APP_ID);
            // 退出当前进程
            std::process::exit(0);
        }
    };

    // 获取 Cargo 清单目录路径
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // 创建共享状态（使用 Arc+Mutex 跨线程访问）
    let pet_state = Arc::new(Mutex::new(PetSharedState::new(1920.0, 1080.0)));

    // 启动 rgpui 应用程序
    application()
        // 设置资源加载器
        .with_assets(ExampleAssets {
            base: manifest_dir.join("examples"),
        })
        // 运行应用程序
        .run(move |cx: &mut App| {
            // 关闭所有窗口后保持进程运行（托盘图标需要）
            cx.set_keep_alive_without_windows(true);
            // 设置系统托盘
            setup_tray(cx);

            // 计算窗口居中位置
            let bounds = Bounds::centered(None, size(px(WINDOW_SIZE), px(WINDOW_SIZE)), cx);
            // 窗口可见性标志
            let window_visible = Arc::new(std::sync::atomic::AtomicBool::new(true));
            let window_visible_close = window_visible.clone();
            let view_state = pet_state.clone();

            // 打开覆盖窗口
            let window_handle = cx
                .open_window(
                    // 窗口创建选项
                    WindowOptions {
                        titlebar: None,                                             // 无标题栏
                        window_bounds: Some(WindowBounds::Windowed(bounds)),        // 窗口位置
                        window_background: WindowBackgroundAppearance::Transparent, // 透明背景
                        mouse_passthrough: true, // 初始鼠标穿透（由 NCHITTEST 动态控制）
                        kind: WindowKind::Overlay, // 覆盖窗口类型（置顶、无装饰）
                        ..Default::default()     // 其他选项使用默认值
                    },
                    // 视图创建回调
                    |window, cx| {
                        // 创建桌宠视图
                        let view = cx.new(|_| DesktopPet::new(view_state));
                        // 注册窗口关闭事件处理
                        window.on_window_should_close(cx, move |window, _cx| {
                            // 标记窗口已隐藏
                            window_visible_close.store(false, std::sync::atomic::Ordering::Release);
                            // 隐藏窗口而非关闭（保留进程和托盘）
                            window.hide_window();
                            // 阻止窗口真正关闭
                            false
                        });
                        // 返回视图
                        view
                    },
                )
                .expect("failed to open desktop pet window");

            // 设置托盘回调
            setup_tray_callbacks(cx, window_handle, window_visible);
            // 设置全局快捷键
            setup_global_hotkey(cx, pet_state.clone(), window_handle);
            // 启动主循环
            spawn_pet_loop(cx, pet_state, window_handle);

            // 激活应用程序
            cx.activate(true);
        });
}

/// 设置系统托盘图标和菜单。
///
/// # 参数
/// * `cx` - 应用程序上下文
fn setup_tray(cx: &mut App) {
    // 设置托盘提示文字
    cx.set_tray_tooltip("rgpui-character 小猫桌宠");
    // 加载托盘图标
    let icon_bytes = include_bytes!("assets/tray-icon.png");
    // 设置托盘图标
    cx.set_tray_icon(Some(icon_bytes.as_slice()));
    // 设置托盘右键菜单
    cx.set_tray_menu(vec![
        // 显示窗口菜单项
        TrayMenuItem::Action {
            label: "显示窗口".into(),
            id: "show_window".into(),
        },
        // 分隔线
        TrayMenuItem::Separator,
        // 退出菜单项
        TrayMenuItem::Action {
            label: "退出".into(),
            id: "quit".into(),
        },
    ]);
}

/// 设置系统托盘事件回调。
///
/// # 参数
/// * `cx` - 应用程序上下文
/// * `window_handle` - 窗口句柄
/// * `window_visible` - 窗口可见性标志
fn setup_tray_callbacks(
    cx: &mut App,
    window_handle: rgpui::WindowHandle<DesktopPet>,
    window_visible: Arc<std::sync::atomic::AtomicBool>,
) {
    let click_handle = window_handle;
    let click_visible = window_visible.clone();
    // 托盘图标点击事件（左键或双击）
    cx.on_tray_icon_event(move |event, cx| {
        // 仅当窗口隐藏时响应点击
        if matches!(event, TrayIconEvent::LeftClick | TrayIconEvent::DoubleClick)
            && !click_visible.load(std::sync::atomic::Ordering::Acquire)
        {
            let visible = click_visible.clone();
            // 更新窗口并显示
            click_handle
                .update(cx, |_, window, _| {
                    // 标记窗口可见
                    visible.store(true, std::sync::atomic::Ordering::Release);
                    // 恢复窗口
                    window.activate_window();
                })
                .ok();
        }
    });

    // 托盘菜单动作事件
    cx.on_tray_menu_action(move |id, cx| match id.as_ref() {
        "quit" => cx.quit(), // 退出应用程序
        "show_window" => {
            let visible = window_visible.clone();
            // 更新窗口并显示
            if let Err(err) = window_handle.update(cx, |_, window, _| {
                visible.store(true, std::sync::atomic::Ordering::Release);
                window.activate_window();
            }) {
                eprintln!("Failed to activate window: {}", err);
            }
        }
        _ => {} // 忽略未知菜单项
    });
}

/// 注册全局快捷键（Ctrl+Shift+P 暂停/恢复）。
///
/// # 参数
/// * `cx` - 应用程序上下文
/// * `pet_state` - 桌宠共享状态
/// * `window_handle` - 窗口句柄（用于更新窗口状态）
fn setup_global_hotkey(
    cx: &mut App,
    pet_state: Arc<Mutex<PetSharedState>>,
    window_handle: rgpui::WindowHandle<DesktopPet>,
) {
    // 解析快捷键按键组合
    let keystroke = Keystroke::parse("ctrl-shift-p").expect("valid keystroke");
    // 注册全局快捷键
    if let Err(err) = cx.register_global_hotkey(HOTKEY_TOGGLE_PAUSE, &keystroke) {
        eprintln!("Failed to register global hotkey: {}", err);
    }

    // 监听快捷键触发事件
    cx.on_global_hotkey(move |id, cx| {
        // 仅处理暂停/恢复快捷键
        if id == HOTKEY_TOGGLE_PAUSE {
            // 切换运行/暂停状态
            let running = pet_state.lock().unwrap().toggle_running();
            // 更新窗口状态
            if let Err(err) = window_handle.update(cx, |_, window, cx| {
                if running {
                    // 恢复运行：记录当前位置，开启鼠标穿透
                    let current_position = window.bounds().origin;
                    pet_state.lock().unwrap().set_position(current_position);
                    window.set_mouse_passthrough(true);
                } else {
                    // 暂停：关闭鼠标穿透，激活窗口显示面板
                    window.set_mouse_passthrough(false);
                    window.activate_window();
                }
                // 触发重绘
                cx.notify();
            }) {
                eprintln!("Failed to update window: {}", err);
            }
        }
    });
}

/// 启动桌宠主循环（异步）。
/// 每帧更新角色状态、处理 Ctrl 键交互、同步窗口位置和鼠标穿透。
///
/// # 参数
/// * `cx` - 应用程序上下文
/// * `pet_state` - 桌宠共享状态
/// * `window_handle` - 窗口句柄
fn spawn_pet_loop(
    cx: &mut App,
    pet_state: Arc<Mutex<PetSharedState>>,
    window_handle: rgpui::WindowHandle<DesktopPet>,
) {
    // 创建猫行为控制器（初始屏幕尺寸 1920x1080）
    let mut behavior = CatBehavior::new(1920.0, 1080.0);
    // 记录上一帧 Ctrl 键状态（用于检测上升沿：按下瞬间触发切换）
    let mut was_ctrl_held = false;
    // 在后台异步任务中运行主循环
    cx.spawn(async move |cx| {
        loop {
            // 等待帧间隔（约 16ms = 60 FPS）
            cx.background_executor().timer(FRAME_TIME).await;
            // 读取当前 Ctrl 和 Shift 键状态
            let ctrl_held = is_ctrl_pressed();
            let shift_held = is_shift_pressed();

            // 更新窗口状态（必须在主线程执行）
            let _ = window_handle.update(cx, |_, window, cx| {
                // 锁定共享状态
                let mut state = pet_state.lock().unwrap();
                // Ctrl 单独按下时切换交互模式（Ctrl+Shift 同时按下时不切换，
                // 避免与 Ctrl+Shift+P 快捷键冲突）
                if ctrl_held && !was_ctrl_held && !shift_held {
                    if state.interactive {
                        // 退出交互模式 → 猫恢复散步
                        state.exit_interactive();
                    } else {
                        // 进入交互模式 → 打开面板，关闭穿透
                        state.enter_interactive();
                        window.set_mouse_passthrough(false);
                        window.activate_window();
                    }
                }

                // Ctrl 按下时锁定猫的位置（方便拖动时位置不自动变化）
                if ctrl_held {
                    state.set_position(window.bounds().origin);
                }

                // 更新上一帧 Ctrl 状态
                was_ctrl_held = ctrl_held;

                if state.interactive {
                    // 交互模式：强制关闭穿透，让面板可点击
                    window.set_mouse_passthrough(false);
                    state.tick(&mut behavior);
                } else if state.running {
                    // 运行模式：先更新状态，然后根据计算结果设置穿透
                    state.tick(&mut behavior);
                    // 从更新后的状态读取穿透开关
                    let should_passthrough = state.passthrough;
                    // 读取猫的当前位置
                    let position = state.position();
                    // 释放状态锁（后续操作不需要访问状态）
                    drop(state);
                    // 同步窗口鼠标穿透状态
                    window.set_mouse_passthrough(should_passthrough);
                    // 更新窗口位置（跟随猫移动）
                    window.set_position(position);
                } else {
                    // 非运行也非交互（暂停状态）：类似处理
                    state.tick(&mut behavior);
                    let should_passthrough = state.passthrough;
                    drop(state);
                    window.set_mouse_passthrough(should_passthrough);
                }
                // 触发视图重绘
                cx.notify();
            });
        }
    })
    .detach();
}

/// 桌宠视图组件。
/// 负责渲染猫精灵和控制面板。
struct DesktopPet {
    /// 共享的桌宠运行状态
    state: Arc<Mutex<PetSharedState>>,
}

/// DesktopPet 的方法实现。
impl DesktopPet {
    /// 创建新的桌宠视图。
    ///
    /// # 参数
    /// * `state` - 共享状态
    ///
    /// # 返回
    /// 返回 DesktopPet 实例。
    fn new(state: Arc<Mutex<PetSharedState>>) -> Self {
        Self { state }
    }
}

/// 为 DesktopPet 实现 Render trait（决定 UI 如何渲染）。
impl Render for DesktopPet {
    /// 渲染桌面宠物 UI。
    ///
    /// # 参数
    /// * `_window` - 窗口引用
    /// * `_cx` - 视图上下文
    ///
    /// # 返回
    /// 返回要渲染的元素。
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // 锁定共享状态读取渲染所需数据
        let state = self.state.lock().unwrap();
        // 当前帧的纹理路径
        let texture_path = state.current_texture_path();
        // 水平缩放（用于精灵翻转）
        let scale_x = state.current_scale_x();
        // 穿透标志（决定是否显示控制面板）
        let passthrough = state.passthrough;
        // 当前活动（用于面板显示）
        let activity = state.activity;
        // 提示信息
        let message = state.message.clone();
        // 克隆 Arc 以供控制面板按钮使用
        let shared_state = self.state.clone();
        // 释放状态锁（渲染过程中不需要访问状态）
        drop(state);

        // 构建 UI 层级
        div()
            .flex() // 弹性布局
            .flex_col() // 垂直排列
            .size_full() // 填满窗口
            .overflow_hidden() // 隐藏溢出内容
            .items_center() // 水平居中
            .justify_center() // 垂直居中
            .child(
                // 猫精灵图片
                img(texture_path)
                    .size(px(SPRITE_SIZE)) // 精灵尺寸
                    .scale_xy(scale_x, 1.0) // 水平翻转
                    .with_fallback(|| div().child("图片加载失败").into_any_element()),
            )
            // 非穿透模式时显示控制面板
            .when(!passthrough, |this| {
                this.child(control_panel(shared_state, activity, message))
            })
    }
}

/// 创建控制面板 UI。
///
/// # 参数
/// * `state` - 共享状态（按钮点击时修改）
/// * `activity` - 当前活动（显示标签）
/// * `message` - 提示信息
///
/// # 返回
/// 返回控制面板的 UI 元素。
fn control_panel(
    state: Arc<Mutex<PetSharedState>>,
    activity: CatActivity,
    message: SharedString,
) -> impl IntoElement {
    div()
        .flex() // 弹性布局
        .flex_col() // 垂直排列
        .gap_2() // 间距
        .mt_2() // 上外边距
        .p_3() // 内边距
        .rounded(px(8.0)) // 圆角
        .bg(rgba(0x00000099)) // 半透明黑色背景
        .text_color(rgb(0xffffff)) // 白色文字
        .child(
            // 当前动作标签
            div()
                .text_sm() // 小号文字
                .font_weight(rgpui::FontWeight::BOLD) // 粗体
                .child(format!("当前动作：{}", activity.label())),
        )
        // 提示信息
        .child(div().text_xs().child(message))
        // 动作按钮行
        .child(
            div()
                .flex() // 水平排列
                .gap_1() // 按钮间距
                // 散步按钮
                .child(action_button(state.clone(), CatActivity::Walk))
                // 吃东西按钮
                .child(action_button(state.clone(), CatActivity::Eat))
                // 转圈圈按钮
                .child(action_button(state.clone(), CatActivity::Spin))
                // 打哈欠按钮
                .child(action_button(state.clone(), CatActivity::Yawn))
                // 睡觉按钮
                .child(action_button(state, CatActivity::Sleep)),
        )
}

/// 创建动作按钮。
///
/// # 参数
/// * `state` - 共享状态（点击时修改）
/// * `activity` - 按钮对应的动作
///
/// # 返回
/// 返回动作按钮的 UI 元素。
fn action_button(state: Arc<Mutex<PetSharedState>>, activity: CatActivity) -> impl IntoElement {
    div()
        // 唯一标识符（用于 gpui 事件路由）
        .id(format!("activity-{}", activity.animation_name()))
        .px_2() // 水平内边距
        .py_1() // 垂直内边距
        .rounded_sm() // 小圆角
        .bg(rgba(0xffffffdd)) // 半透明白色背景
        .text_color(rgb(0x333333)) // 深色文字
        .text_xs() // 特小号文字
        .cursor_pointer() // 鼠标指针样式
        // 按钮标签
        .child(activity.label())
        // 点击事件处理
        .on_click(move |_, _, _| {
            // 修改共享状态：请求切换动作
            state.lock().unwrap().request_activity(activity);
        })
}
