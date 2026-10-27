# slint-borderless

English | [简体中文](README.zh-CN.md)

`slint-borderless` gives Slint borderless windows that are still managed by the native Windows window manager.

It only hides the system-drawn non-client area; it never turns the window into `WS_POPUP`. The window keeps
`WS_CAPTION`, `WS_THICKFRAME`, `WS_SYSMENU` and its minimize/maximize capabilities, so Windows/DWM remains
responsible for:

- resizing from all four edges and corners;
- dragging the title bar and double-clicking to maximize;
- `Win + Arrow` keys and snapping by dragging to screen edges;
- the Windows 11 Snap Layouts flyout on the maximize button;
- minimize, maximize and restore animations;
- the system menu, taskbar, Alt+Tab and per-monitor DPI.

The crate also ships an optional `Win32TitleBar` Slint component. It contains default geometric caption buttons
and lets you replace their icons with PNG, SVG or ICO images. Everything else in the title bar is entirely up to
your application.

## Platforms and versions

- Windows 10/11;
- Rust 1.92+ with the MSVC toolchain;
- Slint 1.18;
- licensed under MIT.

Non-Windows targets still compile, but `attach` returns `FrameError::UnsupportedPlatform`.

## Minimal usage

If you draw the title bar yourself, all you need is the dependency and a single call to `attach`.

```toml
[dependencies]
slint = "1.18"
slint-borderless = "0.1"
```

When developing against a local checkout, use a path dependency:

```toml
slint-borderless = { path = "../slint-borderless" }
```

The Slint root window must use `preferred-width` / `preferred-height`. Do not set `width` and `height` as fixed
constraints, otherwise the Slint backend may mark the window as non-resizable.

```slint
export component AppWindow inherits Window {
    preferred-width: 900px;
    preferred-height: 600px;
    min-width: 640px;
    min-height: 480px;

    VerticalLayout {
        Rectangle {
            height: 48px; // must match FrameOptions.titlebar_height
            background: #111827;
            // Put a title, tabs or anything else here
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

Keep `frame` alive for the duration of `ui.run()` so you can update title-bar client areas or read its state later.
Even if the controller is dropped early, the installed native subclass safely lives on until the HWND is destroyed.

## Using the default title-bar buttons

The crate's `ui/titlebar.slint` provides:

- a default geometric minimize icon;
- a default geometric maximize icon;
- a default geometric close icon;
- a translucent hover effect for minimize/maximize;
- the Windows red hover effect for close;
- an `@children` slot for placing any Slint elements on the left.

### Cargo.toml

Reference the crate both as a normal dependency and as a build dependency:

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

### Syncing native hover state to Slint

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

Hover values are: `0 = none`, `1 = minimize`, `2 = maximize`, `3 = close`.

## Replacing the default button icons

Each of the three `Win32TitleBar` buttons can be replaced independently. Transparent SVG or PNG is recommended;
ICO works too, but requires enabling Slint's additional image formats.

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

To replace only one icon, enable only the matching `use-custom-...-icon`. Buttons that are not enabled keep the
default geometric shape.

If you don't want the bundled component at all, draw whatever visuals you like. Just make sure that:

- the title-bar height equals `FrameOptions.titlebar_height`;
- minimize, maximize and close sit on the right, in that order;
- each button's width equals `FrameOptions.caption_button_width`.

The defaults are a `48 DIP` title bar and `46 DIP` buttons. The crate converts them to physical pixels using the
window's current DPI.

## Menus, search boxes and buttons in the title bar

The title bar returns `HTCAPTION` by default, so it drags the window. For a Slint `TouchArea`, text input or tab
in the title bar to receive mouse events, register its region as a `ClientArea`.

For example, a 48×48 menu button in the top-left corner:

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

Controls on the right, to the left of the system buttons, can be positioned from the right edge:

```rust
// 138 DIP from the right edge of the window, 180 DIP wide.
// 138 = 3 × the default caption_button_width.
ClientArea::right(138.0, 0.0, 180.0, 48.0)
```

When the layout changes at runtime, update the regions:

```rust
frame.set_client_areas(vec![
    ClientArea::left(0.0, 0.0, 48.0, 48.0),
    ClientArea::right(138.0, 0.0, search_width, 48.0),
]);
```

Hit-testing priority: window resize edges → the three system buttons → custom `ClientArea`s → title-bar drag area.

## Options

`FrameOptions`:

| Field | Default | Description |
|---|---:|---|
| `titlebar_height` | `48.0` | Height of the custom-drawn title bar, in DIP |
| `caption_button_width` | `46.0` | Width of each system button, in DIP |
| `minimum_size` | `None` | Optional minimum outer window size |
| `rounded_corners` | `true` | Request Windows 11 DWM rounded corners |
| `client_areas` | empty | Interactive Slint regions inside the title bar |

All sizes are logical pixels. At 100%, 125%, 150%, 175% and 200% scaling they are converted on every hit test
using `GetDpiForWindow`; no screen resolution is hard-coded.

## Multiple windows

Create a separate controller for each window:

```rust
let main_frame = FrameController::attach(&main_window, FrameOptions::default())?;
let tool_frame = FrameController::attach(&tool_window, FrameOptions::default())?;
```

Internally each HWND gets its own `SetWindowSubclass` state; there is no global original window procedure.
Attaching the same window twice returns `FrameError::AlreadyAttached`.

## If you already use the rendering notifier

The regular `attach` takes over the window's Slint rendering notifier. If your application already uses it for
custom OpenGL/WGPU rendering, call this from your own `RenderingSetup` callback instead:

```rust
let controller = FrameController::attach_window(window, FrameOptions::default())?;
```

The HWND must already exist at that point, otherwise `FrameError::WindowHandleUnavailable` is returned.

## Detaching manually

```rust
frame.detach()?;
```

This safely removes the Win32 subclass, restores the original window style and triggers `SWP_FRAMECHANGED`.
Regular applications don't need to call it; cleanup happens automatically when the window receives
`WM_NCDESTROY`.

## Safety design

- The entire public API is safe Rust;
- controllers are intentionally `!Send + !Sync` and must be used on the Slint UI thread;
- uses `SetWindowSubclass` instead of overwriting and globally storing a `WNDPROC`;
- each HWND owns reference-counted state that is released when the window is destroyed;
- user event callbacks run inside `catch_unwind`, so panics never cross the Win32 FFI boundary;
- all unhandled messages are forwarded to `DefSubclassProc`;
- no `WS_POPUP`, and no emulation of the Windows window manager.

## Running the full example in this repository

```powershell
cargo run --manifest-path examples/demo/Cargo.toml
```

The example demonstrates the default buttons, hover state, a hamburger menu in the title bar, dragging,
resizing and Windows 11 Snap Layouts.

## FAQ

### The window cannot be resized

Check whether the Slint root window sets a fixed `width` / `height`. Use `preferred-width` / `preferred-height`
instead.

### Buttons in the title bar don't respond to clicks

Register the button's logical rectangle as a `ClientArea`. The system minimize/maximize/close buttons don't need
to be registered.

### The maximize button shows Snap Layouts, but clicking it does nothing

Don't add a `TouchArea` to the three system buttons in Slint. The crate handles them through non-client messages
and `WM_SYSCOMMAND`.

### Custom icons don't line up with where clicks land

Icons are only visuals inside the button and must not change the button container's width. Keep
`Win32TitleBar.button-width` equal to `FrameOptions.caption_button_width`.

## License

[MIT](LICENSE)
