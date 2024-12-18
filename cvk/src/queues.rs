use crate::{Adapter, Device, Instance};
use ash::vk;
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Debug)]
pub struct Queue {
    handle: vk::Queue,
    device: Arc<Device>,
    state: Mutex<()>,
    flags: vk::QueueFlags,
    family_index: u32,
    queue_index: u32,
}

#[derive(Debug, Clone)]
pub struct QueueRequest {
    pub required_flags: vk::QueueFlags,
    pub exclude_flags: vk::QueueFlags,
    pub strict: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct QueueFamilyInfo {
    pub flags: vk::QueueFlags,
    pub family_index: u32,
    pub queue_index: u32,
}

impl Queue {
    pub fn new(device: Arc<Device>, info: &QueueFamilyInfo) -> Arc<Self> {
        let handle = unsafe {
            device
                .handle()
                .get_device_queue(info.family_index, info.queue_index)
        };

        let new = Self {
            handle,
            device,
            state: Mutex::new(()),
            flags: info.flags,
            family_index: info.family_index,
            queue_index: info.queue_index,
        };

        Arc::new(new)
    }

    pub fn find_queue_families(
        instance: &Instance,
        adapter: &Adapter,
        queue_requests: &[QueueRequest],
    ) -> Option<Vec<QueueFamilyInfo>> {
        unsafe {
            let ihandle = instance.handle();
            let physical_device = adapter.handle();

            let queue_families =
                ihandle.get_physical_device_queue_family_properties(physical_device);
            let mut result = vec![None; queue_requests.len()];
            let mut used_queues: Vec<(u32, u32)> = Vec::new(); // (family_index, count)
            let mut used_family_indices = std::collections::HashSet::new();

            for (idx, request) in queue_requests.iter().enumerate().filter(|(_, r)| r.strict) {
                if let Some(info) = Self::find_best_queue_match(
                    &queue_families,
                    request,
                    true,
                    true,
                    &used_queues,
                    &used_family_indices,
                ) {
                    used_family_indices.insert(info.family_index);
                    Self::update_used_queues(&mut used_queues, info.family_index, info.queue_index);
                    result[idx] = Some(info);
                }
            }

            for (idx, request) in queue_requests.iter().enumerate() {
                if result[idx].is_none() && request.strict {
                    if let Some(info) = Self::find_best_queue_match(
                        &queue_families,
                        request,
                        false, // allow shared queues
                        true,  // respect exclusions
                        &used_queues,
                        &used_family_indices,
                    ) {
                        used_family_indices.insert(info.family_index);
                        Self::update_used_queues(
                            &mut used_queues,
                            info.family_index,
                            info.queue_index,
                        );
                        result[idx] = Some(info);
                    }
                }
            }

            for (idx, request) in queue_requests.iter().enumerate() {
                if result[idx].is_none() {
                    if let Some(info) = Self::find_best_queue_match(
                        &queue_families,
                        request,
                        false, // allow shared queues
                        false, // relaxed exclusions
                        &used_queues,
                        &used_family_indices,
                    ) {
                        used_family_indices.insert(info.family_index);
                        Self::update_used_queues(
                            &mut used_queues,
                            info.family_index,
                            info.queue_index,
                        );
                        result[idx] = Some(info);
                    }
                }
            }

            if result.iter().all(|x| x.is_some()) {
                Some(result.into_iter().map(|x| x.unwrap()).collect())
            } else {
                None
            }
        }
    }

    fn find_best_queue_match(
        queue_families: &[vk::QueueFamilyProperties],
        request: &QueueRequest,
        dedicated_only: bool,
        respect_exclusions: bool,
        used_queues: &[(u32, u32)],
        used_family_indices: &std::collections::HashSet<u32>,
    ) -> Option<QueueFamilyInfo> {
        let mut best_match: Option<(u32, vk::QueueFlags, u32, u32)> = None; // (index, flags, used_count, score)
        let mut min_excluded_flags = u32::MAX;

        for (index, properties) in queue_families.iter().enumerate() {
            let family_index = index as u32;

            // Skip if this family index is already used
            if used_family_indices.contains(&family_index) {
                continue;
            }

            // Check available queues
            let used_count = used_queues
                .iter()
                .find(|(idx, _)| *idx == family_index)
                .map(|(_, count)| *count)
                .unwrap_or(0);

            if used_count >= properties.queue_count {
                continue;
            }

            let flags = properties.queue_flags;

            // Check required flags
            if !flags.contains(request.required_flags) {
                continue;
            }

            // Check dedicated queue requirement
            if dedicated_only && flags != request.required_flags {
                continue;
            }

            // Handle exclusions
            let excluded_flags_present = flags & request.exclude_flags;
            let excluded_count = excluded_flags_present.as_raw().count_ones();

            if respect_exclusions && excluded_count > 0 {
                continue;
            }

            // Calculate score based on how well this queue matches our needs
            let mut score = 0u32;

            // Prefer queues with fewer additional capabilities beyond what we need
            score +=
                (flags.as_raw().count_ones() - request.required_flags.as_raw().count_ones()) * 2;

            // Prefer queues with more available slots
            score += used_count;

            // Penalize queues with excluded flags
            score += excluded_count * 4;

            // Update best match if this is better
            if excluded_count < min_excluded_flags
                || (excluded_count == min_excluded_flags
                    && best_match.map_or(true, |(_, _, _, best_score)| score < best_score))
            {
                min_excluded_flags = excluded_count;
                best_match = Some((family_index, flags, used_count, score));
            }
        }

        best_match.map(|(family_index, flags, queue_index, _)| QueueFamilyInfo {
            family_index,
            flags,
            queue_index,
        })
    }

    fn update_used_queues(used_queues: &mut Vec<(u32, u32)>, family_index: u32, queue_index: u32) {
        if let Some(entry) = used_queues.iter_mut().find(|(idx, _)| *idx == family_index) {
            entry.1 = queue_index + 1;
        } else {
            used_queues.push((family_index, queue_index + 1));
        }
    }
    pub fn handle(&self) -> vk::Queue {
        self.handle
    }

    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    pub fn lock(&self) -> parking_lot::lock_api::MutexGuard<'_, parking_lot::RawMutex, ()> {
        self.state.lock()
    }

    pub fn queue_family(&self) -> u32 {
        self.family_index
    }

    pub fn queue_index(&self) -> u32 {
        self.queue_index
    }

    pub fn flags(&self) -> vk::QueueFlags {
        self.flags
    }
}
