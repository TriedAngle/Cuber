use client::ClientState;
use winit::{
    application::ApplicationHandler,
    event::{StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::WindowId,
};

pub struct Application {
    state: ClientState,
}

impl Application {
    pub fn new(el: &EventLoop<()>) -> Self {
        Self {
            state: ClientState::new(el),
        }
    }
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        log::info!("Client Resumed");
        let window = self.state.create_window(event_loop, "Cuber", 1920, 1080);
        self.state.create_renderer(window);
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        match cause {
            StartCause::Poll
            | StartCause::WaitCancelled { .. }
            | StartCause::ResumeTimeReached { .. } => {
                self.state.update_time();
                self.state.reset_deltas();
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.state.device_events(&event);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        self.state.window_events(window_id, &event);
        match event {
            WindowEvent::CloseRequested => {
                log::info!("Close Requested Window: {:?}", window_id);
                let _window = self.state.remove_window(&window_id);
                if self.state.windows().is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::Destroyed => {
                log::info!("Destroyed Window: {:?}", window_id);
                if self.state.windows().is_empty() {
                    log::info!("No Windows left, exiting...");
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                log::trace!("Redraw Requested Window: {:?}", window_id);
                self.state.render(window_id);
            }
            WindowEvent::Resized(size) => {
                log::debug!("Resize Window: {:?}", window_id);
                self.state.resize(window_id, size);
            }
            WindowEvent::Focused(true) => {
                log::info!("Focus Window: {:?}", window_id);
                self.state.focus_window(window_id);
            }
            WindowEvent::Focused(false) => {
                log::info!("Unfocus Window: {:?}", window_id);
                self.state.unfocus_window(window_id);
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::builder()
        .filter_module("naga", log::LevelFilter::Warn)
        .init();

    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut application = Application::new(&event_loop);
    event_loop.run_app(&mut application).unwrap();

    log::info!("Destroying Client");
}
