use std::collections::HashSet;

use winit::{
    event::{ElementState, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

pub struct Input {
    pressed: HashSet<KeyCode>,
    held: HashSet<KeyCode>,
    released: HashSet<KeyCode>,
}

impl Input {
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
            held: HashSet::new(),
            released: HashSet::new(),
        }
    }

    pub fn update(&mut self, event: &WindowEvent) {
        self.released.clear();
        let x = 0;
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                let key = match event.physical_key {
                    PhysicalKey::Code(code) => code,
                    PhysicalKey::Unidentified(_code) => return,
                };
                match event.state {
                    ElementState::Pressed => {
                        if self.pressed.contains(&key) {
                            self.pressed.remove(&key);
                            self.held.insert(key);
                        } else {
                            self.pressed.insert(key);
                        }
                    }
                    ElementState::Released => {
                        self.pressed.remove(&key);
                        self.held.remove(&key);
                        self.released.insert(key);
                    }
                }
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

    pub fn released(&self, code: KeyCode) -> bool {
        self.released.contains(&code)
    }
}
