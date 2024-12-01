extern crate nalgebra as na;

use std::{
    collections::HashMap,
    ops::Deref,
    sync::Arc,
    time,
};

use cgpu::RenderContext;
use egui::{DragValue, Vec2};
use egui_integration::EguiRenderer;
use game::input::Input;
use log::LevelFilter;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::KeyCode,
    window::{Window, WindowAttributes, WindowId},
};

mod egui_integration;

pub struct App {
    last_update: time::SystemTime,
    delta_time: time::Duration,
    frame_time: time::Duration,
    input: game::input::Input,
    proxy: EventLoopProxy<AppEvent>,
    windows: HashMap<WindowId, Arc<Window>>,
    renderers: HashMap<WindowId, Arc<RenderContext>>,
    eguis: HashMap<WindowId, Arc<EguiRenderer>>,
    scale_factor: f32,
    focus: bool,
    active_window: Option<Arc<Window>>,
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
            frame_time: time::Duration::from_nanos(0),
            input: Input::new(),
            proxy: event_loop.create_proxy(),
            windows: HashMap::new(),
            renderers: HashMap::new(),
            eguis: HashMap::new(),
            scale_factor: 1.0,
            focus: true,
            active_window: None,
        }
    }

    fn handle_input(&mut self) {
        if self.released(KeyCode::Escape) {
            self.proxy.send_event(AppEvent::RequestExit).unwrap();
        }

        if self.pressed(KeyCode::KeyT) {
            self.focus = !self.focus;
            if let Some(window) = &self.active_window {
                if self.focus {
                    if window
                        .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                        .is_ok()
                    {
                        window.set_cursor_visible(false);
                    } else {
                        log::error!("Failed to grab: {:?}", window.id());
                    }
                } else {
                    if window
                        .set_cursor_grab(winit::window::CursorGrabMode::None)
                        .is_ok()
                    {
                        window.set_cursor_visible(true);
                    } else {
                        log::error!("Failed to ungrab: {:?}", window.id());
                    }
                }
            }
        }

        for (_, renderer) in self.renderers.iter_mut() {
            let renderer = Arc::get_mut(renderer).unwrap();
            if self.focus {
                renderer.update_camera_mouse(self.delta_time, &self.input);
            }
        }
    }

    fn render(&mut self, window: &WindowId) {
        let start = time::SystemTime::now();

        if let Some(renderer) = self.renderers.get_mut(window) {
            let renderer = Arc::get_mut(renderer).unwrap();
            if self.focus {
                renderer.update_camera_keyboard(self.delta_time, &self.input);
            }
            renderer.update_uniforms();
            let _ = renderer.prepare_render();
            renderer.render();
        }

        self.draw_egui(window);

        if let Some(renderer) = self.renderers.get_mut(window) {
            let renderer = Arc::get_mut(renderer).unwrap();
            renderer.finish_render();
        }

        self.frame_time = start.elapsed().unwrap();
    }

    fn draw_egui(&mut self, window: &WindowId) {
        let Some(renderer) = self.renderers.get_mut(window) else {
            return;
        };
        if let Some(egui_renderer) = self.eguis.get_mut(window) {
            let renderer = Arc::get_mut(renderer).unwrap();
            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [
                    renderer.surface_config.width,
                    renderer.surface_config.height,
                ],
                pixels_per_point: egui_renderer.window.scale_factor() as f32 * self.scale_factor,
            };

            let mut encoder = renderer.encoder.as_mut().expect("Render must be prepared");
            let output = renderer
                .output_texture
                .as_ref()
                .expect("Render must be prepared");

            let view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let egui_renderer = Arc::get_mut(egui_renderer).unwrap();
            egui_renderer.begin_frame();

            let mut updated_camera = false;
            egui::Window::new("Debug")
                .resizable(true)
                .default_size(Vec2::new(200.0, 100.0))
                .default_open(true)
                .show(egui_renderer.context(), |ui| {
                    ui.heading("Game Settings");
                    ui.label(format!(
                        "Frame Time: {:.3}ms",
                        self.frame_time.as_secs_f64() * 1000.0
                    ));

                    if ui.button("Button!").clicked() {
                        println!("boom!")
                    }

                    ui.separator();
                    ui.heading("Debug");

                    ui.horizontal(|ui| {
                        ui.label("Position: ");

                        if ui
                            .add(
                                DragValue::new(&mut renderer.camera.position.x)
                                    .speed(0.1)
                                    .prefix("x: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {
                            updated_camera = true;
                        }

                        if ui
                            .add(
                                DragValue::new(&mut renderer.camera.position.y)
                                    .speed(0.1)
                                    .prefix("y: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {
                            updated_camera = true;
                        }

                        if ui
                            .add(
                                DragValue::new(&mut renderer.camera.position.z)
                                    .speed(0.1)
                                    .prefix("z: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {
                            updated_camera = true;
                        }
                    });


                    ui.horizontal(|ui| {
                        ui.label("Rotation: ");
                        let (mut roll, mut pitch, mut yaw) =
                            renderer.camera.rotation.euler_angles();

                        roll = roll.to_degrees();
                        pitch = pitch.to_degrees();
                        yaw = yaw.to_degrees();

                        // Add DragValue widgets for roll, pitch, and yaw
                        if ui
                            .add(
                                DragValue::new(&mut yaw)
                                    .speed(1.0)
                                    .prefix("Roll: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {
                            updated_camera = true;
                        }

                        if ui
                            .add(
                                DragValue::new(&mut roll)
                                    .speed(1.0)
                                    .prefix("Pitch: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {
                            updated_camera = true;
                        }

                        if ui
                            .add(
                                DragValue::new(&mut pitch)
                                    .speed(1.0)
                                    .prefix("Yaw: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {
                            updated_camera = true;
                        }
                        
                        if updated_camera { 
                            roll = roll.to_radians();
                            pitch = pitch.to_radians();
                            yaw = yaw.to_radians();
                            renderer.camera.rotation = na::UnitQuaternion::from_euler_angles(roll, pitch, yaw);
                        }
                    });

                    ui.separator();
                    ui.heading("UI Settings");
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Pixels per point: {}",
                            egui_renderer.context().pixels_per_point()
                        ));
                        if ui.button("-").clicked() {
                            self.scale_factor = (self.scale_factor - 0.1).max(0.3);
                        }
                        if ui.button("+").clicked() {
                            self.scale_factor = (self.scale_factor + 0.1).min(3.0);
                        }
                    });
                });

            egui_renderer.end_frame_and_draw(
                &renderer.device,
                &renderer.queue,
                &mut encoder,
                &view,
                screen_descriptor,
            );

            if updated_camera {
                renderer.camera.force_udpate();
                renderer.update_uniforms();
            }
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
        let attribs = WindowAttributes::default()
            .with_inner_size(PhysicalSize::new(1920, 1080))
            .with_title(title);
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

        let egui_renderer = EguiRenderer::new(
            &renderer.device,
            renderer.surface_config.format,
            None,
            1,
            window.clone(),
        );

        log::info!("Egui Created");
        renderer.compute_test();
        let renderer = Arc::new(renderer);
        self.renderers.insert(window.id(), renderer);

        let egui_renderer = Arc::new(egui_renderer);
        self.eguis.insert(window.id(), egui_renderer);
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
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.input.update(&event);
        self.handle_input();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if !self.focus {
            if let Some(egui) = self.eguis.get_mut(&window_id) {
                let egui = Arc::get_mut(egui).unwrap();
                egui.handle_input(&event);
            }
        }

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
                    self.active_window = Some(window.clone());
                    let window = window.deref();

                    if self.focus {
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
            }
            WindowEvent::Focused(false) => {
                log::debug!("Unfocused {:?}", window_id);
                if let Some(window) = self.windows.get(&window_id) {
                    self.active_window = None;
                    let window = window.deref();

                    if self.focus {
                        let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);

                        window.set_cursor_visible(true);
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::builder()
        .filter_module("wgpu_core", LevelFilter::Warn)
        .init();

    let event_loop = EventLoop::with_user_event().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(&event_loop);
    event_loop.run_app(&mut app).unwrap();
}
