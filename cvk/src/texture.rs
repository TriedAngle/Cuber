use std::{ops, sync::Arc};

use ash::vk;
use vkm::Alloc;

use crate::{Allocation, Device};

#[derive(Clone)]
pub struct Sampler {
    pub handle: vk::Sampler,
    pub device: Arc<ash::Device>,
}

pub struct Image {
    pub handle: vk::Image,
    pub view: vk::ImageView,
    pub sampler: Option<Sampler>,
    pub details: ImageDetails,
    pub device: Arc<ash::Device>,
    pub allocation: Option<Allocation>,
}

impl Image {
    /// cloned images don't own resources, only handles and can be destroyed !
    pub unsafe fn unsafe_clone(&self) -> Self {
        Self {
            handle: self.handle,
            view: self.view,
            sampler: None,
            details: self.details,
            device: self.device.clone(),
            allocation: None,
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct ImageDetails {
    pub format: vk::Format,
    pub layout: vk::ImageLayout,
    pub width: u32,
    pub height: u32,
    pub layers: u32,
}

pub struct SamplerInfo<'a> {
    pub mag: vk::Filter,
    pub min: vk::Filter,
    pub mipmap: vk::SamplerMipmapMode,
    pub address_u: vk::SamplerAddressMode,
    pub address_v: vk::SamplerAddressMode,
    pub address_w: vk::SamplerAddressMode,
    pub anisotropy: Option<f32>,
    pub compare: Option<vk::CompareOp>,
    pub min_lod: f32,
    pub max_lod: f32,
    pub label: Option<&'a str>,
    pub tag: Option<(u64, &'a [u8])>,
}

pub struct ImageViewInfo<'a> {
    pub ty: vk::ImageViewType,
    pub aspect: vk::ImageAspectFlags,
    pub swizzle: vk::ComponentMapping,
    pub mips: ops::Range<u32>,
    pub layers: ops::Range<u32>,
    pub label: Option<&'a str>,
    pub tag: Option<(u64, &'a [u8])>,
}

pub struct ImageInfo<'a> {
    pub format: vk::Format,
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub usage: vk::ImageUsageFlags,
    pub sharing: vk::SharingMode,
    pub usage_locality: vkm::MemoryUsage,
    pub allocation_locality: vk::MemoryPropertyFlags,
    pub layout: vk::ImageLayout,
    pub view: ImageViewInfo<'a>,
    pub sampler: Option<SamplerInfo<'a>>,
    pub label: Option<&'a str>,
    pub tag: Option<(u64, &'a [u8])>,
}

impl Default for SamplerInfo<'_> {
    fn default() -> Self {
        Self {
            mag: vk::Filter::LINEAR,
            min: vk::Filter::LINEAR,
            mipmap: vk::SamplerMipmapMode::LINEAR,
            address_u: vk::SamplerAddressMode::CLAMP_TO_EDGE,
            address_v: vk::SamplerAddressMode::CLAMP_TO_EDGE,
            address_w: vk::SamplerAddressMode::CLAMP_TO_EDGE,
            anisotropy: None,
            compare: None,
            min_lod: 0.0,
            max_lod: 0.0,
            label: None,
            tag: None,
        }
    }
}

impl Default for ImageViewInfo<'_> {
    fn default() -> Self {
        Self {
            ty: vk::ImageViewType::TYPE_2D,
            aspect: vk::ImageAspectFlags::empty(),
            swizzle: vk::ComponentMapping::default(),
            mips: 0..1,
            layers: 0..1,
            label: None,
            tag: None,
        }
    }
}

impl Default for ImageInfo<'_> {
    fn default() -> Self {
        Self {
            format: vk::Format::UNDEFINED,
            width: 0,
            height: 0,
            layers: 1,
            usage: vk::ImageUsageFlags::empty(),
            sharing: vk::SharingMode::EXCLUSIVE,
            usage_locality: vkm::MemoryUsage::Auto,
            allocation_locality: vk::MemoryPropertyFlags::empty(),
            layout: vk::ImageLayout::UNDEFINED,
            view: ImageViewInfo::default(),
            sampler: None,
            label: None,
            tag: None,
        }
    }
}

pub struct CustomImageViewInfo<'a> {
    pub image: vk::Image,
    pub format: vk::Format,
    pub ty: vk::ImageViewType,
    pub aspect: vk::ImageAspectFlags,
    pub swizzle: vk::ComponentMapping,
    pub mips: ops::Range<u32>,
    pub layers: ops::Range<u32>,
    pub label: Option<&'a str>,
    pub tag: Option<(u64, &'a [u8])>,
}

impl Default for CustomImageViewInfo<'_> {
    fn default() -> Self {
        Self {
            image: vk::Image::null(),
            format: vk::Format::UNDEFINED,
            ty: vk::ImageViewType::TYPE_2D,
            aspect: vk::ImageAspectFlags::empty(),
            swizzle: vk::ComponentMapping::default(),
            mips: 0..1,
            layers: 0..1,
            label: None,
            tag: None,
        }
    }
}

impl<'a> CustomImageViewInfo<'a> {
    pub fn new(image: vk::Image, format: vk::Format, info: &'a ImageViewInfo<'a>) -> Self {
        Self {
            image,
            format,
            ty: info.ty,
            aspect: info.aspect,
            swizzle: info.swizzle,
            mips: info.mips.clone(),
            layers: info.layers.clone(),
            label: info.label,
            tag: info.tag,
        }
    }
}

