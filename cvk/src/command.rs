use anyhow::Result;
use ash::vk;
use parking_lot::Mutex;
use std::{
    borrow::BorrowMut,
    cell::RefCell,
    collections::HashMap,
    ops,
    rc::Rc,
    sync::{atomic::Ordering, Arc},
    thread::{self, ThreadId},
};

use crate::{Buffer, DescriptorSet, Device, Image, ImageTransition, Pipeline, Queue, Texture};

#[derive(Debug, Clone)]
pub struct CommandBuffer {
    pub handle: vk::CommandBuffer,
    pub submission: Rc<RefCell<u64>>,
}

#[derive(Debug, Clone, Copy)]
pub struct DroppedCommandBuffer {
    pub handle: vk::CommandBuffer,
    pub submission: u64,
}

#[derive(Debug)]
pub struct ThreadCommandPool {
    pub handle: vk::CommandPool,
    device: Arc<Device>,
    pub ready: RefCell<Vec<CommandBuffer>>,
    pub retired: RefCell<Vec<DroppedCommandBuffer>>,
}

#[derive(Debug)]
pub struct CommandPools {
    pub device: Arc<Device>,
    pub pools: Mutex<HashMap<ThreadId, Rc<ThreadCommandPool>>>,
}

pub struct CommandRecorder {
    pub pool: Rc<ThreadCommandPool>,
    pub buffer: CommandBuffer,
    pub device: Arc<ash::Device>,
    pub recording: bool,
    pub pipeline: Option<PipelineBinding>,
}

impl CommandPools {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            pools: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_pool(&self, queue: &Queue) -> Rc<ThreadCommandPool> {
        let thread_id = thread::current().id();
        let mut pools = self.pools.lock();

        if let Some(pool) = pools.get(&thread_id) {
            return pool.clone();
        }

        let info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue.family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

        let handle = unsafe { self.device.handle.create_command_pool(&info, None).unwrap() };

        let pool = Rc::new(ThreadCommandPool {
            handle,
            device: self.device.clone(),
            ready: RefCell::new(Vec::new()),
            retired: RefCell::new(Vec::new()),
        });

        pools.insert(thread_id, pool.clone());

        pool
    }

    pub fn try_cleanup(&self, completed_index: u64) {
        let mut pools = self.pools.lock();

        for (_t, pool) in pools.iter_mut() {
            let mut retired = pool.retired.borrow_mut();
            let freeable = {
                let mut freeable = Vec::new();
                retired.retain(|b| {
                    if b.submission <= completed_index {
                        freeable.push(b.handle);
                        false
                    } else {
                        true
                    }
                });
                freeable
            };

            if !freeable.is_empty() {
                let mut ready = pool.ready.borrow_mut();
                if ready.len() < 10 {
                    let new_ready_buffers = freeable.into_iter().map(|b| {
                        unsafe {
                            let _ = self.device.handle.reset_command_buffer(
                                b,
                                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
                            );
                        }
                        CommandBuffer {
                            handle: b,
                            submission: Rc::new(RefCell::new(0)),
                        }
                    });
                    ready.extend(new_ready_buffers);
                } else {
                    unsafe {
                        self.device
                            .handle
                            .free_command_buffers(pool.handle, &freeable);
                    }
                }
            }
        }
    }
}

impl ThreadCommandPool {
    pub fn retire_buffer(&self, buffer: DroppedCommandBuffer) {
        let mut retired = self.retired.borrow_mut();
        retired.push(buffer);
    }

    pub fn get_buffer(&self) -> CommandBuffer {
        if let Some(buffer) = self.ready.borrow_mut().pop() {
            unsafe {
                self.device
                    .handle
                    .begin_command_buffer(
                        buffer.handle,
                        &vk::CommandBufferBeginInfo::default()
                            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                    )
                    .unwrap();
            }
            return buffer;
        }
        let info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.handle)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let buffer_handle =
            unsafe { self.device.handle.allocate_command_buffers(&info).unwrap()[0] };

        unsafe {
            self.device
                .handle
                .begin_command_buffer(
                    buffer_handle,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .unwrap();
        }

        let buffer = CommandBuffer {
            handle: buffer_handle,
            submission: Rc::new(RefCell::new(0)),
        };

        buffer
    }
}

