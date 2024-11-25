mod input;

use std::{collections::HashMap, sync::Arc};

use cgpu::RenderContext;
use input::Input;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::KeyCode,
    window::{Window, WindowAttributes, WindowId},
};

pub struct App {
    input: Input,
    proxy: EventLoopProxy<AppEvent>,
    windows: HashMap<WindowId, Arc<Window>>,
    renderers: HashMap<WindowId, Arc<RenderContext>>,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    RequestExit,
}

impl App {
    pub fn new(event_loop: &EventLoop<AppEvent>) -> Self {
        Self {
            input: Input::new(),
            proxy: event_loop.create_proxy(),
            windows: HashMap::new(),
            renderers: HashMap::new(),
        }
    }

    fn input(&mut self) {
        if self.released(KeyCode::Escape) {
            self.proxy.send_event(AppEvent::RequestExit).unwrap();
        }
    }

    fn render(&mut self, window: &WindowId) {
        if let Some(render) = self.renderers.get_mut(window) {
            let render = Arc::get_mut(render).unwrap();
            let _ = render.render();
        }
    }

    fn resize(&mut self, window: &WindowId, size: PhysicalSize<u32>) {
        if let Some(render) = self.renderers.get_mut(window) {
            let render = Arc::get_mut(render).unwrap();
            render.resize(size);
        }
    }

    pub fn pressed(&self, code: KeyCode) -> bool {
        self.input.pressed(code)
    }

    pub fn held(&self, code: KeyCode) -> bool {
        self.input.held(code)
    }

    pub fn released(&self, code: KeyCode) -> bool {
        self.input.released(code)
    }

    pub fn new_window(&mut self, event_loop: &ActiveEventLoop, title: &str) -> Arc<Window> {
        let attribs = WindowAttributes::default().with_title(title);
        let window = match event_loop.create_window(attribs) {
            Ok(window) => window,
            Err(e) => panic!("Error creating window: {:?}", e),
        };

        log::info!("Window created");

        let id = window.id();
        let window = Arc::new(window);
        self.windows.insert(id, window.clone());
        window
    }

    pub fn new_renderer(&mut self, window: Arc<Window>) {
        let renderer = pollster::block_on(RenderContext::new(window.clone()));
        log::info!("Renderer Created");
        let renderer = Arc::new(renderer);
        self.renderers.insert(window.id(), renderer);
    }

    pub fn new_render_window(&mut self, event_loop: &ActiveEventLoop, title: &str) {
        let window = self.new_window(event_loop, title);
        self.new_renderer(window);
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        log::info!("Window Resumed");
        self.new_render_window(event_loop, "Cuber");
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::RequestExit => {
                let _windows = self.windows.drain();
                let _renderers = self.renderers.drain();
                event_loop.exit();
                log::info!("AppEvent: RequestExit");
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        self.input.update(&event);
        self.input();
        match event {
            WindowEvent::CloseRequested => {
                log::info!("Close requested");
                let _window = self.windows.remove(&window_id);
                let _renderer = self.renderers.remove(&window_id);
            }
            WindowEvent::Destroyed => {
                log::info!("Destroyed {:?}", window_id);
            }
            WindowEvent::RedrawRequested => {
                log::trace!("Redraw requested {:?}", window_id);
                self.render(&window_id);
            }
            WindowEvent::Resized(size) => {
                self.resize(&window_id, size);
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::builder().init();

    let event_loop = EventLoop::with_user_event().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(&event_loop);
    event_loop.run_app(&mut app).unwrap();
}
