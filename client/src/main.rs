use std::{collections::HashMap, ops::Deref, sync::Arc, time};

use cgpu::RenderContext;
use game::input::Input;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::KeyCode,
    window::{Window, WindowAttributes, WindowId},
};

pub struct App {
    last_update: time::SystemTime,
    delta_time: time::Duration,
    input: game::input::Input,
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
            last_update: time::SystemTime::now(),
            delta_time: time::Duration::from_nanos(0),
            input: Input::new(),
            proxy: event_loop.create_proxy(),
            windows: HashMap::new(),
            renderers: HashMap::new(),
        }
    }

    fn handle_input(&mut self) {
        if self.released(KeyCode::Escape) {
            self.proxy.send_event(AppEvent::RequestExit).unwrap();
        }

        for (_, renderer) in self.renderers.iter_mut() {
            let renderer = Arc::get_mut(renderer).unwrap();
            renderer.update_camera_mouse(self.delta_time, &self.input);
        }
    }

    fn render(&mut self, window: &WindowId) {
        if let Some(renderer) = self.renderers.get_mut(window) {
            let renderer = Arc::get_mut(renderer).unwrap();
            renderer.update_camera_keyboard(self.delta_time, &self.input);
            renderer.update_uniforms();
            let _ = renderer.render();
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
        let mut renderer = pollster::block_on(RenderContext::new(window.clone()));
        log::info!("Renderer Created");
        renderer.compute_test();
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
        self.last_update = time::SystemTime::now();
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        match cause {
            StartCause::Poll
            | StartCause::WaitCancelled { .. }
            | StartCause::ResumeTimeReached { .. } => {
                let now = time::SystemTime::now();
                self.delta_time = now
                    .duration_since(self.last_update)
                    .unwrap_or(time::Duration::from_secs_f32(1.0 / 60.0));
                self.last_update = now;
                for window in self.windows.values() {
                    window.request_redraw(); // Request redraw for all windows
                }
            }
            _ => {}
        }
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

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.input.update(&event);
        self.handle_input();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
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
            WindowEvent::Focused(true) => {
                log::debug!("Focused: {:?}", window_id);
                if let Some(window) = self.windows.get(&window_id) {
                    let window = window.deref();

                    if window
                        .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                        .is_ok()
                    {
                        window.set_cursor_visible(false);
                    } else {
                        log::error!("Failed to grab: {:?}", window_id);
                    }
                }
            }
            WindowEvent::Focused(false) => {
                log::debug!("Unfocused {:?}", window_id);
                if let Some(window) = self.windows.get(&window_id) {
                    let window = window.deref();

                    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);

                    window.set_cursor_visible(true);
                }
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