#[derive(Clone, Copy)]
pub struct PipelineBinding {
    handle: vk::Pipeline,
    layout: vk::PipelineLayout,
    binding: vk::PipelineBindPoint,
    flags: vk::ShaderStageFlags,
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
            self.device.cmd_begin_rendering(self.buffer.handle, &info);
        }
    }

    pub fn end_rendering(&mut self) {
        unsafe {
            self.device.cmd_end_rendering(self.buffer.handle);
        }
    }

    pub fn image_barrier(
        &mut self,
        image: &impl Image,
        old: vk::ImageLayout,
        new: vk::ImageLayout,
        src_stage: vk::PipelineStageFlags,
        dst_stage: vk::PipelineStageFlags,
        src_access: vk::AccessFlags,
        dst_access: vk::AccessFlags,
    ) {
        let image = image.handle();
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
                self.buffer.handle,
                src_stage,
                dst_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }
    }

    pub fn image_transition_subresource(
        &mut self,
        image: &impl Image,
        transition: ImageTransition,
        aspect_mask: vk::ImageAspectFlags,
        base_mip_level: u32,
        level_count: u32,
        base_array_layer: u32,
        layer_count: u32,
    ) {
        let (old_layout, new_layout, src_stage, dst_stage, src_access, dst_access) =
            transition.get_barrier_info();

        let image = image.handle();
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(aspect_mask)
                    .base_mip_level(base_mip_level)
                    .level_count(level_count)
                    .base_array_layer(base_array_layer)
                    .layer_count(layer_count),
            )
            .src_access_mask(src_access)
            .dst_access_mask(dst_access);

        unsafe {
            self.device.cmd_pipeline_barrier(
                self.buffer.handle,
                src_stage,
                dst_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }
    }

    pub fn image_transition(&mut self, image: &impl Image, transition: ImageTransition) {
        self.image_transition_subresource(
            image,
            transition,
            vk::ImageAspectFlags::COLOR,
            0,
            1,
            0,
            1,
        );
    }

    pub fn copy_buffer(
        &mut self,
        src: &Buffer,
        dst: &Buffer,
        src_offset: usize,
        dst_offset: usize,
        size: usize,
    ) {
        let copy = vk::BufferCopy::default()
            .size(size as u64)
            .src_offset(src_offset as u64)
            .dst_offset(dst_offset as u64);

        unsafe {
            self.device
                .cmd_copy_buffer(self.buffer.handle, src.handle, dst.handle, &[copy]);
        }
    }

    pub fn copy_buffer_many(
        &mut self,
        src: &Buffer,
        dst: &Buffer,
        src_offsets: &[usize],
        dst_offsets: &[usize],
        sizes: &[usize],
    ) {
        let copies = src_offsets
            .iter()
            .zip(dst_offsets)
            .zip(sizes)
            .map(|((&src, &dst), &size)| {
                vk::BufferCopy::default()
                    .size(size as u64)
                    .src_offset(src as u64)
                    .dst_offset(dst as u64)
            })
            .collect::<Vec<_>>();

        unsafe {
            self.device
                .cmd_copy_buffer(self.buffer.handle, src.handle, dst.handle, &copies);
        }
    }

    pub fn copy_buffer_image(
        &mut self,
        src: &Buffer,
        dst: &Texture,
        layout: vk::ImageLayout,
        regions: &[vk::BufferImageCopy],
    ) {
        unsafe {
            self.device.cmd_copy_buffer_to_image(
                self.buffer.handle,
                src.handle,
                dst.image,
                layout,
                regions,
            );
        }
    }

    pub fn bind_pipeline(&mut self, pipeline: &impl Pipeline) {
        let pipeline_binding = PipelineBinding {
            handle: pipeline.handle(),
            layout: pipeline.layout(),
            binding: pipeline.bind_point(),
            flags: pipeline.flags(),
        };

        self.pipeline = Some(pipeline_binding);

        unsafe {
            self.device.cmd_bind_pipeline(
                self.buffer.handle,
                pipeline_binding.binding,
                pipeline_binding.handle,
            );
        }
    }

    pub fn push_constants<T: bytemuck::Pod>(&mut self, pc: T) {
        let Some(pipeline) = &self.pipeline else {
            log::error!("Calling Push Constants without a bound pipeline");
            return;
        };

        unsafe {
            self.device.cmd_push_constants(
                self.buffer.handle,
                pipeline.layout,
                pipeline.flags,
                0,
                bytemuck::cast_slice(&[pc]),
            );
        }
    }

    pub fn bind_vertex(&mut self, vertex: &Buffer, binding: u32) {
        unsafe {
            self.device.cmd_bind_vertex_buffers(
                self.buffer.handle,
                binding,
                &[vertex.handle],
                &[0],
            );
        }
    }

    pub fn bind_index(&mut self, index: &Buffer, ty: vk::IndexType) {
        unsafe {
            self.device
                .cmd_bind_index_buffer(self.buffer.handle, index.handle, 0, ty);
        }
    }

    pub fn bind_descriptor_set(&self, set: &DescriptorSet, index: u32, offsets: &[u32]) {
        let Some(pipeline) = &self.pipeline else {
            log::error!("Calling Push Constants without a bound pipeline");
            return;
        };

        unsafe {
            self.device.cmd_bind_descriptor_sets(
                self.buffer.handle,
                pipeline.binding,
                pipeline.layout,
                index,
                &[set.handle],
                offsets,
            );
        }
    }

    pub fn viewport(&mut self, viewport: vk::Viewport) {
        unsafe {
            self.device
                .cmd_set_viewport(self.buffer.handle, 0, &[viewport]);
        }
    }

    pub fn scissor(&mut self, scissor: vk::Rect2D) {
        unsafe {
            self.device
                .cmd_set_scissor(self.buffer.handle, 0, &[scissor]);
        }
    }

    pub fn draw(&mut self, vertex: ops::Range<u32>, instance: ops::Range<u32>) {
        unsafe {
            self.device.cmd_draw(
                self.buffer.handle,
                vertex.len() as u32,
                instance.len() as u32,
                vertex.start,
                instance.start,
            );
        }
    }

    pub fn draw_indexed(
        &mut self,
        indices: ops::Range<u32>,
        instance: ops::Range<u32>,
        vertex_offset: i32,
    ) {
        unsafe {
            self.device.cmd_draw_indexed(
                self.buffer.handle,
                indices.len() as u32,
                instance.len() as u32,
                indices.start,
                vertex_offset,
                instance.start,
            );
        }
    }

    pub fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        unsafe {
            self.device.cmd_dispatch(self.buffer.handle, x, y, z);
        }
    }

    pub fn finish(&mut self) -> CommandBuffer {
        unsafe {
            self.device.end_command_buffer(self.buffer.handle).unwrap();
            self.recording = false;
        }
        self.buffer.clone()
    }

    pub fn reset(&mut self) {
        if self.recording {
            let _ = self.finish();
        }

        unsafe {
            self.device
                .reset_command_buffer(self.buffer.handle, vk::CommandBufferResetFlags::empty())
                .unwrap();
        }

        let info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            self.device
                .begin_command_buffer(self.buffer.handle, &info)
                .unwrap();
        }

        self.recording = true;
    }
}

