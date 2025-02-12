use std::{collections::HashSet, time::Duration};

use bytemuck::Zeroable;
use winit::{
    event::{DeviceEvent, ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

pub struct Input {
    cursor_position: na::Point2<f32>,
    cursor_delta: na::Vector2<f32>,
    cursor_delta_accumulator: na::Vector2<f32>,
    scroll_delta: na::Vector2<f32>,
    scroll_cooldown: f32,
    pressed: HashSet<KeyCode>,
    held: HashSet<KeyCode>,
    released: HashSet<KeyCode>,

    pressed_buttons: HashSet<MouseButton>,
    held_buttons: HashSet<MouseButton>,
    released_buttons: HashSet<MouseButton>,
}

impl Input {
    pub fn new() -> Self {
        Self {
            cursor_position: na::Point2::zeroed(),
            cursor_delta: na::Vector2::zeros(),
            cursor_delta_accumulator: na::Vector2::zeros(),
            scroll_delta: na::Vector2::zeros(),
            scroll_cooldown: 0.0,
            pressed: HashSet::new(),
            held: HashSet::new(),
            released: HashSet::new(),

            pressed_buttons: HashSet::new(),
            held_buttons: HashSet::new(),
            released_buttons: HashSet::new(),
        }
    }

    pub fn flush(&mut self, dt: Duration) {
        self.cursor_delta = self.cursor_delta_accumulator;
        self.cursor_delta_accumulator = na::Vector2::zeros();
        self.scroll_delta = na::Vector2::zeros();
        self.released.clear();
        self.released_buttons.clear();

        if self.scroll_cooldown > 0.0 {
            self.scroll_cooldown -= dt.as_secs_f64() as f32;
        }
        if self.scroll_cooldown <= 0.0 {
            self.scroll_delta = na::Vector2::zeros();
        }

        self.released.clear();
        self.held.extend(self.pressed.drain());

        self.released_buttons.clear();
        self.held_buttons.extend(self.pressed_buttons.drain());
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
                        self.pressed.insert(key);
                    }
                    ElementState::Released => {
                        self.held.remove(&key);
                        self.released.insert(key);
                    }
                }
            }
            DeviceEvent::MouseMotion { delta } => {
                let (dx, dy) = *delta;
                self.cursor_delta_accumulator += na::Vector2::new(dx as f32, -dy as f32);
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
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => {
                    self.pressed_buttons.insert(*button);
                }
                ElementState::Released => {
                    self.held_buttons.remove(button);
                    self.released_buttons.insert(*button);
                    log::trace!("Released: {:?}", button);
                }
            },

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

    pub fn mouse_pressed(&self, code: MouseButton) -> bool {
        self.pressed_buttons.contains(&code)
    }

    pub fn mouse_held(&self, code: MouseButton) -> bool {
        self.held_buttons.contains(&code)
    }

    pub fn mouse_pressing(&self, code: MouseButton) -> bool {
        self.pressed_buttons.contains(&code) || self.held_buttons.contains(&code)
    }

    pub fn mouse_released(&self, code: MouseButton) -> bool {
        self.released_buttons.contains(&code)
    }
}
