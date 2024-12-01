use std::collections::HashSet;

use winit::{
    event::{DeviceEvent, ElementState},
    keyboard::{KeyCode, PhysicalKey},
};

pub struct Input {
    cursor_position: na::Point2<f32>,
    cursor_delta: na::Vector2<f32>,
    pressed: HashSet<KeyCode>,
    held: HashSet<KeyCode>,
    released: HashSet<KeyCode>,
}

impl Input {
    pub fn new() -> Self {
        Self {
            cursor_position: na::Point2::new(0., 0.),
            cursor_delta: na::Vector2::new(0., 0.),
            pressed: HashSet::new(),
            held: HashSet::new(),
            released: HashSet::new(),
        }
    }

    pub fn update(&mut self, event: &DeviceEvent) {
        self.released.clear();
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
}
