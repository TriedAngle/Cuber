use std::{collections::HashMap, sync::Arc, time};

use game::Input;
use render::RenderContext;
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::KeyCode,
    window::{Window, WindowAttributes, WindowId},
};

mod render;

pub struct ClientState {
    last_update: time::SystemTime,
    delta_time: time::Duration,
    gpu: Arc<cgpu::GPUContext>,
    input: Input,
    #[allow(unused)]
    proxy: EventLoopProxy<()>,
    windows: HashMap<WindowId, Arc<Window>>,
    renderes: HashMap<WindowId, RenderContext>,
    focused: Option<Arc<Window>>,
}

impl ClientState {
    pub fn new(el: &EventLoop<()>) -> Self {
        let gpu = cgpu::GPUContext::new().unwrap();

        Self {
            last_update: time::SystemTime::now(),
            delta_time: time::Duration::ZERO,
            gpu,
            input: Input::new(),
            proxy: el.create_proxy(),
            windows: HashMap::new(),
            renderes: HashMap::new(),
            focused: None,
        }
    }

    pub fn render(&mut self, id: WindowId) {
        let Some(window) = self.windows.get(&id).cloned() else {
            return;
        };

        if let Some(renderer) = self.renderes.get_mut(&id) {
            renderer.render();
        }

        window.request_redraw();
    }

    pub fn resize(&mut self, id: WindowId, size: PhysicalSize<u32>) {
        let _ = id;
        let _ = size;
    }

    pub fn reset_deltas(&mut self) {
        self.input.flush(self.delta_time);
    }

    pub fn device_events(&mut self, event: &DeviceEvent) {
        self.input.update(event);

        for (_id, renderer) in self.renderes.iter_mut() {
            renderer.egui.handle_device_events(event);
        }
    }

    pub fn window_events(&mut self, id: WindowId, event: &WindowEvent) {
        self.input.update_window(event);

        if let Some(renderer) = self.renderes.get_mut(&id) {
            renderer.egui.handle_window_events(&renderer.window, event);
        }

        if self.input.pressing(KeyCode::Escape) {
            if let Some(window) = self.focused.take() {
                let id = window.id();
                let _window = self.windows.remove(&id);
                let _render = self.renderes.remove(&id);
            }
        }
    }

    pub fn focus_window(&mut self, id: WindowId) {
        let window = self.windows.get(&id).unwrap().clone();
        self.focused = Some(window);
    }

    pub fn unfocus_window(&mut self, _id: WindowId) {
        self.focused = None
    }

    pub fn update_time(&mut self) {
        let now = time::SystemTime::now();
        self.delta_time = now
            .duration_since(self.last_update)
            .unwrap_or(time::Duration::from_secs_f64(1. / 60.));
        self.last_update = now;
        for (_id, renderer) in &mut self.renderes {
            renderer.update_delta_time(self.delta_time);
        }
    }

    pub fn create_window(
        &mut self,
        el: &ActiveEventLoop,
        title: &str,
        width: u32,
        height: u32,
    ) -> Arc<Window> {
        let attribs = WindowAttributes::default()
            .with_inner_size(PhysicalSize::new(width, height))
            .with_title(title);

        let window = match el.create_window(attribs) {
            Ok(window) => window,
            Err(e) => panic!("Error creating window: {:?}", e),
        };

        let id = window.id();
        log::info!("Window {:?} Created", id);

        let window = Arc::new(window);
        self.windows.insert(id, window.clone());
        window
    }

    pub fn create_renderer(&mut self, window: Arc<Window>) {
        let id = window.id();
        let renderer = RenderContext::new(self.gpu.clone(), window).unwrap();

        self.renderes.insert(id, renderer);
    }

    pub fn remove_window(&mut self, id: &WindowId) -> Arc<Window> {
        self.windows.remove(id).unwrap()
    }

    pub fn windows(&self) -> &HashMap<WindowId, Arc<Window>> {
        &self.windows
    }
}
