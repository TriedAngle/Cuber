use crate::{Adapter, Buffer, BufferInfo, CommandRecorder, Device, Image, ImageTransition};
use ash::vk;
use rand::Rng;

pub fn print_queues_pretty(adapter: &Adapter) {
    println!("Queue Families:");

    for (index, queue_family) in adapter.queue_properties.iter().enumerate() {
        println!("\tQueue Family {}:", index);
        println!("\t\tQueue Count: {}", queue_family.queue_count);

        // Print queue flags
        let flags = queue_family.queue_flags;
        println!("\t\tQueue Flags:");
        if flags.contains(vk::QueueFlags::GRAPHICS) {
            println!("\t\t\tGRAPHICS");
        }
        if flags.contains(vk::QueueFlags::COMPUTE) {
            println!("\t\t\tCOMPUTE");
        }
        if flags.contains(vk::QueueFlags::TRANSFER) {
            println!("\t\t\tTRANSFER");
        }
        if flags.contains(vk::QueueFlags::SPARSE_BINDING) {
            println!("\t\t\tSPARSE_BINDING");
        }
        if flags.contains(vk::QueueFlags::PROTECTED) {
            println!("\t\t\tPROTECTED");
        }

        println!(
            "\t\tTimestamp Valid Bits: {}",
            queue_family.timestamp_valid_bits
        );
        println!(
            "\t\tMin Image Transfer Granularity: {:?}",
            queue_family.min_image_transfer_granularity
        );
    }
}

pub fn fill_texture_squares(
    device: &Device,
    recorder: &mut CommandRecorder,
    image: &Image,
    size: i32,
) -> Buffer {
    let width = image.details().width;
    let height = image.details().height;
    let square_size = size;
    let squares_x = (width as i32 + square_size - 1) / square_size;
    let squares_y = (height as i32 + square_size - 1) / square_size;
    let mut rng = rand::thread_rng();

    let mut color_data = vec![0u8; (width * height * 4) as usize];

    for sy in 0..squares_y {
        for sx in 0..squares_x {
            let r = rng.gen::<u8>();
            let g = rng.gen::<u8>();
            let b = rng.gen::<u8>();
            let a = 255; // Full opacity

            for y in 0..square_size {
                let py = sy * square_size + y;
                if py >= height as i32 {
                    continue;
                }

                for x in 0..square_size {
                    let px = sx * square_size + x;
                    if px >= width as i32 {
                        continue;
                    }

                    let idx = ((py * width as i32 + px) * 4) as usize;
                    color_data[idx] = r;
                    color_data[idx + 1] = g;
                    color_data[idx + 2] = b;
                    color_data[idx + 3] = a;
                }
            }
        }
    }

    let staging_buffer = device.create_buffer(&BufferInfo {
        size: color_data.len() as u64,
        usage: vk::BufferUsageFlags::TRANSFER_SRC,
        sharing: vk::SharingMode::EXCLUSIVE,
        usage_locality: vkm::MemoryUsage::AutoPreferHost,
        allocation_locality: vk::MemoryPropertyFlags::HOST_VISIBLE
            | vk::MemoryPropertyFlags::HOST_COHERENT,
        host_access: Some(vkm::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE),
        label: Some("Random Squares Texture Upload Buffer"),
        ..Default::default()
    });

    staging_buffer.upload(&color_data, 0);

    recorder.image_transition(
        image,
        ImageTransition::Custom {
            // old_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            // src_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
            dst_stage: vk::PipelineStageFlags::TRANSFER,
            // src_access: vk::AccessFlags::SHADER_WRITE,
            dst_access: vk::AccessFlags::TRANSFER_WRITE,
        },
    );

    recorder.copy_buffer_image(
        &staging_buffer,
        &image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        &[vk::BufferImageCopy {
            buffer_offset: 0,
            buffer_row_length: 0,
            buffer_image_height: 0,
            image_subresource: vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
            image_extent: vk::Extent3D {
                width,
                height,
                depth: 1,
            },
        }],
    );

    staging_buffer
}
