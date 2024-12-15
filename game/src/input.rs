use std::{collections::HashSet, time::Duration};

use bytemuck::Zeroable;
use na::ComplexField;
use winit::{
    event::{DeviceEvent, ElementState, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

pub struct Input {
    cursor_position: na::Point2<f32>,
    cursor_delta: na::Vector2<f32>,
    scroll_delta: na::Vector2<f32>,
    scroll_cooldown: f32,
    pressed: HashSet<KeyCode>,
    held: HashSet<KeyCode>,
    released: HashSet<KeyCode>,
}

impl Input {
    pub fn new() -> Self {
        Self {
            cursor_position: na::Point2::zeroed(),
            cursor_delta: na::Vector2::zeros(),
            scroll_delta: na::Vector2::zeros(),
            scroll_cooldown: 0.0,
            pressed: HashSet::new(),
            held: HashSet::new(),
            released: HashSet::new(),
        }
    }

    pub fn flush(&mut self, dt: Duration) {
        self.cursor_delta = na::Vector2::zeros();
        self.scroll_delta = na::Vector2::zeros();
        self.released.clear();

        if self.scroll_cooldown > 0.0 {
            self.scroll_cooldown -= dt.as_secs_f64() as f32;
        }
        if self.scroll_cooldown <= 0.0 {
            self.scroll_delta = na::Vector2::zeros();
        }
    }

    pub fn update(&mut self, event: &DeviceEvent) {
        match event {
            DeviceEvent::Key(event) => {
                let key = match event.physical_key {
                    PhysicalKey::Code(code) => code,
                    PhysicalKey::Unidentified(_code) => return,
                };
                match event.state {
                    ElementState::Pressed => {
                        if self.pressed.contains(&key) {
                            self.pressed.remove(&key);
                            self.held.insert(key);
                            log::trace!("Hold: {:?}", key);
                        } else if self.held.contains(&key) {
                        } else {
                            log::trace!("Pressed: {:?}", key);
                            self.pressed.insert(key);
                        }
                    }
                    ElementState::Released => {
                        self.pressed.remove(&key);
                        self.held.remove(&key);
                        self.released.insert(key);
                        log::trace!("Released: {:?}", key);
                    }
                }
            }
            DeviceEvent::MouseMotion { delta } => {
                let (dx, dy) = *delta;
                self.cursor_delta = na::Vector2::new(dx as f32, dy as f32);
            }
            DeviceEvent::MouseWheel { delta } => {
                if self.scroll_cooldown > 0. {
                    return;
                }
                match *delta {
                    MouseScrollDelta::LineDelta(h, v) => {
                        let v = if v != 0. { v.signum() } else { 0. };
                        let h = if h != 0. { h.signum() } else { 0. };
                        self.scroll_delta = na::Vector2::new(h, -v);
                        self.scroll_cooldown = 0.1;
                        log::trace!("Scroll: {:?}", self.scroll_delta);
                    }
                    MouseScrollDelta::PixelDelta(p) => {
                        const PIXEL_TO_LINE_FACTOR: f32 = 0.005;
                        self.scroll_delta = na::Vector2::new(
                            (p.x as f32) * PIXEL_TO_LINE_FACTOR,
                            (p.y as f32) * PIXEL_TO_LINE_FACTOR,
                        );
                        self.scroll_cooldown = 0.1;
                        log::trace!("Touchpad: {:?}, raw: {:?}", self.scroll_delta, p);
                    }
                };
            }
            _ => {}
        }
    }

    pub fn update_window(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = na::Point2::new(position.x as f32, position.y as f32);
            }

            _ => {}
        }
    }

    pub fn pressed(&self, code: KeyCode) -> bool {
        self.pressed.contains(&code)
    }

    pub fn held(&self, code: KeyCode) -> bool {
        self.held.contains(&code)
    }

    pub fn pressing(&self, code: KeyCode) -> bool {
        self.pressed.contains(&code) || self.held.contains(&code)
    }

    pub fn released(&self, code: KeyCode) -> bool {
        self.released.contains(&code)
    }

    pub fn cursor(&self) -> na::Point2<f32> {
        self.cursor_position
    }

    pub fn cursor_move(&self) -> na::Vector2<f32> {
        self.cursor_delta
    }

    pub fn scroll(&self) -> na::Vector2<f32> {
        self.scroll_delta
    }
}
