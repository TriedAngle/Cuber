use std::sync::Arc;

use ash::vk::{self, Sampler};
use vkm::Alloc;

use crate::Device;

pub struct Texture {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub sampler: Option<vk::Sampler>,
    allocation: vkm::Allocation,
    device: Arc<ash::Device>,
    allocator: Arc<vk_mem::Allocator>,
}

pub struct SamplerInfo<'a> {
    pub mag: vk::Filter,
    pub min: vk::Filter,
    pub mipmap: vk::SamplerMipmapMode,
    pub address_u: vk::SamplerAddressMode,
    pub address_v: vk::SamplerAddressMode,
    pub address_w: vk::SamplerAddressMode,
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
            label: None,
            tag: None,
        }
    }
}

pub struct TextureInfo<'a> {
    pub format: vk::Format,
    pub width: u32,
    pub height: u32,
    pub usage: vk::ImageUsageFlags,
    pub sharing: vk::SharingMode,
    pub usage_locality: vkm::MemoryUsage,
    pub allocation_locality: vk::MemoryPropertyFlags,
    pub aspect_mask: vk::ImageAspectFlags,
    pub layout: vk::ImageLayout,
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
            usage: vk::ImageUsageFlags::empty(),
            sharing: vk::SharingMode::EXCLUSIVE,
            usage_locality: vkm::MemoryUsage::Auto,
            allocation_locality: vk::MemoryPropertyFlags::empty(),
            aspect_mask: vk::ImageAspectFlags::empty(),
            layout: vk::ImageLayout::UNDEFINED,
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
            .array_layers(1)
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
            .view_type(vk::ImageViewType::TYPE_2D)
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
            let sampler_create_info = vk::SamplerCreateInfo::default()
                .mag_filter(info.mag)
                .min_filter(info.min)
                .mipmap_mode(info.mipmap)
                .address_mode_u(info.address_u)
                .address_mode_v(info.address_v)
                .address_mode_w(info.address_w);

            let sampler =
                unsafe { self.handle.create_sampler(&sampler_create_info, None) }.unwrap();
            self.set_object_debug_info(sampler, info.label, info.tag);

            Some(sampler)
        } else {
            None
        };

        Texture {
            image,
            view,
            sampler,
            allocation,
            device: self.handle.clone(),
            allocator,
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
