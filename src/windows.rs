#![allow(unsafe_op_in_unsafe_fn)]

use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::ComponentHandle;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::{
        Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DwmSetWindowAttribute},
        Gdi::ScreenToClient,
    },
    UI::{
        HiDpi::{EnableNonClientDpiScaling, GetDpiForWindow, GetSystemMetricsForDpi},
        Input::KeyboardAndMouse::{TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent},
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            GWL_STYLE, GetClientRect, GetPropW, GetWindowLongPtrW, GetWindowRect, HTBOTTOM,
            HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTCLOSE, HTLEFT, HTMAXBUTTON,
            HTMINBUTTON, HTNOWHERE, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, IsZoomed, MINMAXINFO,
            NCCALCSIZE_PARAMS, RemovePropW, SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE, SC_RESTORE,
            SM_CXPADDEDBORDER, SM_CXSIZEFRAME, SM_CYSIZEFRAME, SWP_FRAMECHANGED, SWP_NOMOVE,
            SWP_NOSIZE, SWP_NOZORDER, SetPropW, SetWindowLongPtrW, SetWindowPos, WM_GETMINMAXINFO,
            WM_NCCALCSIZE, WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP,
            WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_SYSCOMMAND, WS_CAPTION, WS_MAXIMIZEBOX,
            WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
        },
    },
};

use crate::{
    CaptionButton, ClientArea, EventHandler, FrameError, FrameEvent, FrameOptions, HorizontalAnchor,
};

const SUBCLASS_ID: usize = 0x534C_4652; // "SLFR"
const DWMWCP_ROUND: u32 = 2;
const FRAME_PROPERTY: &[u16] = &[
    's' as u16, 'l' as u16, 'i' as u16, 'n' as u16, 't' as u16, '-' as u16, 'b' as u16, 'o' as u16,
    'r' as u16, 'd' as u16, 'e' as u16, 'r' as u16, 'l' as u16, 'e' as u16, 's' as u16, 's' as u16,
    0,
];

pub struct FrameInner {
    hwnd: Cell<HWND>,
    attached: Cell<bool>,
    original_style: Cell<u32>,
    options: FrameOptions,
    client_areas: RefCell<Vec<ClientArea>>,
    hover: Cell<Option<CaptionButton>>,
    pressed: Cell<Option<CaptionButton>>,
    handler: RefCell<Option<EventHandler>>,
    last_error: RefCell<Option<FrameError>>,
}

impl FrameInner {
    fn new(options: FrameOptions) -> Rc<Self> {
        Rc::new(Self {
            hwnd: Cell::new(std::ptr::null_mut()),
            attached: Cell::new(false),
            original_style: Cell::new(0),
            client_areas: RefCell::new(options.client_areas.clone()),
            options,
            hover: Cell::new(None),
            pressed: Cell::new(None),
            handler: RefCell::new(None),
            last_error: RefCell::new(None),
        })
    }

    pub fn set_event_handler(&self, handler: Option<EventHandler>) {
        *self.handler.borrow_mut() = handler;
    }

    pub fn set_client_areas(&self, areas: Vec<ClientArea>) {
        *self.client_areas.borrow_mut() = areas;
    }

    pub fn caption_hover(&self) -> Option<CaptionButton> {
        self.hover.get()
    }

    pub fn is_attached(&self) -> bool {
        self.attached.get()
    }

    pub fn last_error(&self) -> Option<FrameError> {
        self.last_error.borrow().clone()
    }

