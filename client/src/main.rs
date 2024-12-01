use std::time;

use log::LevelFilter;
use winit::{
    application::ApplicationHandler,
    event::{StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::WindowId,
};

use client::{AppEvent, AppState};

pub struct App {
    state: AppState,
}

impl App {
    pub fn empty(event_loop: &EventLoop<AppEvent>) -> Self {
        Self {
            state: AppState::new(event_loop),
        }
    }
    pub fn focus_window(&mut self, window_id: &WindowId) {
        if let Some(window) = self.state.windows.get(window_id) {
            self.state.active_window = Some(window.clone());

            if self.state.focus {
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

    pub fn unfocus_window(&mut self, window_id: &WindowId) {
        if let Some(window) = self.state.windows.get(&window_id) {
            self.state.active_window = None;

            if self.state.focus {
                let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);

                window.set_cursor_visible(true);
            }
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        log::info!("Window Resumed");
        self.state.new_render_window(event_loop, "Cuber");
        self.state.last_update = time::SystemTime::now();
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        match cause {
            StartCause::Poll
            | StartCause::WaitCancelled { .. }
            | StartCause::ResumeTimeReached { .. } => {
                let now = time::SystemTime::now();
                self.state.delta_time = now
                    .duration_since(self.state.last_update)
                    .unwrap_or(time::Duration::from_secs_f32(1.0 / 60.0));
                self.state.last_update = now;
                for window in self.state.windows.values() {
                    window.request_redraw(); // Request redraw for all windows
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::RequestExit => {
                let _windows = self.state.windows.drain();
                let _renderers = self.state.renderers.drain();
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
        self.state.handle_device_event(&event);
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        self.state.handle_window_event(&event, window_id);

        match event {
            WindowEvent::CloseRequested => {
                log::info!("Close requested");
                let _window = self.state.windows.remove(&window_id);
                let _renderer = self.state.renderers.remove(&window_id);
            }
            WindowEvent::Destroyed => {
                log::info!("Destroyed {:?}", window_id);
            }
            WindowEvent::RedrawRequested => {
                log::trace!("Redraw requested {:?}", window_id);
                self.state.render(&window_id);
            }
            WindowEvent::Resized(size) => {
                log::debug!("Resize: {:?}", window_id);
                self.state.resize(&window_id, size);
            }
            WindowEvent::Focused(true) => {
                log::debug!("Focused: {:?}", window_id);
                self.focus_window(&window_id);
            }
            WindowEvent::Focused(false) => {
                log::debug!("Unfocused {:?}", window_id);
                self.unfocus_window(&window_id);
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::builder()
        .filter_module("wgpu_core", LevelFilter::Warn)
        .filter_module("wgpu_hal", LevelFilter::Warn)
        .filter_module("naga", LevelFilter::Warn)
        .init();

    let event_loop = EventLoop::with_user_event().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::empty(&event_loop);
    event_loop.run_app(&mut app).unwrap();
}
