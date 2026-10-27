# slint-borderless

[English](README.md) | 简体中文

`slint-borderless` 为 Slint 提供由 Windows 原生窗口管理器控制的无边框窗口。

它只隐藏系统绘制的非客户区，不会把窗口改成 `WS_POPUP`。窗口仍保留
`WS_CAPTION`、`WS_THICKFRAME`、`WS_SYSMENU`、最小化和最大化能力，因此以下行为继续由 Windows/DWM 负责：

- 四边、四角调整窗口大小；
- 拖动标题栏和双击最大化；
- `Win + 方向键`、拖到屏幕边缘贴靠；
- Windows 11 最大化按钮 Snap Layout；
- 最小化、最大化和还原动画；
- 系统菜单、任务栏、Alt+Tab 和多显示器 DPI。

crate 同时附带一个可选的 `Win32TitleBar` Slint 组件。它包含默认的几何窗口按钮，也支持替换为 PNG、SVG 或 ICO 图标。标题栏的其余内容完全由应用自己编写。

## 平台和版本

- Windows 10/11；
- Rust 1.92+ MSVC 工具链；
- Slint 1.18；
- crate 使用 MIT 许可证。

非 Windows 目标可以编译依赖，但调用 `attach` 会返回 `FrameError::UnsupportedPlatform`。

## 最少用法

如果你自己绘制标题栏，只需增加依赖并调用一次 `attach`。

```toml
[dependencies]
slint = "1.18"
slint-borderless = "0.1"
```

本仓库内开发时可使用路径依赖：

```toml
slint-borderless = { path = "../slint-borderless" }
```

Slint 根窗口必须使用 `preferred-width` / `preferred-height`，不要把 `width` 和 `height` 写成固定约束，否则 Slint 后端可能把窗口标记为不可缩放。

```slint
export component AppWindow inherits Window {
    preferred-width: 900px;
    preferred-height: 600px;
    min-width: 640px;
    min-height: 480px;

    VerticalLayout {
        Rectangle {
            height: 48px; // 必须与 FrameOptions.titlebar_height 一致
            background: #111827;
            // 在这里自由添加标题、标签页或其他内容
        }
        Rectangle { background: #eef2f7; }
    }
}
```

```rust
use slint_borderless::{FrameController, FrameOptions, LogicalSize};

let ui = AppWindow::new()?;
let frame = FrameController::attach(
    &ui,
    FrameOptions {
        minimum_size: Some(LogicalSize { width: 640.0, height: 480.0 }),
        ..Default::default()
    },
)?;

ui.run()?;
```

需要在 `ui.run()` 期间保留 `frame`，以便后续更新标题栏交互区或读取状态。即使控制器提前释放，已安装的原生子类仍会安全地活到 HWND 销毁。

## 使用默认标题栏按钮

crate 发布的 `ui/titlebar.slint` 包含：

- 默认几何最小化图标；
- 默认几何最大化图标；
- 默认几何关闭图标；
- 最小化/最大化半透明悬停；
- Windows 红色关闭悬停；
- 一个 `@children` 插槽，可在左侧自由加入 Slint 元素。

### Cargo.toml

正常依赖和构建依赖都需要引用本 crate：

```toml
[dependencies]
slint = "1.18"
slint-borderless = "0.1"

[build-dependencies]
slint-build = "1.18"
slint-borderless = "0.1"
```

### build.rs

```rust
fn main() {
    let libraries = std::collections::HashMap::from([(
        slint_borderless::SLINT_LIBRARY_NAME.to_owned(),
        slint_borderless::slint_library_path(),
    )]);

    let config = slint_build::CompilerConfiguration::new()
        .with_library_paths(libraries);
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}
```

### ui/app.slint

```slint
import { Win32TitleBar } from "@slint-borderless/titlebar.slint";

export component AppWindow inherits Window {
    preferred-width: 900px;
    preferred-height: 600px;
    in property <int> caption-hover: 0;

    VerticalLayout {
        Win32TitleBar {
            hover-button: root.caption-hover;

            Text {
                text: "  My Slint App";
                color: white;
                vertical-alignment: center;
            }
        }
        Rectangle { background: #eef2f7; }
    }
}
```

### 将原生悬停状态同步给 Slint

```rust
use slint::ComponentHandle;
use slint_borderless::{FrameController, FrameEvent, FrameOptions};

let ui = AppWindow::new()?;
let frame = FrameController::attach(&ui, FrameOptions::default())?;

let weak = ui.as_weak();
frame.on_event(move |event| {
    let Some(ui) = weak.upgrade() else { return };
    match event {
        FrameEvent::CaptionHoverChanged(button) => {
            ui.set_caption_hover(button.map_or(0, |button| button.slint_value()));
        }
        FrameEvent::Failed(error) => eprintln!("frame error: {error}"),
        FrameEvent::Installed | FrameEvent::Detached => {}
    }
});
```

悬停值约定为：`0 = 无`、`1 = 最小化`、`2 = 最大化`、`3 = 关闭`。

## 替换默认按钮图标

`Win32TitleBar` 的三个按钮都能独立替换。推荐使用透明 SVG 或 PNG；ICO 也可以使用，但需要让 Slint 启用额外图片格式。

```toml
slint = { version = "1.18", features = ["image-default-formats"] }
```

