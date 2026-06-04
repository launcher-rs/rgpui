# 窗口管理

> 窗口创建、视图与焦点管理。

## 概述

rgpui 的窗口系统提供了跨平台的窗口管理能力，包括：
- 窗口创建和销毁
- 视图管理和替换
- 焦点管理
- 窗口状态（最小化、最大化、隐藏）

## 创建窗口

### 基本创建

```rust
use rgpui::Window;

// 使用默认配置创建窗口
cx.open_window(Window::default(), |window, cx| {
    // 设置根视图
    window.replace_root_view(cx, |window, cx| {
        MyView::new(cx)
    });
});
```

### 自定义窗口配置

```rust
cx.open_window(
    Window {
        title: "My Application".into(),
        width: 800.0,
        height: 600.0,
        ..Default::default()
    },
    |window, cx| {
        window.replace_root_view(cx, |window, cx| {
            MyView::new(cx)
        });
    }
);
```

## 视图管理

### 替换根视图

```rust
window.replace_root_view(cx, |window, cx| {
    // 创建新的根视图
    MyView::new(cx)
});
```

### 获取当前视图

```rust
let view_entity = window.current_view();
```

### 更新视图

```rust
window.update(cx, |view, cx| {
    // view 是 AnyView 类型
    // 可以向下转换为具体类型
    if let Ok(editor) = view.downcast::<Editor>() {
        editor.update(cx, |editor, cx| {
            editor.do_something();
        });
    }
});
```

## 焦点管理

### 基本焦点

```rust
use rgpui::Focusable;

impl Focusable for MyView {}

impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)  // 跟踪焦点
    }
}
```

### 焦点处理

```rust
impl MyView {
    fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        
        // 注册焦点变化回调
        cx.on_focus(&focus_handle, |cx| {
            // 获得焦点
        });
        
        cx.on_blur(&focus_handle, |cx| {
            // 失去焦点
        });
        
        Self { focus_handle }
    }
}
```

### 编程式焦点

```rust
// 聚焦到特定实体
cx.focus(&other_entity);

// 检查是否聚焦
if focus_handle.is_focused(cx) {
    // 当前拥有焦点
}
```

### 焦点容器

```rust
impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .focus(|style| style.bg(gpui::blue()))  // 聚焦时的样式
            .track_focus(&self.focus_handle)
            .child(
                div()
                    .child("Click me")
                    .on_click(cx.listener(|this, _, cx| {
                        this.focus_handle.focus(cx);
                    }))
            )
    }
}
```

## 窗口操作

### 窗口状态

```rust
// 最小化窗口
window.minimize();

// 最大化窗口
window.maximize();

// 恢复窗口
window.restore();

// 隐藏窗口
window.hide();

// 激活窗口（带到前台）
window.activate();
```

### 窗口属性

```rust
// 设置标题
window.set_title("New Title");

// 设置窗口大小
window.set_size(Size {
    width: px(800.0),
    height: px(600.0),
});

// 设置窗口位置
window.set_position(Point {
    x: px(100.0),
    y: px(100.0),
});

// 获取窗口边界
let bounds = window.bounds();
```

### 窗口关闭

```rust
// 注册关闭回调
cx.on_app_quit(|cx| {
    // 在应用退出前执行清理
    // 可以返回 Future 来延迟退出
});

// 关闭当前窗口
window.close_window();
```

## 系统托盘

### 创建托盘图标

```rust
use rgpui::Tray;

// 设置托盘图标
cx.set_tray_icon(Tray {
    tooltip: "My App".into(),
    icon: Some(icon_image),
    ..Default::default()
});
```

### 托盘菜单

```rust
cx.set_tray_menu(|cx| {
    vec![
        MenuItem::action("Show", tray::Show),
        MenuItem::separator(),
        MenuItem::action("Quit", tray::Quit),
    ]
});
```

### 托盘事件

```rust
cx.on_tray_menu_action(|action, cx| {
    match action {
        tray::Show => {
            // 显示窗口
        }
        tray::Quit => {
            cx.quit();
        }
    }
});
```

## 窗口层级

### 窗口组

```rust
// 创建窗口组
let group = cx.create_window_group();

// 将窗口添加到组
group.add_window(window_handle);
```

### 窗口层级

```rust
// 设置窗口层级
window.set_z_index(100);
```

## 多窗口示例

```rust
impl Workspace {
    fn new(cx: &mut Context<Self>) -> Self {
        // 创建主窗口
        cx.open_window(Window::default(), |window, cx| {
            window.replace_root_view(cx, |window, cx| {
                MainView::new(cx)
            });
        });
        
        Self {}
    }
    
    fn open_new_window(&mut self, cx: &mut Context<Self>) {
        // 打开新窗口
        cx.open_window(Window::default(), |window, cx| {
            window.replace_root_view(cx, |window, cx| {
                SecondaryView::new(cx)
            });
        });
    }
}
```

## 最佳实践

1. **窗口配置**：合理设置初始大小和位置
2. **焦点管理**：确保键盘操作在正确的窗口中工作
3. **资源清理**：窗口关闭时清理相关资源
4. **托盘集成**：提供最小化到托盘的功能
