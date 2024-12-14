use std::sync::Arc;

use game::{brick::BrickMap, material::MaterialRegistry, palette::PaletteRegistry};

use crate::{bricks::BrickState, materials::MaterialState};

pub struct GPUState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,

    pub bricks: Arc<BrickState>,
    pub materials: Arc<MaterialState>,
}

impl GPUState {
    pub async fn new(
        brickmap: Arc<BrickMap>,
        materials: Arc<MaterialRegistry>,
        palettes: Arc<PaletteRegistry>,
        initial_bricks_size: u64,
        initial_palette_size: u64,
    ) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
        {
            Some(adapter) => adapter,
            None => panic!("Error creating GPU Adapter"),
        };

        let features = adapter.features();

        let custom_features = wgpu::Features::empty()
            | wgpu::Features::TIMESTAMP_QUERY
            | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        // TODO: implement this
        //    | wgpu::Features::SHADER_INT64;

        let mut custom_limits = if cfg!(target_arch = "wasm32") {
            wgpu::Limits::downlevel_webgl2_defaults()
        } else {
            wgpu::Limits::default()
        };

        custom_limits.max_storage_buffer_binding_size = 1073741820;
        custom_limits.max_buffer_size = u64::MAX;

        if !features.contains(wgpu::Features::TIMESTAMP_QUERY) {
            panic!("TIMESTAMP QUERY REQUIRED");
        };

        let (device, queue) = match adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: custom_features,
                    required_limits: custom_limits,
                    label: None,
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
        {
            Ok(res) => res,
            Err(e) => panic!("Error requesting device and queue: {:?}", e),
        };

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let bricks = Arc::new(BrickState::new(
            brickmap.clone(),
            device.clone(),
            queue.clone(),
            initial_bricks_size,
        ));

        let materials = Arc::new(MaterialState::new(
            palettes.clone(),
            materials.clone(),
            device.clone(),
            queue.clone(),
            initial_palette_size,
        ));

        Self {
            instance,
            adapter,
            device,
            queue,
            bricks,
            materials,
        }
    }
}
