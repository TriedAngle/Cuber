use anyhow::Result;
use std::sync::Arc;

use ash::vk;
use vkm::Alloc;

use crate::Device;

pub struct Texture {
    image: vk::Image,
    view: vk::ImageView,
    sampler: vk::Sampler,
    allocation: vkm::Allocation,
    device: Arc<ash::Device>,
    allocator: Arc<vk_mem::Allocator>,
}

impl Device {
    pub fn create_texture(&self, format: vk::Format, width: u32, height: u32) -> Texture {
        let allocator = self.allocator.clone();
        let texture_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .usage(vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let allocation_create_info = vkm::AllocationCreateInfo {
            usage: vkm::MemoryUsage::AutoPreferDevice,
            required_flags: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            ..Default::default()
        };

        let (image, allocation) = unsafe {
            allocator
                .create_image(&texture_info, &allocation_create_info)
                .unwrap()
        };

        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1),
            );

        let view = unsafe { self.handle().create_image_view(&view_info, None).unwrap() };

        let sampler_create_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE);

        let sampler = unsafe { self.handle().create_sampler(&sampler_create_info, None) }.unwrap();

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
            println!("DROPPED");

            self.device.destroy_image_view(self.view, None);
            self.device.destroy_sampler(self.sampler, None);
            self.allocator
                .destroy_image(self.image, &mut self.allocation);
        }
    }
}
