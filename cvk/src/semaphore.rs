use std::{fmt, sync::Arc, time::Duration, u64};

use ash::vk;

use crate::Device;

pub struct Semaphore {
    pub handle: vk::Semaphore,
    device: Arc<ash::Device>,
}

impl fmt::Debug for Semaphore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Semaphore").finish()
    }
}

impl Device {
    pub fn create_semaphore(&self, value: u64) -> Semaphore {
        let mut type_info = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(value);

        let info = vk::SemaphoreCreateInfo::default().push_next(&mut type_info);

        let handle = unsafe { self.handle.create_semaphore(&info, None).unwrap() };

        Semaphore {
            handle,
            device: self.handle.clone(),
        }
    }

    pub fn create_binary_semaphore(&self, signaled: bool) -> Semaphore {
        let info = vk::SemaphoreCreateInfo::default();

        let handle = unsafe { self.handle.create_semaphore(&info, None).unwrap() };

        if signaled {
            let signal_info = vk::SemaphoreSignalInfo::default()
                .semaphore(handle)
                .value(1);

            unsafe {
                let _ = self.handle.signal_semaphore(&signal_info);
            }
        }

        Semaphore {
            handle,
            device: self.handle.clone(),
        }
    }
}

impl Semaphore {
    pub fn get(&self) -> u64 {
        unsafe {
            self.device
                .get_semaphore_counter_value(self.handle)
                .unwrap()
        }
    }

    pub fn signal(&self, value: u64) {
        let info = vk::SemaphoreSignalInfo::default()
            .semaphore(self.handle)
            .value(value);

        unsafe {
            let _ = self.device.signal_semaphore(&info);
        }
    }

    pub fn wait(&self, value: u64, timeout: Option<Duration>) {
        let handles = [self.handle];
        let values = [value];
        let timeout_ns = timeout.map_or(u64::MAX, |d| d.as_nanos() as u64);
        let info = vk::SemaphoreWaitInfo::default()
            .semaphores(&handles)
            .values(&values);

        unsafe {
            let _ = self.device.wait_semaphores(&info, timeout_ns);
        }
    }

    pub fn wait_many<S: AsRef<Semaphore>>(
        semaphores: &[S],
        values: &[u64],
        timeout: Option<Duration>,
    ) {
        if semaphores.is_empty() {
            return;
        }

        let timeout_ns = timeout.map_or(u64::MAX, |d| d.as_nanos() as u64);

        let device = semaphores[0].as_ref().device.clone();

        let handles = semaphores
            .iter()
            .map(|s| s.as_ref().handle)
            .collect::<Vec<_>>();

        let info = vk::SemaphoreWaitInfo::default()
            .semaphores(&handles)
            .values(values);

        unsafe {
            let _ = device.wait_semaphores(&info, timeout_ns);
        }
    }
}

impl AsRef<Semaphore> for Semaphore {
    fn as_ref(&self) -> &Semaphore {
        self
    }
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_semaphore(self.handle, None);
        }
    }
}
