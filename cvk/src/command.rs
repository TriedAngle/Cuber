use anyhow::Result;
use ash::vk;
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::Arc,
    thread::{self, ThreadId},
};

use crate::Queue;

pub struct ThreadCommandPools {
    pub device: Arc<ash::Device>,
    pub pools: Mutex<HashMap<(ThreadId, u32), vk::CommandPool>>,
}

impl ThreadCommandPools {
    pub fn new(device: Arc<ash::Device>) -> Self {
        Self {
            device,
            pools: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_pool(&self, queue: &Queue) -> vk::CommandPool {
        let thread_id = thread::current().id();
        let mut pools = self.pools.lock();

        if let Some(&pool) = pools.get(&(thread_id, queue.family_index)) {
            return pool;
        }

        let info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue.family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

        let pool = unsafe { self.device.create_command_pool(&info, None).unwrap() };

        pools.insert((thread_id, queue.family_index), pool);

        pool
    }
}

// TODO: investigate if multiple buffers actually has a usecase?
// why not just create more recorders?
// what are secondary?
pub struct CommandRecorder {
    pub pool: vk::CommandPool,
    pub buffer: vk::CommandBuffer,
    pub device: Arc<ash::Device>,
    pub recording: bool,
}

impl CommandRecorder {
    pub fn begin_rendering(
        &mut self,
        attachments: &[vk::RenderingAttachmentInfo],
        extent: vk::Extent2D,
    ) {
        let info = vk::RenderingInfo::default()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent,
            })
            .layer_count(1)
            .color_attachments(attachments);

        unsafe {
            self.device.cmd_begin_rendering(self.buffer, &info);
        }
    }

    pub fn end_rendering(&mut self) {
        unsafe {
            self.device.cmd_end_rendering(self.buffer);
        }
    }

    pub fn image_barrier(
        &mut self,
        image: vk::Image,
        old: vk::ImageLayout,
        new: vk::ImageLayout,
        src_stage: vk::PipelineStageFlags,
        dst_stage: vk::PipelineStageFlags,
        src_access: vk::AccessFlags,
        dst_access: vk::AccessFlags,
    ) {
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(old)
            .new_layout(new)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1),
            )
            .src_access_mask(src_access)
            .dst_access_mask(dst_access);

        unsafe {
            self.device.cmd_pipeline_barrier(
                self.buffer,
                src_stage,
                dst_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }
    }

    pub fn finish(&mut self) -> vk::CommandBuffer {
        unsafe {
            self.device.end_command_buffer(self.buffer).unwrap();
            self.recording = false;
        }
        self.buffer
    }

    pub fn reset(&mut self) {
        if self.recording {
            let _ = self.finish();
        }

        unsafe {
            self.device
                .reset_command_buffer(self.buffer, vk::CommandBufferResetFlags::empty())
                .unwrap();
        }

        let info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            self.device
                .begin_command_buffer(self.buffer, &info)
                .unwrap();
        }

        self.recording = true;
    }
}

impl Queue {
    pub fn record(&self) -> CommandRecorder {
        let pool = self.device.command_pools.get_pool(&self);

        let info = vk::CommandBufferAllocateInfo::default()
            .command_pool(pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let buffer = unsafe { self.device.handle.allocate_command_buffers(&info).unwrap()[0] };

        unsafe {
            self.device
                .handle
                .begin_command_buffer(
                    buffer,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .unwrap();
        }

        CommandRecorder {
            pool,
            buffer,
            device: self.device.handle.clone(),
            recording: true,
        }
    }

    pub fn submit(
        &self,
        command_buffers: &[vk::CommandBuffer],
        wait_binary: &[(vk::Semaphore, vk::PipelineStageFlags)],
        wait_timeline: &[(vk::Semaphore, u64, vk::PipelineStageFlags)],
        signal_binary: &[vk::Semaphore],
        signal_timeline: &[(vk::Semaphore, u64)],
    ) -> Result<()> {
        // Combine all semaphores
        let mut wait_semaphores = Vec::with_capacity(wait_binary.len() + wait_timeline.len());
        let mut wait_stages = Vec::with_capacity(wait_binary.len() + wait_timeline.len());
        let mut wait_values = Vec::with_capacity(wait_timeline.len());

        for &(sem, stage) in wait_binary {
            wait_semaphores.push(sem);
            wait_stages.push(stage);
            wait_values.push(0);
        }

        for &(sem, value, stage) in wait_timeline {
            wait_semaphores.push(sem);
            wait_stages.push(stage);
            wait_values.push(value);
        }

        let mut signal_semaphores = Vec::with_capacity(signal_binary.len() + signal_timeline.len());
        let mut signal_values = Vec::with_capacity(signal_timeline.len());

        for &sem in signal_binary {
            signal_semaphores.push(sem);
            signal_values.push(0);
        }

        for &(sem, value) in signal_timeline {
            signal_semaphores.push(sem);
            signal_values.push(value);
        }

        let mut timeline_info = vk::TimelineSemaphoreSubmitInfo::default()
            .wait_semaphore_values(&wait_values)
            .signal_semaphore_values(&signal_values);

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(command_buffers)
            .signal_semaphores(&signal_semaphores)
            .push_next(&mut timeline_info);

        unsafe {
            let _lock = self.lock();
            self.device
                .handle
                .queue_submit(self.handle, &[submit_info], vk::Fence::null())?;
        }

        Ok(())
    }
}

impl Drop for CommandRecorder {
    fn drop(&mut self) {
        if self.recording {
            let _ = unsafe { self.device.end_command_buffer(self.buffer) };
        }
        unsafe {
            let _ = self.device.device_wait_idle();
            let _ = self
                .device
                .reset_command_buffer(self.buffer, vk::CommandBufferResetFlags::empty());
        }
    }
}
