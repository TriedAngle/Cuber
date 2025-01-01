use std::sync::Arc;

use egui_wgpu as egpu;
use egui_winit as ewin;
use winit::event::WindowEvent;
use winit::window::Window;

pub struct EguiRenderer {
    pub state: ewin::State,
    pub renderer: egpu::Renderer,
    pub frame_started: bool,
    pub device: Arc<egpu::wgpu::Device>,
    pub queue: Arc<egpu::wgpu::Queue>,
    pub window: Arc<Window>,
}

impl EguiRenderer {
    pub fn context(&self) -> &egui::Context {
        self.state.egui_ctx()
    }

    pub fn new(
        device: Arc<egpu::wgpu::Device>,
        queue: Arc<egpu::wgpu::Queue>,
        output_color_format: egpu::wgpu::TextureFormat,
        output_depth_format: Option<egpu::wgpu::TextureFormat>,
        msaa_samples: u32,
        window: Arc<Window>,
    ) -> Self {
        let context = egui::Context::default();
        log::debug!("Egui Context created");
        let state = ewin::State::new(
            context,
            egui::viewport::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            Some(2 * 1024),
        );
        log::debug!("Egui State created");

        let renderer = egpu::Renderer::new(
            &device,
            output_color_format,
            output_depth_format,
            msaa_samples,
            true,
        );
        log::debug!("Egui Renderer created");

        Self {
            device,
            queue,
            state,
            renderer,
            frame_started: false,
            window,
        }
    }

    pub fn handle_input(&mut self, event: &WindowEvent) {
        let _ = self.state.on_window_event(&self.window, event);
    }

    pub fn ppp(&mut self, v: f32) {
        self.context().set_pixels_per_point(v);
    }

    pub fn begin_frame(&mut self) {
        let raw_input = self.state.take_egui_input(&self.window);
        self.state.egui_ctx().begin_pass(raw_input);
        self.frame_started = true;
    }

    pub fn end_frame_and_draw(
        &mut self,
        encoder: &mut egpu::wgpu::CommandEncoder,
        window_surface_view: &egpu::wgpu::TextureView,
        screen_descriptor: egpu::ScreenDescriptor,
    ) {
        if !self.frame_started {
            panic!("begin_frame must be called before end_frame_and_draw can be called!");
        }

        self.ppp(screen_descriptor.pixels_per_point);

        let full_output = self.state.egui_ctx().end_pass();

        self.state
            .handle_platform_output(&self.window, full_output.platform_output);

        let tris = self
            .state
            .egui_ctx()
            .tessellate(full_output.shapes, self.state.egui_ctx().pixels_per_point());
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }
        self.renderer.update_buffers(
            &self.device,
            &self.queue,
            encoder,
            &tris,
            &screen_descriptor,
        );
        let rpass = encoder.begin_render_pass(&egpu::wgpu::RenderPassDescriptor {
            color_attachments: &[Some(egpu::wgpu::RenderPassColorAttachment {
                view: window_surface_view,
                resolve_target: None,
                ops: egui_wgpu::wgpu::Operations {
                    load: egui_wgpu::wgpu::LoadOp::Load,
                    store: egpu::wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            label: Some("egui main render pass"),
            occlusion_query_set: None,
        });

        self.renderer
            .render(&mut rpass.forget_lifetime(), &tris, &screen_descriptor);
        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x)
        }

        self.frame_started = false;
    }
}