```slint
Win32TitleBar {
    hover-button: root.caption-hover;
    icon-size: 12px;

    use-custom-minimize-icon: true;
    use-custom-maximize-icon: true;
    use-custom-close-icon: true;

    minimize-icon: @image-url("icons/minimize.ico");
    maximize-icon: @image-url("icons/maximize.svg");
    close-icon: @image-url("icons/close.png");
}
```

只替换其中一个图标时，只需要启用对应的 `use-custom-...-icon`。没有启用的按钮继续使用默认几何图形。

如果你完全不想使用附带组件，可以自行绘制任意视觉。只需确保：

- 标题栏高度等于 `FrameOptions.titlebar_height`；
- 右侧依次为最小化、最大化、关闭；
- 每个按钮宽度等于 `FrameOptions.caption_button_width`。

默认尺寸为标题栏 `48 DIP`、按钮 `46 DIP`。crate 会按当前窗口 DPI 转换为物理像素。

## 在标题栏中放菜单、搜索框和按钮

标题栏默认返回 `HTCAPTION`，因此会拖动窗口。要让其中的 Slint `TouchArea`、输入框或标签页正常收到鼠标事件，需要把对应区域注册为 `ClientArea`。

例如左上角有一个 48×48 的菜单按钮：

```rust
use slint_borderless::{ClientArea, FrameController, FrameOptions};

let frame = FrameController::attach(
    &ui,
    FrameOptions {
        client_areas: vec![ClientArea::left(0.0, 0.0, 48.0, 48.0)],
        ..Default::default()
    },
)?;
```

右侧、位于系统按钮左边的控件可以从右边缘定位：

```rust
// 距离窗口右边 138 DIP，宽 180 DIP。
// 138 = 3 × 默认 caption_button_width。
ClientArea::right(138.0, 0.0, 180.0, 48.0)
```

运行时布局变化后，可以更新区域：

```rust
frame.set_client_areas(vec![
    ClientArea::left(0.0, 0.0, 48.0, 48.0),
    ClientArea::right(138.0, 0.0, search_width, 48.0),
]);
```

命中优先级为：窗口缩放边缘 → 三个系统按钮 → 自定义 ClientArea → 标题栏拖动区。

## 配置项

`FrameOptions`：

| 字段 | 默认值 | 说明 |
|---|---:|---|
| `titlebar_height` | `48.0` | 自绘标题栏高度，单位 DIP |
| `caption_button_width` | `46.0` | 每个系统按钮宽度，单位 DIP |
| `minimum_size` | `None` | 可选最小窗口外部尺寸 |
| `rounded_corners` | `true` | 请求 Windows 11 DWM 圆角 |
| `client_areas` | 空 | 标题栏中的 Slint 交互区 |

所有尺寸都是逻辑像素。100%、125%、150%、175% 和 200% 缩放都会在每次命中测试时根据 `GetDpiForWindow` 计算，不写死屏幕分辨率。

## 多窗口

每个窗口创建独立控制器：

```rust
let main_frame = FrameController::attach(&main_window, FrameOptions::default())?;
let tool_frame = FrameController::attach(&tool_window, FrameOptions::default())?;
```

内部使用 `SetWindowSubclass` 的每 HWND 独立状态，不使用全局原窗口过程。重复 attach 同一个窗口会返回 `FrameError::AlreadyAttached`。

## 已经使用 rendering notifier

常规 `attach` 会占用该窗口的 Slint rendering notifier。如果应用已经用它做自定义 OpenGL/WGPU 渲染，请在自己的 `RenderingSetup` 回调中使用：

```rust
let controller = FrameController::attach_window(window, FrameOptions::default())?;
```

此时 HWND 必须已经创建，否则会返回 `FrameError::WindowHandleUnavailable`。

## 手动卸载

```rust
frame.detach()?;
```

这会安全移除 Win32 subclass、恢复原窗口 style，并触发 `SWP_FRAMECHANGED`。普通应用无需手动调用；窗口收到 `WM_NCDESTROY` 时会自动清理。

## 安全设计

- 所有公开 API 都是安全 Rust；
- 控制器有意保持 `!Send + !Sync`，要求在 Slint UI 线程使用；
- 使用 `SetWindowSubclass`，避免直接覆盖和全局保存 `WNDPROC`；
- 每个 HWND 持有独立引用计数状态，窗口销毁时释放；
- 用户事件回调使用 `catch_unwind`，panic 不会跨越 Win32 FFI 边界；
- 未处理消息全部转发给 `DefSubclassProc`；
- 不使用 `WS_POPUP`，不模拟 Windows 窗口管理器。

## 运行仓库中的完整示例

```powershell
cargo run --manifest-path examples/demo/Cargo.toml
```

示例演示默认按钮、悬停状态、标题栏三道杠菜单、拖动、缩放与 Windows 11 Snap Layout。

## 常见问题

### 窗口不能缩放

检查 Slint 根窗口是否写了固定的 `width` / `height`。请改成 `preferred-width` / `preferred-height`。

### 标题栏中的按钮点不到

把按钮的逻辑矩形注册为 `ClientArea`。系统最小化/最大化/关闭按钮不需要注册。

### 最大化按钮能弹出 Snap Layout，但点击无反应

不要在 Slint 中给三个系统按钮添加 `TouchArea`。它们由 crate 通过非客户区消息和 `WM_SYSCOMMAND` 处理。

### 自定义图标与点击位置不一致

图标只是按钮内部视觉，不应改变按钮容器宽度。保持 `Win32TitleBar.button-width` 与 `FrameOptions.caption_button_width` 相同。

## 许可证

[MIT](LICENSE)
