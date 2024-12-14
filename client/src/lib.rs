extern crate nalgebra as na;

use std::{collections::HashMap, sync::Arc, time};

use cgpu::{state::GPUState, RenderContext};
use game::{
    brick::BrickMap,
    input::Input,
    material::{ExpandedMaterialMapping, MaterialRegistry},
    palette::PaletteRegistry,
    worldgen::{GeneratedBrick, WorldGenerator},
    Diagnostics,
};
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::KeyCode,
    window::{Window, WindowAttributes, WindowId},
};

mod diagnostics;
mod egui_integration;
mod ui;

use egui_integration::EguiRenderer;

pub struct AppState {
    pub last_update: time::SystemTime,
    pub delta_time: time::Duration,
    pub diagnostics: Diagnostics,
    pub input: game::input::Input,
    pub proxy: EventLoopProxy<AppEvent>,
    pub windows: HashMap<WindowId, Arc<Window>>,

    pub gpu: Arc<GPUState>,
    pub renderers: HashMap<WindowId, Arc<RenderContext>>,
    pub eguis: HashMap<WindowId, Arc<EguiRenderer>>,
    pub scale_factor: f32,
    pub focus: bool,
    pub active_window: Option<Arc<Window>>,

    pub generator: WorldGenerator,
    pub material_mapping: ExpandedMaterialMapping,

    pub brickmap: Arc<BrickMap>,
    pub palettes: Arc<PaletteRegistry>,
    pub materials: Arc<MaterialRegistry>,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    RequestExit,
}

impl AppState {
    pub fn new(event_loop: &EventLoop<AppEvent>) -> Self {
        let materials = Arc::new(MaterialRegistry::new());

        materials.register_default_materials();

        let palettes = Arc::new(PaletteRegistry::new());

        let brickmap_dimensions = na::Vector3::new(128, 64, 128);
        let brickmap = Arc::new(BrickMap::new(brickmap_dimensions));

        let generator = WorldGenerator::new();

        let mut material_mapping = ExpandedMaterialMapping::new();
        material_mapping.add_from_registry(&materials, "air", 0);
        material_mapping.add_from_registry(&materials, "stone", 1);
        material_mapping.add_from_registry(&materials, "bedrock", 2);
        material_mapping.add_from_registry(&materials, "dirt", 3);
        material_mapping.add_from_registry(&materials, "grass", 4);
        material_mapping.add_from_registry(&materials, "snow", 5);

        let gpu = Arc::new(pollster::block_on(GPUState::new(
            brickmap.clone(),
            materials.clone(),
            palettes.clone(),
            512 << 20,
            128 << 20,
        )));

        let dimensions = brickmap.dimensions();

        generator.generate_volume(
            &brickmap,
            na::Point3::origin(),
            na::Point3::from(dimensions),
            na::Point3::new(128, 20, 128),
            100,
            &material_mapping,
            |brick, _at, handle| {
                let brick = match brick {
                    GeneratedBrick::Brick(material_brick) => material_brick,
                    GeneratedBrick::Lod(_lod_material) => {
                        return;
                    }
                    GeneratedBrick::None => return,
                };

                let (material_brick, materials) = brick.compress(&material_mapping);

                let palette = palettes.register_palette(materials);

                let _ = gpu.bricks.allocate_brick(material_brick, handle, palette);
            },
        );

        // sdf::distance_field_parallel_pass(
        //     &brickmap,
        //     na::Point3::zeroed(),
        //     na::Point3::from(dimensions),
        // );

        gpu.bricks.update_all_handles();
        gpu.bricks.update_all_bricks();

        gpu.materials.update_all_materials();
        gpu.materials.update_all_palettes();

        Self {
            last_update: time::SystemTime::now(),
            delta_time: time::Duration::from_nanos(0),
            diagnostics: Diagnostics::new(),
            input: Input::new(),
            proxy: event_loop.create_proxy(),
            windows: HashMap::new(),

            gpu,
            renderers: HashMap::new(),
            eguis: HashMap::new(),
            scale_factor: 1.0,
            focus: true,
            active_window: None,

            generator,
            material_mapping,

            brickmap,
            palettes,
            materials,
        }
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent, window_id: WindowId) {
        self.input.update_window(event);
        if !self.focus {
            if let Some(egui) = self.eguis.get_mut(&window_id) {
                let egui = Arc::get_mut(egui).unwrap();
                egui.handle_input(&event);
            }
        }
    }