impl Device {
    pub fn create_texture(&self, info: &ImageInfo<'_>) -> Image {
        let allocator = self.allocator.clone();
        let texture_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(info.format)
            .extent(vk::Extent3D {
                width: info.width,
                height: info.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(info.layers)
            .samples(vk::SampleCountFlags::TYPE_1)
            .usage(info.usage)
            .sharing_mode(info.sharing)
            .initial_layout(info.layout);

        let allocation_create_info = vkm::AllocationCreateInfo {
            usage: info.usage_locality,
            required_flags: info.allocation_locality,
            ..Default::default()
        };

        let (handle, allocation) = unsafe {
            allocator
                .create_image(&texture_info, &allocation_create_info)
                .unwrap()
        };

        self.set_object_debug_info(handle, info.label, info.tag);

        let view_info = CustomImageViewInfo::new(handle, info.format, &info.view);
        let view = self.create_image_view(&view_info);

        let sampler = if let Some(info) = &info.sampler {
            let sampler = self.create_sampler(info);
            Some(sampler)
        } else {
            None
        };

        let details = ImageDetails {
            format: info.format,
            width: info.width,
            height: info.height,
            layers: info.layers,
            layout: info.layout,
        };

        let allocation = Some(Allocation {
            handle: allocation,
            allocator: allocator.clone(),
        });

        Image {
            handle,
            view,
            sampler,
            details,
            device: self.handle.clone(),
            allocation,
        }
    }

    pub fn create_image_view(&self, info: &CustomImageViewInfo<'_>) -> vk::ImageView {
        let view_info = vk::ImageViewCreateInfo::default()
            .image(info.image)
            .view_type(info.ty)
            .format(info.format)
            .components(info.swizzle)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(info.aspect)
                    .base_mip_level(info.mips.start)
                    .level_count(info.mips.len() as u32)
                    .base_array_layer(info.layers.start)
                    .layer_count(info.layers.len() as u32),
            );

        let handle = unsafe { self.handle.create_image_view(&view_info, None).unwrap() };
        handle
    }

    pub fn create_sampler(&self, info: &SamplerInfo<'_>) -> Sampler {
        let mut create_info = vk::SamplerCreateInfo::default()
            .mag_filter(info.mag)
            .min_filter(info.min)
            .mipmap_mode(info.mipmap)
            .address_mode_u(info.address_u)
            .address_mode_v(info.address_v)
            .address_mode_w(info.address_w)
            .min_lod(info.min_lod)
            .max_lod(info.max_lod);

        if let Some(anisotropy) = info.anisotropy {
            create_info.anisotropy_enable = 1;
            create_info.max_anisotropy = anisotropy;
        }

        if let Some(compare) = info.compare {
            create_info = create_info.compare_op(compare);
        }

        let handle = unsafe { self.handle.create_sampler(&create_info, None) }.unwrap();
        self.set_object_debug_info(handle, info.label, info.tag);

        Sampler {
            handle,
            device: self.handle.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ImageTransition {
    /// Transition from undefined to general layout
    General,
    /// Transition to shader read-only optimal layout
    ShaderRead,
    /// Transition to color attachment optimal layout
    ColorAttachment,
    /// Transition to transfer dst optimal layout
    TransferDst,
    /// Transition to present src layout
    Present,
    /// Custom transition with specified layouts
    Custom {
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        src_stage: vk::PipelineStageFlags,
        dst_stage: vk::PipelineStageFlags,
        src_access: vk::AccessFlags,
        dst_access: vk::AccessFlags,
    },
}

impl ImageTransition {
    pub fn get_barrier_info(
        &self,
    ) -> (
        vk::ImageLayout,
        vk::ImageLayout,
        vk::PipelineStageFlags,
        vk::PipelineStageFlags,
        vk::AccessFlags,
        vk::AccessFlags,
    ) {
        match *self {
            ImageTransition::General => (
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::GENERAL,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::AccessFlags::empty(),
                vk::AccessFlags::SHADER_WRITE,
            ),
            ImageTransition::ShaderRead => (
                vk::ImageLayout::GENERAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::AccessFlags::SHADER_WRITE,
                vk::AccessFlags::SHADER_READ,
            ),
            ImageTransition::ColorAttachment => (
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::AccessFlags::empty(),
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            ),
            ImageTransition::TransferDst => (
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::AccessFlags::empty(),
                vk::AccessFlags::TRANSFER_WRITE,
            ),
            ImageTransition::Present => (
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::AccessFlags::empty(),
            ),
            ImageTransition::Custom {
                old_layout,
                new_layout,
                src_stage,
                dst_stage,
                src_access,
                dst_access,
            } => (
                old_layout, new_layout, src_stage, dst_stage, src_access, dst_access,
            ),
        }
    }
}

impl Image {
    pub fn sampler(&self) -> &Sampler {
        let ptr = if let Some(sampler) = &self.sampler {
            sampler as *const Sampler
        } else {
            std::ptr::null()
        };

        let sampler = unsafe { ptr.as_ref().unwrap() };
        sampler
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_sampler(self.handle, None);
        }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            if let Some(allocation) = &mut self.allocation {
                self.device.destroy_image_view(self.view, None);
                allocation
                    .allocator
                    .destroy_image(self.handle, &mut allocation.handle);
            }
        }
    }
}