    pub fn detach(&self) -> Result<(), FrameError> {
        let hwnd = self.hwnd.get();
        if hwnd.is_null() || !self.attached.get() {
            return Ok(());
        }

        unsafe {
            let ref_data = GetPropW(hwnd, FRAME_PROPERTY.as_ptr());
            if ref_data.is_null()
                || RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID) == 0
            {
                return Err(FrameError::NativeDetachFailed);
            }
            RemovePropW(hwnd, FRAME_PROPERTY.as_ptr());

            SetWindowLongPtrW(hwnd, GWL_STYLE, self.original_style.get() as _);
            self.hwnd.set(std::ptr::null_mut());
            self.attached.set(false);
            self.pressed.set(None);
            self.set_hover(None);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
            self.emit(FrameEvent::Detached);
            drop(Rc::from_raw(ref_data as *const FrameInner));
        }
        Ok(())
    }

    fn set_hover(&self, hover: Option<CaptionButton>) {
        if self.hover.replace(hover) != hover {
            self.emit(FrameEvent::CaptionHoverChanged(hover));
        }
    }

    fn emit(&self, event: FrameEvent) {
        let handler = self.handler.borrow().clone();
        if let Some(handler) = handler {
            // Never unwind through a Win32 callback boundary.
            let _ = catch_unwind(AssertUnwindSafe(|| handler(event)));
        }
    }

    fn fail(&self, error: FrameError) {
        *self.last_error.borrow_mut() = Some(error.clone());
        self.emit(FrameEvent::Failed(error));
    }

    fn contains_client_area(&self, x: f32, y: f32, client_width: f32) -> bool {
        self.client_areas.borrow().iter().any(|area| {
            let left = match area.anchor {
                HorizontalAnchor::Left => area.origin.x,
                HorizontalAnchor::Right => client_width - area.origin.x - area.size.width,
            };
            x >= left
                && x < left + area.size.width
                && y >= area.origin.y
                && y < area.origin.y + area.size.height
        })
    }
}

pub fn attach<C: ComponentHandle + 'static>(
    component: &C,
    options: FrameOptions,
) -> Result<Rc<FrameInner>, FrameError> {
    validate_options(&options)?;
    let inner = FrameInner::new(options);
    let weak_component = component.as_weak();
    let setup_inner = inner.clone();

    component
        .window()
        .set_rendering_notifier(move |state, _graphics_api| {
            if !matches!(state, slint::RenderingState::RenderingSetup) || setup_inner.is_attached()
            {
                return;
            }
            let Some(component) = weak_component.upgrade() else {
                return;
            };
            if let Err(error) = install_window(component.window(), &setup_inner) {
                setup_inner.fail(error);
            }
        })
        .map_err(|error| FrameError::RenderingNotifierUnavailable(error.to_string()))?;

    Ok(inner)
}

pub fn attach_window(
    window: &slint::Window,
    options: FrameOptions,
) -> Result<Rc<FrameInner>, FrameError> {
    validate_options(&options)?;
    let inner = FrameInner::new(options);
    install_window(window, &inner)?;
    Ok(inner)
}

fn validate_options(options: &FrameOptions) -> Result<(), FrameError> {
    if !options.titlebar_height.is_finite()
        || options.titlebar_height <= 0.0
        || !options.caption_button_width.is_finite()
        || options.caption_button_width <= 0.0
    {
        return Err(FrameError::InvalidOptions);
    }
    Ok(())
}

fn install_window(window: &slint::Window, inner: &Rc<FrameInner>) -> Result<(), FrameError> {
    if inner.is_attached() {
        return Err(FrameError::AlreadyAttached);
    }

    let persistent = window.window_handle();
    let borrowed = persistent
        .window_handle()
        .map_err(|error| FrameError::WindowHandleUnavailable(error.to_string()))?;
    let RawWindowHandle::Win32(handle) = borrowed.as_raw() else {
        return Err(FrameError::NotAWin32Window);
    };
    let hwnd = handle.hwnd.get() as HWND;

    unsafe {
        if !GetPropW(hwnd, FRAME_PROPERTY.as_ptr()).is_null() {
            return Err(FrameError::AlreadyAttached);
        }

        let original_style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let required_style = original_style
            | WS_CAPTION
            | WS_THICKFRAME
            | WS_SYSMENU
            | WS_MINIMIZEBOX
            | WS_MAXIMIZEBOX;
        // `as _`: on 32-bit targets windows-sys aliases these to the `i32` variants.
        SetWindowLongPtrW(hwnd, GWL_STYLE, required_style as _);
        let _ = EnableNonClientDpiScaling(hwnd);

        let raw_inner = Rc::into_raw(inner.clone());
        if SetPropW(hwnd, FRAME_PROPERTY.as_ptr(), raw_inner as *mut _) == 0
            || SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, raw_inner as usize) == 0
        {
            RemovePropW(hwnd, FRAME_PROPERTY.as_ptr());
            drop(Rc::from_raw(raw_inner));
            SetWindowLongPtrW(hwnd, GWL_STYLE, original_style as _);
            return Err(FrameError::NativeSubclassFailed);
        }

        inner.original_style.set(original_style);
        inner.hwnd.set(hwnd);
        inner.attached.set(true);

        if inner.options.rounded_corners {
            let preference = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                (&preference as *const u32).cast(),
                size_of::<u32>() as u32,
            );
        }

        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
        inner.emit(FrameEvent::Installed);
    }
    Ok(())
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    ref_data: usize,
) -> LRESULT {
    let raw_inner = ref_data as *const FrameInner;
    let result = catch_unwind(AssertUnwindSafe(|| {
        handle_message(&*raw_inner, hwnd, message, wparam, lparam)
    }));
    let result = match result {
        Ok(value) => value,
        Err(_) => DefSubclassProc(hwnd, message, wparam, lparam),
    };
    if message == WM_NCDESTROY {
        // Release the subclass' strong reference only after every borrow of it has ended.
        drop(Rc::from_raw(raw_inner));
    }
    result
}

