# GPUI Component basic examples

This folder contains basic examples of how to use the GPUI Component library. Each example demonstrates a specific feature or functionality of the library.

Unlike the examples in the `story` folder, these examples focus on 1 example for 1 feature, making it easier to understand and implement specific functionalities in your own projects.

## 示例列表

### 桌面示例

| 示例 | 说明 |
|------|------|
| `button_example.rs` | 按钮组件：变体、状态、图标、提示、按钮组 |
| `components_overview.rs` | 组件总览：所有主要组件的基本用法 |
| `dialog_example.rs` | 对话框：基础、自定义按钮、模态、警告对话框 |
| `input_example.rs` | 输入框：基础、可清除、密码、禁用、尺寸、前缀后缀、多行 |
| `form_controls_example.rs` | 表单控件：复选框、开关、单选框 |
| `data_display_example.rs` | 数据展示组件 |
| `navigation_example.rs` | 导航组件 |
| `feedback_example.rs` | 反馈组件 |
| `select_example.rs` | 下拉选择组件 |

### Web 示例

| 示例 | 说明 |
|------|------|
| `hello_world_web/` | 最简 Web 示例，展示基本按钮和布局 |
| `components_web/` | Web 组件总览，展示常用组件在浏览器中的效果 |

运行 Web 示例请参考 [Web 开发指南](WEB.md)。

## Contributing

Feel free to contribute more examples to this folder!

If you have a specific use case or feature you'd like to demonstrate, please create a new example file and submit a pull request. We will happy to merge it into the repository.

When creating an example, please follow these guidelines:

1. Keep 1 example just doing 1 thing for more clarity.
2. Testing the example to ensure it works as expected.
3. Write some comment at some key parts of the code to explain what it does.
4. Following the code style and name style used in the existing examples or in entire of GPUI Component.
