use slint::ComponentHandle;
use slint_borderless::{
    ClientArea, FrameController, FrameEvent, FrameOptions, LogicalSize,
};

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = AppWindow::new()?;

    let frame = FrameController::attach(
        &app,
        FrameOptions {
            minimum_size: Some(LogicalSize {
                width: 640.0,
                height: 480.0,
            }),
            // The hamburger button is a real Slint client control inside the
            // otherwise draggable title bar.
            client_areas: vec![ClientArea::left(0.0, 0.0, 48.0, 48.0)],
            ..Default::default()
        },
    )?;

    let weak = app.as_weak();
    frame.on_event(move |event| {
        let Some(app) = weak.upgrade() else { return };
        match event {
            FrameEvent::CaptionHoverChanged(button) => {
                app.set_caption_hover(button.map_or(0, |button| button.slint_value()));
            }
            FrameEvent::Failed(error) => eprintln!("native frame failed: {error}"),
            FrameEvent::Installed | FrameEvent::Detached => {}
        }
    });

    app.run()?;
    Ok(())
}