unsafe fn handle_message(
    inner: &FrameInner,
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_GETMINMAXINFO => {
            if let Some(minimum) = inner.options.minimum_size {
                let dpi = GetDpiForWindow(hwnd).max(96);
                let info = &mut *(lparam as *mut MINMAXINFO);
                info.ptMinTrackSize.x = dip_to_px(minimum.width, dpi);
                info.ptMinTrackSize.y = dip_to_px(minimum.height, dpi);
                return 0;
            }
        }
        WM_NCCALCSIZE if wparam != 0 => {
            if IsZoomed(hwnd) != 0 {
                let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);
                let dpi = GetDpiForWindow(hwnd).max(96);
                let x = GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi)
                    + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
                let y = GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi)
                    + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
                params.rgrc[0].left += x;
                params.rgrc[0].right -= x;
                params.rgrc[0].top += y;
                params.rgrc[0].bottom -= y;
            }
            return 0;
        }
        WM_NCHITTEST => {
            let hit = hit_test(inner, hwnd, lparam);
            if hit != HTNOWHERE {
                return hit as LRESULT;
            }
        }
        WM_NCMOUSEMOVE => {
            inner.set_hover(button_from_hit(wparam as u32));
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE | TME_NONCLIENT,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tracking);
        }
        WM_NCMOUSELEAVE => {
            inner.pressed.set(None);
            inner.set_hover(None);
        }
        WM_NCLBUTTONDOWN => {
            let pressed = button_from_hit(wparam as u32);
            inner.pressed.set(pressed);
            if pressed.is_some() {
                return 0;
            }
        }
        WM_NCLBUTTONUP => {
            // Like native caption buttons, only act when press and release hit the same button.
            if let Some(button) = button_from_hit(wparam as u32)
                && inner.pressed.take() == Some(button)
            {
                let command = match button {
                    CaptionButton::Minimize => SC_MINIMIZE,
                    CaptionButton::Maximize if IsZoomed(hwnd) != 0 => SC_RESTORE,
                    CaptionButton::Maximize => SC_MAXIMIZE,
                    CaptionButton::Close => SC_CLOSE,
                };
                return DefSubclassProc(hwnd, WM_SYSCOMMAND, command as WPARAM, 0);
            }
        }
        WM_NCDESTROY => {
            // `subclass_proc` releases the subclass' reference after this returns.
            RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
            RemovePropW(hwnd, FRAME_PROPERTY.as_ptr());
            inner.hwnd.set(std::ptr::null_mut());
            inner.attached.set(false);
            inner.pressed.set(None);
            inner.set_hover(None);
            inner.emit(FrameEvent::Detached);
        }
        _ => {}
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

