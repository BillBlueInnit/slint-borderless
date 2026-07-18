//! Windows-native borderless frame support for Slint.
//!
//! This crate removes the visible Win32 non-client frame without turning the
//! window into `WS_POPUP`. Windows therefore remains responsible for resize,
//! minimize/maximize animations, Aero Snap, `Win + Arrow`, the system menu,
//! task switching, and the Windows 11 Snap Layout flyout.
//!
//! The normal entry point is [`FrameController::attach`].

use std::{error::Error, fmt, rc::Rc};

use slint::ComponentHandle;

#[cfg(not(target_os = "windows"))]
mod unsupported;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(target_os = "windows"))]
use unsupported as platform;
#[cfg(target_os = "windows")]
use windows as platform;

/// Which system caption button is under the pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptionButton {
    Minimize,
    Maximize,
    Close,
}

impl CaptionButton {
    /// Value expected by the bundled `Win32TitleBar.hover-button` property.
    pub const fn slint_value(self) -> i32 {
        match self {
            Self::Minimize => 1,
            Self::Maximize => 2,
            Self::Close => 3,
        }
    }
}

/// Notifications emitted by the native frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameEvent {
    Installed,
    CaptionHoverChanged(Option<CaptionButton>),
    Detached,
    Failed(FrameError),
}

/// A logical point in Slint device-independent pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalPoint {
    pub x: f32,
    pub y: f32,
}

/// A logical size in Slint device-independent pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalSize {
    pub width: f32,
    pub height: f32,
}

/// The edge from which a title-bar client area is positioned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HorizontalAnchor {
    Left,
    Right,
}

/// A rectangle in the custom title bar that remains a Slint client area.
///
/// Use this for menus, search fields, tabs, or arbitrary buttons placed in the
/// title bar. Outside these areas, the title bar drags the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClientArea {
    pub anchor: HorizontalAnchor,
    pub origin: LogicalPoint,
    pub size: LogicalSize,
}

impl ClientArea {
    pub const fn left(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            anchor: HorizontalAnchor::Left,
            origin: LogicalPoint { x, y },
            size: LogicalSize { width, height },
        }
    }

    pub const fn right(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            anchor: HorizontalAnchor::Right,
            origin: LogicalPoint { x, y },
            size: LogicalSize { width, height },
        }
    }
}

/// Geometry and behavior of the native borderless frame.
#[derive(Clone, Debug)]
pub struct FrameOptions {
    /// Height of the Slint-drawn title bar in logical pixels.
    pub titlebar_height: f32,
    /// Width of each of the three system caption buttons in logical pixels.
    pub caption_button_width: f32,
    /// Minimum outer window size in logical pixels. `None` keeps Slint's limit.
    pub minimum_size: Option<LogicalSize>,
    /// Ask DWM for Windows 11 rounded corners. Ignored on older Windows.
    pub rounded_corners: bool,
    /// Interactive Slint controls located inside the title bar.
    pub client_areas: Vec<ClientArea>,
}

impl Default for FrameOptions {
    fn default() -> Self {
        Self {
            titlebar_height: 48.0,
            caption_button_width: 46.0,
            minimum_size: None,
            rounded_corners: true,
            client_areas: Vec::new(),
        }
    }
}

/// Errors returned while attaching or detaching a frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameError {
    UnsupportedPlatform,
    RenderingNotifierUnavailable(String),
    WindowHandleUnavailable(String),
    NotAWin32Window,
    InvalidOptions,
    AlreadyAttached,
    NativeSubclassFailed,
    NativeDetachFailed,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => f.write_str("slint-borderless only supports Windows"),
            Self::RenderingNotifierUnavailable(error) => {
                write!(f, "Slint rendering notifier is unavailable: {error}")
            }
            Self::WindowHandleUnavailable(error) => {
                write!(f, "native window handle is unavailable: {error}")
            }
            Self::NotAWin32Window => f.write_str("Slint did not create a Win32 window"),
            Self::InvalidOptions => f.write_str("frame dimensions must be finite and positive"),
            Self::AlreadyAttached => f.write_str("a frame is already attached"),
            Self::NativeSubclassFailed => f.write_str("SetWindowSubclass failed"),
            Self::NativeDetachFailed => f.write_str("RemoveWindowSubclass failed"),
        }
    }
}

impl Error for FrameError {}

/// A UI-thread controller for one Slint window.
///
/// The controller intentionally is not `Send` or `Sync`: Slint windows and
/// Win32 subclass operations belong to the UI thread. Cloning it is cheap.
#[derive(Clone)]
pub struct FrameController {
    inner: Rc<platform::FrameInner>,
}

impl FrameController {
    /// Attach automatically during Slint's first `RenderingSetup` callback.
    ///
    /// This is the one-line API for normal applications. It occupies Slint's
    /// rendering notifier for this window; use [`Self::attach_window`] if the
    /// application already owns that notifier.
    pub fn attach<C: ComponentHandle + 'static>(
        component: &C,
        options: FrameOptions,
    ) -> Result<Self, FrameError> {
        platform::attach(component, options).map(|inner| Self { inner })
    }

    /// Attach immediately to an already-created Slint window.
    ///
    /// Call this from the application's own `RenderingSetup` notifier when it
    /// needs to keep using that notifier for custom rendering.
    pub fn attach_window(
        window: &slint::Window,
        options: FrameOptions,
    ) -> Result<Self, FrameError> {
        platform::attach_window(window, options).map(|inner| Self { inner })
    }

    /// Receive hover/install/detach events. Replaces the previous handler.
    pub fn on_event(&self, handler: impl Fn(FrameEvent) + 'static) {
        self.inner.set_event_handler(Some(Rc::new(handler)));
    }

    /// Remove the event handler.
    pub fn clear_event_handler(&self) {
        self.inner.set_event_handler(None);
    }

    /// Replace interactive Slint areas in the title bar.
    pub fn set_client_areas(&self, areas: impl Into<Vec<ClientArea>>) {
        self.inner.set_client_areas(areas.into());
    }

    /// Current caption-button hover state.
    pub fn caption_hover(&self) -> Option<CaptionButton> {
        self.inner.caption_hover()
    }

    /// Whether the Win32 frame is currently installed.
    pub fn is_attached(&self) -> bool {
        self.inner.is_attached()
    }

    /// Most recent asynchronous installation error, if any.
    pub fn last_error(&self) -> Option<FrameError> {
        self.inner.last_error()
    }

    /// Restore the normal Windows non-client frame and remove the subclass.
    pub fn detach(&self) -> Result<(), FrameError> {
        self.inner.detach()
    }
}

pub(crate) type EventHandler = Rc<dyn Fn(FrameEvent)>;

/// Name used with Slint's `@library` import syntax.
pub const SLINT_LIBRARY_NAME: &str = "slint-borderless";

/// Filesystem location of the bundled Slint component library.
///
/// Call this from `build.rs` and pass it to
/// `slint_build::CompilerConfiguration::with_library_paths`.
pub fn slint_library_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui")
}
