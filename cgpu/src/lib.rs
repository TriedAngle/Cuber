extern crate nalgebra as na;
use std::sync::Arc;

use anyhow::Result;

mod brickmap;
pub use brickmap::GPUBrickMap;

pub struct GPUContext {
    pub compute_queue: Arc<cvk::Queue>,
    pub transfer_queue: Arc<cvk::Queue>,
    pub render_queue: Arc<cvk::Queue>,
    pub pool: Arc<cvk::DescriptorPool>,
    pub layout: cvk::DescriptorSetLayout,
    pub descriptors: cvk::DescriptorSet,
    pub device: Arc<cvk::Device>,
}

impl GPUContext {
    pub fn new() -> Result<Arc<Self>> {
        let instance = cvk::Instance::new("Cuber", "Cuber Engine")?;

        let formats = &[
            cvk::Format::R8G8B8_UNORM,
            cvk::Format::R8G8B8_SRGB,
            cvk::Format::D16_UNORM,
            cvk::Format::D32_SFLOAT,
            cvk::Format::R32_SFLOAT,
        ];

        let adapters = instance.adapters(formats)?;
        let adapter = adapters[0].clone();
        cvk::utils::print_queues_pretty(&adapter);

        let (device, queues) = cvk::Device::new(
            instance,
            adapter,
            &[
                cvk::QueueRequest {
                    required_flags: cvk::QueueFlags::COMPUTE | cvk::QueueFlags::TRANSFER,
                    exclude_flags: cvk::QueueFlags::GRAPHICS,
                    strict: true,
                    allow_fallback_share: true,
                },
                cvk::QueueRequest {
                    required_flags: cvk::QueueFlags::GRAPHICS | cvk::QueueFlags::TRANSFER,
                    exclude_flags: cvk::QueueFlags::empty(),
                    strict: false,
                    allow_fallback_share: true,
                },
                cvk::QueueRequest {
                    required_flags: cvk::QueueFlags::TRANSFER,
                    exclude_flags: cvk::QueueFlags::COMPUTE | cvk::QueueFlags::GRAPHICS,
                    strict: true,
                    allow_fallback_share: true,
                },
            ],
        )?;

        let compute_queue = queues[0].clone();
        let render_queue = queues[1].clone();
        let transfer_queue = queues[2].clone();

        let layout = device.create_descriptor_set_layout(&cvk::DescriptorSetLayoutInfo {
            label: Some("General Descriptor Set Layout"),
            bindings: &[
                cvk::DescriptorBinding::unique(
                    0,
                    cvk::DescriptorType::StorageBuffer,
                    cvk::ShaderStageFlags::COMPUTE,
                ),
                cvk::DescriptorBinding::unique(
                    1,
                    cvk::DescriptorType::StorageBuffer,
                    cvk::ShaderStageFlags::COMPUTE,
                ),
                cvk::DescriptorBinding::unique(
                    2,
                    cvk::DescriptorType::StorageBuffer,
                    cvk::ShaderStageFlags::COMPUTE,
                ),
                cvk::DescriptorBinding::unique(
                    3,
                    cvk::DescriptorType::StorageBuffer,
                    cvk::ShaderStageFlags::COMPUTE,
                ),
                cvk::DescriptorBinding::array(
                    4,
                    cvk::DescriptorType::StorageImage,
                    10,
                    cvk::ShaderStageFlags::COMPUTE,
                ),
                cvk::DescriptorBinding::array(
                    5,
                    cvk::DescriptorType::SampledImage,
                    10,
                    cvk::ShaderStageFlags::FRAGMENT,
                ),
                cvk::DescriptorBinding::array(
                    6,
                    cvk::DescriptorType::Sampler,
                    10,
                    cvk::ShaderStageFlags::FRAGMENT,
                ),
            ],
            ..Default::default()
        });

        let pool = device.create_descriptor_pool(&cvk::DescriptorPoolInfo {
            max_sets: 1,
            layouts: &[&layout],
            label: Some("General Descriptor Pool"),
            ..Default::default()
        });

        let descriptors = device.create_descriptor_set(pool.clone(), &layout);

        Ok(Arc::new(Self {
            compute_queue,
            transfer_queue,
            render_queue,
            pool,
            layout,
            descriptors,
            device,
        }))
    }
}
