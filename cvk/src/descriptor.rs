use ash::vk;
use std::sync::Arc;

use crate::{Buffer, Device, Sampler};

#[derive(Clone, Copy, Debug)]
pub enum DescriptorType {
    UniformBuffer,
    StorageBuffer,
    StorageImage,
    SampledImage,
    Sampler,
    CombinedImageSampler,
}

impl From<DescriptorType> for vk::DescriptorType {
    fn from(ty: DescriptorType) -> Self {
        match ty {
            DescriptorType::UniformBuffer => vk::DescriptorType::UNIFORM_BUFFER,
            DescriptorType::StorageBuffer => vk::DescriptorType::STORAGE_BUFFER,
            DescriptorType::StorageImage => vk::DescriptorType::STORAGE_IMAGE,
            DescriptorType::SampledImage => vk::DescriptorType::SAMPLED_IMAGE,
            DescriptorType::Sampler => vk::DescriptorType::SAMPLER,
            DescriptorType::CombinedImageSampler => vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        }
    }
}

#[derive(Copy, Clone)]
pub struct DescriptorBinding {
    pub binding: u32,
    pub ty: DescriptorType,
    pub count: u32,
    pub stages: vk::ShaderStageFlags,
    pub flags: Option<vk::DescriptorBindingFlags>,
}

impl DescriptorBinding {
    pub fn unique(binding: u32, ty: DescriptorType, stages: vk::ShaderStageFlags) -> Self {
        Self {
            binding,
            ty,
            count: 1,
            stages,
            flags: None,
        }
    }
    pub fn array(
        binding: u32,
        ty: DescriptorType,
        count: u32,
        stages: vk::ShaderStageFlags,
    ) -> Self {
        Self {
            binding,
            ty,
            count,
            stages,
            flags: None,
        }
    }
}

pub struct DescriptorSetLayoutInfo<'a> {
    pub bindings: &'a [DescriptorBinding],
    pub flags: vk::DescriptorSetLayoutCreateFlags,
    pub label: Option<&'a str>,
    pub tag: Option<(u64, &'a [u8])>,
}

impl Default for DescriptorSetLayoutInfo<'_> {
    fn default() -> Self {
        Self {
            bindings: &[],
            flags: vk::DescriptorSetLayoutCreateFlags::empty(),
            label: None,
            tag: None,
        }
    }
}

pub struct DescriptorSetLayout {
    pub handle: vk::DescriptorSetLayout,
    pub bindings: Vec<DescriptorBinding>,
    device: Arc<ash::Device>,
}

pub struct DescriptorPoolInfo<'a> {
    pub max_sets: u32,
    pub layouts: &'a [&'a DescriptorSetLayout],
    pub flags: vk::DescriptorPoolCreateFlags,
    pub label: Option<&'a str>,
    pub tag: Option<(u64, &'a [u8])>,
}

impl Default for DescriptorPoolInfo<'_> {
    fn default() -> Self {
        Self {
            max_sets: 0,
            layouts: &[],
            flags: vk::DescriptorPoolCreateFlags::empty(),
            label: None,
            tag: None,
        }
    }
}

pub struct DescriptorPool {
    pub handle: vk::DescriptorPool,
    device: Arc<ash::Device>,
}

pub enum DescriptorWrite<'a> {
    StorageBuffer {
        binding: u32,
        buffer: &'a Buffer,
        offset: vk::DeviceSize,
        range: vk::DeviceSize,
        array_element: Option<u32>,
    },
    StorageImage {
        binding: u32,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
        array_element: Option<u32>,
    },
    SampledImage {
        binding: u32,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
        array_element: Option<u32>,
    },
    Sampler {
        binding: u32,
        sampler: &'a Sampler,
        array_element: Option<u32>,
    },
    CombinedImageSampler {
        binding: u32,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
        sampler: &'a Sampler,
        array_element: Option<u32>,
    },
}

pub struct DescriptorSet {
    pub handle: vk::DescriptorSet,
    pub pool: Arc<DescriptorPool>,
    device: Arc<ash::Device>,
}

impl Device {
    pub fn create_descriptor_set_layout(
        &self,
        info: &DescriptorSetLayoutInfo,
    ) -> DescriptorSetLayout {
        let vk_bindings: Vec<_> = info
            .bindings
            .iter()
            .map(|binding| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(binding.binding)
                    .descriptor_type(binding.ty.clone().into())
                    .descriptor_count(binding.count)
                    .stage_flags(binding.stages)
            })
            .collect();

        let binding_flags = info
            .bindings
            .iter()
            .map(|binding| {
                if let Some(flags) = binding.flags {
                    flags
                } else if binding.count > 1 {
                    vk::DescriptorBindingFlags::PARTIALLY_BOUND
                        | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND
                } else {
                    vk::DescriptorBindingFlags::empty()
                }
            })
            .collect::<Vec<_>>();

        let mut binding_flags_create_info =
            vk::DescriptorSetLayoutBindingFlagsCreateInfo::default().binding_flags(&binding_flags);

        let mut create_flags = info.flags;
        if binding_flags
            .iter()
            .any(|&f| f.contains(vk::DescriptorBindingFlags::UPDATE_AFTER_BIND))
        {
            create_flags |= vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL;
        }

