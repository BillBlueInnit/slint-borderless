use std::{cell::RefCell, rc::Rc};

use slint::ComponentHandle;

use crate::{CaptionButton, ClientArea, EventHandler, FrameError, FrameOptions};

pub struct FrameInner {
    handler: RefCell<Option<EventHandler>>,
}

pub fn attach<C: ComponentHandle + 'static>(
    _component: &C,
    _options: FrameOptions,
) -> Result<Rc<FrameInner>, FrameError> {
    Err(FrameError::UnsupportedPlatform)
}

pub fn attach_window(
    _window: &slint::Window,
    _options: FrameOptions,
) -> Result<Rc<FrameInner>, FrameError> {
    Err(FrameError::UnsupportedPlatform)
}

impl FrameInner {
    pub fn set_event_handler(&self, handler: Option<EventHandler>) {
        *self.handler.borrow_mut() = handler;
    }

    pub fn set_client_areas(&self, _areas: Vec<ClientArea>) {}

    pub fn caption_hover(&self) -> Option<CaptionButton> {
        None
    }

    pub fn is_attached(&self) -> bool {
        false
    }

    pub fn last_error(&self) -> Option<FrameError> {
        Some(FrameError::UnsupportedPlatform)
    }

    pub fn detach(&self) -> Result<(), FrameError> {
        Err(FrameError::UnsupportedPlatform)
    }
}
