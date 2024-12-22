use std::sync::Arc;

use ash::vk;
use vkm::Alloc;

use crate::Device;

pub struct Texture {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub sampler: Option<vk::Sampler>,
    pub details: TextureDetails,
    allocation: vkm::Allocation,
    device: Arc<ash::Device>,
    allocator: Arc<vk_mem::Allocator>,
}

pub struct TextureDetails {
    pub format: vk::Format,
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

pub struct TextureInfo<'a> {
    pub format: vk::Format,
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub usage: vk::ImageUsageFlags,
    pub sharing: vk::SharingMode,
    pub usage_locality: vkm::MemoryUsage,
    pub allocation_locality: vk::MemoryPropertyFlags,
    pub aspect_mask: vk::ImageAspectFlags,
    pub layout: vk::ImageLayout,
    pub view_type: vk::ImageViewType,
    pub sampler: Option<SamplerInfo<'a>>,
    pub label: Option<&'a str>,
    pub tag: Option<(u64, &'a [u8])>,
}

impl Default for TextureInfo<'_> {
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
            aspect_mask: vk::ImageAspectFlags::empty(),
            layout: vk::ImageLayout::UNDEFINED,
            view_type: vk::ImageViewType::TYPE_2D,
            sampler: None,
            label: None,
            tag: None,
        }
    }
}

impl Device {
    pub fn create_texture(&self, info: &TextureInfo<'_>) -> Texture {
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

        let (image, allocation) = unsafe {
            allocator
                .create_image(&texture_info, &allocation_create_info)
                .unwrap()
        };

        self.set_object_debug_info(image, info.label, info.tag);

        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(info.view_type)
            .format(info.format)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(info.aspect_mask)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1),
            );

        let view = unsafe { self.handle.create_image_view(&view_info, None).unwrap() };

        let sampler = if let Some(info) = &info.sampler {
            let mut sampler_create_info = vk::SamplerCreateInfo::default()
                .mag_filter(info.mag)
                .min_filter(info.min)
                .mipmap_mode(info.mipmap)
                .address_mode_u(info.address_u)
                .address_mode_v(info.address_v)
                .address_mode_w(info.address_w)
                .min_lod(info.min_lod)
                .max_lod(info.max_lod);

            if let Some(anisotropy) = info.anisotropy {
                sampler_create_info.anisotropy_enable = 1;
                sampler_create_info.max_anisotropy = anisotropy;
            }

            if let Some(compare) = info.compare {
                sampler_create_info.compare_op(compare);
            }

            let sampler =
                unsafe { self.handle.create_sampler(&sampler_create_info, None) }.unwrap();
            self.set_object_debug_info(sampler, info.label, info.tag);

            Some(sampler)
        } else {
            None
        };

        let details = TextureDetails {
            format: info.format,
            width: info.width,
            height: info.height,
            layers: info.layers,
        };

        Texture {
            image,
            view,
            sampler,
            details,
            allocation,
            device: self.handle.clone(),
            allocator,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Frame {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub index: u32,
}

pub trait Image {
    fn handle(&self) -> vk::Image;
}

impl Image for Texture {
    fn handle(&self) -> vk::Image {
        self.image
    }
}

impl Image for Frame {
    fn handle(&self) -> vk::Image {
        self.image
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

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_image_view(self.view, None);
            if let Some(sampler) = self.sampler {
                self.device.destroy_sampler(sampler, None);
            }
            self.allocator
                .destroy_image(self.image, &mut self.allocation);
        }
    }
}