unsafe fn hit_test(inner: &FrameInner, hwnd: HWND, lparam: LPARAM) -> u32 {
    let screen_x = (lparam as i16) as i32;
    let screen_y = ((lparam >> 16) as i16) as i32;
    let dpi = GetDpiForWindow(hwnd).max(96);

    if IsZoomed(hwnd) == 0 {
        let mut window = std::mem::zeroed();
        GetWindowRect(hwnd, &mut window);
        let border_x = GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi)
            + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
        let border_y = GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi)
            + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
        let left = screen_x < window.left + border_x;
        let right = screen_x >= window.right - border_x;
        let top = screen_y < window.top + border_y;
        let bottom = screen_y >= window.bottom - border_y;
        let resize_hit = match (left, right, top, bottom) {
            (true, _, true, _) => HTTOPLEFT,
            (_, true, true, _) => HTTOPRIGHT,
            (true, _, _, true) => HTBOTTOMLEFT,
            (_, true, _, true) => HTBOTTOMRIGHT,
            (true, _, _, _) => HTLEFT,
            (_, true, _, _) => HTRIGHT,
            (_, _, true, _) => HTTOP,
            (_, _, _, true) => HTBOTTOM,
            _ => HTNOWHERE,
        };
        if resize_hit != HTNOWHERE {
            return resize_hit;
        }
    }

    let mut point = POINT {
        x: screen_x,
        y: screen_y,
    };
    ScreenToClient(hwnd, &mut point);
    let mut client = std::mem::zeroed();
    GetClientRect(hwnd, &mut client);
    let x = px_to_dip(point.x, dpi);
    let y = px_to_dip(point.y, dpi);
    let client_width = px_to_dip(client.right, dpi);

    if y < 0.0 || y >= inner.options.titlebar_height {
        return HTNOWHERE;
    }

    if let Some(hit) = caption_button_hit(client_width - x, inner.options.caption_button_width) {
        hit
    } else if inner.contains_client_area(x, y, client_width) {
        HTCLIENT
    } else {
        HTCAPTION
    }
}

/// Hit-test the three caption buttons by distance from the client's right edge.
fn caption_button_hit(from_right: f32, button_width: f32) -> Option<u32> {
    if from_right <= 0.0 {
        None
    } else if from_right <= button_width {
        Some(HTCLOSE)
    } else if from_right <= button_width * 2.0 {
        Some(HTMAXBUTTON)
    } else if from_right <= button_width * 3.0 {
        Some(HTMINBUTTON)
    } else {
        None
    }
}

fn button_from_hit(hit: u32) -> Option<CaptionButton> {
    match hit {
        HTMINBUTTON => Some(CaptionButton::Minimize),
        HTMAXBUTTON => Some(CaptionButton::Maximize),
        HTCLOSE => Some(CaptionButton::Close),
        _ => None,
    }
}

fn dip_to_px(value: f32, dpi: u32) -> i32 {
    (value * dpi as f32 / 96.0).round() as i32
}

fn px_to_dip(value: i32, dpi: u32) -> f32 {
    value as f32 * 96.0 / dpi as f32
}

#[cfg(test)]
mod tests {
    use super::{caption_button_hit, dip_to_px, px_to_dip};
    use windows_sys::Win32::UI::WindowsAndMessaging::{HTCLOSE, HTMAXBUTTON, HTMINBUTTON};

    #[test]
    fn caption_buttons_are_laid_out_from_the_right_edge() {
        assert_eq!(caption_button_hit(0.0, 46.0), None);
        assert_eq!(caption_button_hit(1.0, 46.0), Some(HTCLOSE));
        assert_eq!(caption_button_hit(46.0, 46.0), Some(HTCLOSE));
        assert_eq!(caption_button_hit(47.0, 46.0), Some(HTMAXBUTTON));
        assert_eq!(caption_button_hit(138.0, 46.0), Some(HTMINBUTTON));
        assert_eq!(caption_button_hit(139.0, 46.0), None);
    }

    #[test]
    fn dpi_conversion_covers_common_windows_scales() {
        assert_eq!(dip_to_px(46.0, 96), 46);
        assert_eq!(dip_to_px(46.0, 120), 58);
        assert_eq!(dip_to_px(46.0, 144), 69);
        assert_eq!(dip_to_px(46.0, 168), 81);
        assert_eq!(dip_to_px(46.0, 192), 92);
        assert!((px_to_dip(69, 144) - 46.0).abs() < f32::EPSILON);
    }
}
