use std::{
    collections::HashMap,
    time::{self, Duration},
};

#[derive(Debug)]
pub struct Diagnostics {
    pub sys_times: HashMap<String, time::SystemTime>,
    pub timings: HashMap<String, time::Duration>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self {
            sys_times: HashMap::new(),
            timings: HashMap::new(),
        }
    }

    pub fn start(&mut self, name: &str) {
        let now = time::SystemTime::now();
        self.sys_times.insert(name.to_owned(), now);
    }

    pub fn stop(&mut self, name: &str) {
        let start = self
            .sys_times
            .get(name)
            .map(|t| *t)
            .unwrap_or(time::SystemTime::now());
        let time = start.elapsed().unwrap();
        self.timings.insert(name.to_owned(), time);
    }

    pub fn time(&self, name: &str) -> time::Duration {
        self.timings.get(name).map(|t| *t).unwrap_or(Duration::ZERO)
    }

    pub fn time_millis(&self, name: &str) -> f64 {
        self.time(name).as_secs_f64() * 1000.0
    }
}
