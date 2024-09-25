mod input;

use std::sync::Arc;

use input::Input;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::KeyCode,
    platform::windows::WindowAttributesExtWindows,
    window::Window,
};

pub struct App {
    input: Input,
    window: Option<Arc<Window>>,
    proxy: EventLoopProxy<AppEvent>,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    RequestExit,
}

impl App {
    pub fn new(event_loop: &EventLoop<AppEvent>) -> Self {
        Self {
            input: Input::new(),
            window: None,
            proxy: event_loop.create_proxy(),
        }
    }

    fn input(&mut self) {
        if self.released(KeyCode::Escape) {
            self.proxy.send_event(AppEvent::RequestExit).unwrap();
        }
    }

    fn render(&mut self) {}

    fn resize(&mut self, _width: u32, _height: u32) {}

    pub fn pressed(&self, code: KeyCode) -> bool {
        self.input.pressed(code)
    }

    pub fn held(&self, code: KeyCode) -> bool {
        self.input.held(code)
    }

    pub fn released(&self, code: KeyCode) -> bool {
        self.input.released(code)
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window = Some(Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_class_name("Cuber"))
                .unwrap(),
        ));
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::RequestExit => {
                let _ = self.window.take();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        self.input.update(&event);
        self.input();
        match event {
            WindowEvent::CloseRequested => {
                log::info!("Close requested");
                let _ = self.window.take();
            }
            WindowEvent::Destroyed => {
                log::info!("Destroyed");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                log::trace!("Redraw");
                if let Some(window) = self.window.clone() {
                    if window_id == window.id() {
                        self.render();
                        window.pre_present_notify();
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::Resized(size) => {
                self.resize(size.width, size.height);
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let event_looop = EventLoop::with_user_event().build().unwrap();
    event_looop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(&event_looop);
    event_looop.run_app(&mut app).unwrap();
}