impl Queue {
    pub fn record(&self) -> CommandRecorder {
        let pool = self.pools.get_pool(&self);

        let buffer = pool.get_buffer();

        CommandRecorder {
            pool,
            buffer,
            device: self.device.handle.clone(),
            recording: true,
            pipeline: None,
        }
    }

    pub fn submit(
        &self,
        command_buffers: &[CommandBuffer],
        wait_binary: &[(vk::Semaphore, vk::PipelineStageFlags)],
        wait_timeline: &[(vk::Semaphore, u64, vk::PipelineStageFlags)],
        signal_binary: &[vk::Semaphore],
        signal_timeline: &[(vk::Semaphore, u64)],
    ) -> Result<u64> {
        let submission_index = self.submission_counter.fetch_add(1, Ordering::Relaxed);

        let timeline = self.timeline.get();

        self.pools.try_cleanup(timeline);

        let submit_buffers = command_buffers
            .iter()
            .map(|b| {
                *RefCell::borrow_mut(&b.submission) = submission_index;
                b.handle
            })
            .collect::<Vec<_>>();

        let mut signal_timeline = signal_timeline.to_vec();
        signal_timeline.push((self.timeline.handle, submission_index));

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

        for &(sem, value) in &signal_timeline {
            signal_semaphores.push(sem);
            signal_values.push(value);
        }

        let mut timeline_info = vk::TimelineSemaphoreSubmitInfo::default()
            .wait_semaphore_values(&wait_values)
            .signal_semaphore_values(&signal_values);

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&submit_buffers)
            .signal_semaphores(&signal_semaphores)
            .push_next(&mut timeline_info);

        unsafe {
            let _lock = self.lock();
            self.device
                .handle
                .queue_submit(self.handle, &[submit_info], vk::Fence::null())?;
        }

        Ok(submission_index)
    }

    pub fn submit_express(&self, command_buffers: &[CommandBuffer]) -> Result<u64> {
        self.submit(command_buffers, &[], &[], &[], &[])
    }
}

impl Drop for CommandRecorder {
    fn drop(&mut self) {
        if self.recording {
            log::error!("Dropping RecordingRecorder Buffer");
            let _ = unsafe { self.device.end_command_buffer(self.buffer.handle) };
        }
        let dropped = DroppedCommandBuffer {
            handle: self.buffer.handle,
            submission: *RefCell::borrow(&self.buffer.submission),
        };

        self.pool.retire_buffer(dropped);
    }
}

impl Drop for CommandPools {
    fn drop(&mut self) {
        for (_t, pool) in self.pools.get_mut() {
            let ready = pool.ready.borrow_mut();
            let retired = pool.retired.borrow_mut();

            let free_retired = retired.iter().map(|b| b.handle).collect::<Vec<_>>();
            let free_ready = ready.iter().map(|b| b.handle).collect::<Vec<_>>();
            unsafe {
                let _ = self.device.handle.device_wait_idle();
                self.device
                    .handle
                    .free_command_buffers(pool.handle, &free_retired);
                self.device
                    .handle
                    .free_command_buffers(pool.handle, &free_ready);
                self.device.handle.destroy_command_pool(pool.handle, None);
            }
        }
    }
}
