extern crate nalgebra as na;

use std::{collections::HashMap, sync::Arc, time};

use game::{Camera, Input};
use parking_lot::Mutex;
use render::RenderContext;
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::KeyCode,
    window::{Window, WindowAttributes, WindowId},
};

mod render;
mod ui;

pub struct TimeTicker {
    last: time::SystemTime,
    accumulator: time::Duration,
    rate: time::Duration,
}

impl TimeTicker {
    pub fn new(rate: time::Duration) -> Self {
        Self {
            last: time::SystemTime::now(),
            accumulator: time::Duration::ZERO,
            rate,
        }
    }

    pub fn update(&mut self) -> time::Duration {
        let now = time::SystemTime::now();
        let mut ticked_time = match now.duration_since(self.last) {
            Ok(ticked) => ticked,
            Err(e) => {
                log::warn!("{:?}", e);
                return time::Duration::ZERO;
            }
        };
        self.last = now;

        if ticked_time > time::Duration::from_millis(250) {
            log::warn!("Frame time exploding");
            ticked_time = time::Duration::from_millis(250);
        }

        self.accumulator += ticked_time;
        ticked_time
    }
}

pub struct ClientState {
    ticker: TimeTicker,
    render_ticker: TimeTicker,
    gpu: Arc<cgpu::GPUContext>,
    input: Input,
    #[allow(unused)]
    proxy: EventLoopProxy<()>,
    windows: HashMap<WindowId, Arc<Window>>,
    renderes: HashMap<WindowId, Mutex<RenderContext>>,
    capture: bool,
    focused: Option<Arc<Window>>,
    camera: Camera,
}

impl ClientState {
    pub fn new(el: &EventLoop<()>) -> Self {
        let gpu = cgpu::GPUContext::new().unwrap();

        let camera = Camera::new(
            na::Point3::new(0.0, 0.0, 0.0),
            na::UnitQuaternion::identity(),
            50.0,
            5.,
            45.0,
            16.0 / 9.0,
            0.1,
            100.0,
        );

        Self {
            ticker: TimeTicker::new(time::Duration::from_secs_f64(1.0 / 60.0)),
            render_ticker: TimeTicker::new(time::Duration::from_secs_f64(1.0 / 166.0)),
            gpu,
            input: Input::new(),
            proxy: el.create_proxy(),
            windows: HashMap::new(),
            renderes: HashMap::new(),
            focused: None,
            capture: true,
            camera,
        }
    }

    pub fn resize(&mut self, id: WindowId, size: PhysicalSize<u32>) {
        let _ = id;
        let _ = size;
    }

    pub fn handle_input(&mut self, dt: time::Duration) {
        let dtf32 = dt.as_secs_f32();
        for (_id, render) in &self.renderes {
            self.camera.update_mouse(dtf32, &self.input);
            self.camera.update_keyboard(dtf32, &self.input);
            let mut render = render.lock();

            render.update_camera(&self.camera);
            if self.input.pressed(KeyCode::KeyM) {
                render.ppc.mode += 1;
                if render.ppc.mode == 4 {
                    render.ppc.mode = 0;
                }
            }
        }
    }

    pub fn fixed_tick(&mut self) {
        let dt = self.ticker.update();

        while self.ticker.accumulator >= self.ticker.rate {
            self.ticker.accumulator -= self.ticker.rate;
        }

        self.handle_input(dt);
        self.input.flush(self.ticker.rate);
    }

    pub fn fixed_render_tick(&mut self, window: WindowId) {
        if let Some(render) = self.renderes.get(&window) {
            let mut render = render.lock();
            let dt = self.render_ticker.update();
            let _dtf32 = dt.as_secs_f32();
            while self.render_ticker.accumulator >= self.render_ticker.rate {
                let _alpha = (self.ticker.accumulator.as_secs_f32()
                    / self.ticker.rate.as_secs_f32())
                .clamp(0.0, 1.0);
                render.update_delta_time(self.render_ticker.rate);

                self.render_ui(&mut render);
                render.render();
                self.render_ticker.accumulator -= self.render_ticker.rate;
            }
        } else {
            log::error!("Renderer does not exist for {:?}", window);
            return;
        }

        if let Some(window) = self.windows.get(&window) {
            window.request_redraw();
        }
    }

    pub fn device_events(&mut self, event: &DeviceEvent) {
        self.input.update(event);

        for (_id, render) in self.renderes.iter() {
            let mut render = render.lock();
            render.egui.handle_device_events(event);
        }
    }

    pub fn window_events(&mut self, id: WindowId, event: &WindowEvent) {
        self.input.update_window(event);

        if let Some(render) = self.renderes.get(&id) {
            let mut render = render.lock();
            render.egui_handle_window_events(event);
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
        if self.capture {
            if window
                .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                .is_ok()
            {
                window.set_cursor_visible(false);
            } else {
                log::error!("Failed to grab cursor {:?}", window.id())
            }
        }
        self.focused = Some(window);
    }

    pub fn unfocus_window(&mut self, _id: WindowId) {
        self.focused = None
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

        self.renderes.insert(id, Mutex::new(renderer));
    }

    pub fn remove_window(&mut self, id: &WindowId) -> Arc<Window> {
        self.windows.remove(id).unwrap()
    }

    pub fn windows(&self) -> &HashMap<WindowId, Arc<Window>> {
        &self.windows
    }
}