    pub fn handle_device_event(&mut self, event: &DeviceEvent) {
        self.input.update(&event);
        self.handle_input();
    }

    pub fn handle_input(&mut self) {
        if self.released(KeyCode::Escape) {
            self.proxy.send_event(AppEvent::RequestExit).unwrap();
        }

        if self.pressed(KeyCode::KeyT) {
            self.focus = !self.focus;
            if let Some(window) = &self.active_window {
                if self.focus {
                    if window
                        .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                        .is_ok()
                    {
                        window.set_cursor_visible(false);
                    } else {
                        log::error!("Failed to grab: {:?}", window.id());
                    }
                } else {
                    if window
                        .set_cursor_grab(winit::window::CursorGrabMode::None)
                        .is_ok()
                    {
                        window.set_cursor_visible(true);
                    } else {
                        log::error!("Failed to ungrab: {:?}", window.id());
                    }
                }
            }
        }

        for (_, renderer) in self.renderers.iter_mut() {
            let renderer = Arc::get_mut(renderer).unwrap();
            if self.focus {
                renderer.update_camera_mouse(self.delta_time, &self.input);
            }
        }
    }

    pub fn render(&mut self, window: &WindowId) {
        self.diagnostics.start("render");

        if let Some(renderer) = self.renderers.get_mut(window) {
            let renderer = Arc::get_mut(renderer).unwrap();
            if self.focus {
                renderer.update_camera_keyboard(self.delta_time, &self.input);
            }
            self.diagnostics.start("vertex");
            renderer.update_uniforms(self.delta_time);
            let _ = renderer.prepare_render();
            renderer.render();

            self.diagnostics.stop("vertex");
        }

        self.diagnostics.start("egui");
        self.draw_egui(window);
        self.diagnostics.stop("egui");

        if let Some(renderer) = self.renderers.get_mut(window) {
            let renderer = Arc::get_mut(renderer).unwrap();

            renderer.finish_render(&mut self.diagnostics);
        }

        self.diagnostics.stop("render");
    }

    pub fn resize(&mut self, window: &WindowId, size: PhysicalSize<u32>) {
        if let Some(render) = self.renderers.get_mut(window) {
            let render = Arc::get_mut(render).unwrap();
            render.resize(size);

            if let Some(egui) = self.eguis.get_mut(window) {
                let _egui = Arc::get_mut(egui).unwrap();
                // TODO
            }
        }
    }

    pub fn pressed(&self, code: KeyCode) -> bool {
        self.input.pressed(code)
    }

    pub fn held(&self, code: KeyCode) -> bool {
        self.input.held(code)
    }

    pub fn released(&self, code: KeyCode) -> bool {
        self.input.released(code)
    }

    pub fn new_window(&mut self, event_loop: &ActiveEventLoop, title: &str) -> Arc<Window> {
        let attribs = WindowAttributes::default()
            .with_inner_size(PhysicalSize::new(1920, 1080))
            .with_title(title);
        let window = match event_loop.create_window(attribs) {
            Ok(window) => window,
            Err(e) => panic!("Error creating window: {:?}", e),
        };

        log::info!("Window created");

        let id = window.id();
        let window = Arc::new(window);
        self.windows.insert(id, window.clone());
        window
    }

    pub fn new_renderer(&mut self, window: Arc<Window>) {
        let renderer = pollster::block_on(RenderContext::new(window.clone(), self.gpu.clone()));

        log::info!("Renderer Created");

        let egui_renderer = EguiRenderer::new(
            &renderer.device,
            renderer.surface_config.format,
            None,
            1,
            window.clone(),
        );

        log::info!("Egui Created");
        let renderer = Arc::new(renderer);
        self.renderers.insert(window.id(), renderer);

        let egui_renderer = Arc::new(egui_renderer);
        self.eguis.insert(window.id(), egui_renderer);
    }

    pub fn new_render_window(&mut self, event_loop: &ActiveEventLoop, title: &str) {
        let window = self.new_window(event_loop, title);
        self.new_renderer(window);
    }
}