        let create_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(&vk_bindings)
            .flags(create_flags)
            .push_next(&mut binding_flags_create_info);

        let handle = unsafe {
            self.handle
                .create_descriptor_set_layout(&create_info, None)
                .unwrap()
        };

        self.set_object_debug_info(handle, info.label, info.tag);

        DescriptorSetLayout {
            handle,
            bindings: Vec::from_iter(info.bindings.iter().copied()),
            device: self.handle.clone(),
        }
    }

    pub fn create_descriptor_pool(&self, info: &DescriptorPoolInfo) -> Arc<DescriptorPool> {
        let mut type_counts: std::collections::HashMap<vk::DescriptorType, u32> =
            std::collections::HashMap::new();

        for layout in info.layouts {
            for binding in &layout.bindings {
                let ty: vk::DescriptorType = binding.ty.clone().into();
                *type_counts.entry(ty).or_default() += binding.count;
            }
        }

        let pool_sizes: Vec<_> = type_counts
            .iter()
            .map(|(&ty, &count)| {
                vk::DescriptorPoolSize::default()
                    .ty(ty)
                    .descriptor_count(count * info.max_sets)
            })
            .collect();

        let create_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(info.max_sets)
            .flags(info.flags);

        let handle = unsafe {
            self.handle
                .create_descriptor_pool(&create_info, None)
                .unwrap()
        };

        self.set_object_debug_info(handle, info.label, info.tag);

        Arc::new(DescriptorPool {
            handle,
            device: self.handle.clone(),
        })
    }
}

impl Device {
    pub fn create_descriptor_set(
        &self,
        pool: Arc<DescriptorPool>,
        layout: &DescriptorSetLayout,
    ) -> DescriptorSet {
        let layouts = [layout.handle];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(pool.handle)
            .set_layouts(&layouts);

        let handle = unsafe { self.handle.allocate_descriptor_sets(&alloc_info).unwrap()[0] };

        DescriptorSet {
            handle,
            device: layout.device.clone(),
            pool,
        }
    }
}

impl DescriptorSet {
    pub fn write(&self, writes: &[DescriptorWrite]) {
        let mut vk_writes = Vec::with_capacity(writes.len());
        let mut buffer_infos = Vec::with_capacity(writes.len());
        let mut image_infos = Vec::with_capacity(writes.len());

        for write in writes {
            match write {
                DescriptorWrite::StorageBuffer {
                    binding,
                    buffer,
                    offset,
                    range,
                    array_element,
                } => {
                    buffer_infos.push((
                        *binding,
                        vk::DescriptorType::STORAGE_BUFFER,
                        vk::DescriptorBufferInfo::default()
                            .buffer(buffer.handle)
                            .offset(*offset)
                            .range(*range),
                        array_element.unwrap_or(0),
                    ));
                }
                DescriptorWrite::StorageImage {
                    binding,
                    image_view,
                    image_layout,
                    array_element,
                } => {
                    image_infos.push((
                        *binding,
                        vk::DescriptorType::STORAGE_IMAGE,
                        vk::DescriptorImageInfo::default()
                            .image_view(*image_view)
                            .image_layout(*image_layout),
                        array_element.unwrap_or(0),
                    ));
                }
                DescriptorWrite::SampledImage {
                    binding,
                    image_view,
                    image_layout,
                    array_element,
                } => {
                    image_infos.push((
                        *binding,
                        vk::DescriptorType::SAMPLED_IMAGE,
                        vk::DescriptorImageInfo::default()
                            .image_view(*image_view)
                            .image_layout(*image_layout),
                        array_element.unwrap_or(0),
                    ));
                }
                DescriptorWrite::Sampler {
                    binding,
                    sampler,
                    array_element,
                } => {
                    image_infos.push((
                        *binding,
                        vk::DescriptorType::SAMPLER,
                        vk::DescriptorImageInfo::default().sampler(sampler.handle),
                        array_element.unwrap_or(0),
                    ));
                }
                DescriptorWrite::CombinedImageSampler {
                    binding,
                    image_view,
                    image_layout,
                    sampler,
                    array_element,
                } => {
                    image_infos.push((
                        *binding,
                        vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                        vk::DescriptorImageInfo::default()
                            .image_view(*image_view)
                            .image_layout(*image_layout)
                            .sampler(sampler.handle),
                        array_element.unwrap_or(0),
                    ));
                }
            }
        }

        // Create descriptor writes for buffers
        for (binding, descriptor_type, buffer_info, array_element) in &buffer_infos {
            vk_writes.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(self.handle)
                    .dst_binding(*binding)
                    .dst_array_element(*array_element)
                    .descriptor_type(*descriptor_type)
                    .buffer_info(std::slice::from_ref(buffer_info)),
            );
        }

        // Create descriptor writes for images
        for (binding, descriptor_type, image_info, array_element) in &image_infos {
            vk_writes.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(self.handle)
                    .dst_binding(*binding)
                    .dst_array_element(*array_element)
                    .descriptor_type(*descriptor_type)
                    .image_info(std::slice::from_ref(image_info)),
            );
        }

        unsafe {
            self.device.update_descriptor_sets(&vk_writes, &[]);
        }
    }
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_descriptor_set_layout(self.handle, None);
        }
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_descriptor_pool(self.handle, None);
        }
    }
}
