use crate::AppState;

use egui::{Button, Color32, DragValue, Pos2, RichText, Stroke, Vec2};
use std::sync::Arc;
use winit::window::WindowId;

impl AppState {
    pub fn draw_egui(&mut self, window: &WindowId) {
        let Some(renderer) = self.renderers.get_mut(window) else {
            return;
        };
        if let Some(egui_renderer) = self.eguis.get_mut(window) {
            let renderer = Arc::get_mut(renderer).unwrap();
            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [
                    renderer.surface_config.width,
                    renderer.surface_config.height,
                ],
                pixels_per_point: egui_renderer.window.scale_factor() as f32 * self.scale_factor,
            };

            let mut encoder = renderer.encoder.as_mut().expect("Render must be prepared");
            let output = renderer
                .output_texture
                .as_ref()
                .expect("Render must be prepared");

            let view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let egui_renderer = Arc::get_mut(egui_renderer).unwrap();
            egui_renderer.begin_frame();
            let mut cycle_renderer = false;
            let mut updated_camera = false;
            let mut toggle_sdf = false;

            egui::CentralPanel::default()
                .frame(egui::Frame::none())
                .show(egui_renderer.context(), |ui| {
                    let painter = ui.painter();

                    let screen_rect = ui.max_rect();
                    let center = screen_rect.center();

                    let crosshair_size = 10.0;
                    let stroke = Stroke::new(2.0, Color32::WHITE);

                    painter.line_segment(
                        [
                            Pos2::new(center.x - crosshair_size, center.y),
                            Pos2::new(center.x + crosshair_size, center.y),
                        ],
                        stroke,
                    );

                    painter.line_segment(
                        [
                            Pos2::new(center.x, center.y - crosshair_size),
                            Pos2::new(center.x, center.y + crosshair_size),
                        ],
                        stroke,
                    );
                });

            egui::Window::new("Debug")
                .resizable(true)
                .default_size(Vec2::new(200.0, 100.0))
                .default_open(true)
                .show(egui_renderer.context(), |ui| {
                    ui.heading("Game Settings");

                    ui.separator();
                    ui.heading("Debug");

                    ui.label(format!(
                        "Total Render: {:.3}ms",
                        self.diagnostics.time_millis("render")
                    ));
                    ui.label(format!(
                        "Compute Pass Time: {:.3}ms",
                        self.diagnostics.time_millis("ComputePass")
                    ));
                    ui.label(format!(
                        "Rasterization Pass Time: {:.3}ms",
                        self.diagnostics.time_millis("RasterPass")
                    ));
                    ui.label(format!(
                        "Egui Time: {:.3}ms",
                        self.diagnostics.time_millis("egui")
                    ));

                    ui.separator();
                    ui.heading("Tools");
                    if ui.add(Button::new("Cycle render mode")).clicked() {
                        cycle_renderer = true;
                    }
                    if ui.add(Button::new("Toggle SDF accelleration")).clicked() {
                        toggle_sdf = true;
                    }
                    ui.label(RichText::new("Player:").underline());
                    ui.horizontal(|ui| {
                        ui.label("Position: ");

                        if ui
                            .add(
                                DragValue::new(&mut self.camera.position.x)
                                    .speed(0.1)
                                    .prefix("x: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {
                            updated_camera = true;
                        }

                        if ui
                            .add(
                                DragValue::new(&mut self.camera.position.y)
                                    .speed(0.1)
                                    .prefix("y: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {
                            updated_camera = true;
                        }

                        if ui
                            .add(
                                DragValue::new(&mut self.camera.position.z)
                                    .speed(0.1)
                                    .prefix("z: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {
                            updated_camera = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Rotation: ");
                        let (mut roll, mut pitch, mut yaw) = self.camera.rotation.euler_angles();

                        roll = roll.to_degrees();
                        pitch = pitch.to_degrees();
                        yaw = yaw.to_degrees();

                        if ui
                            .add(
                                DragValue::new(&mut yaw)
                                    .speed(1.0)
                                    .prefix("Roll: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {
                            updated_camera = true;
                        }

                        if ui
                            .add(
                                DragValue::new(&mut roll)
                                    .speed(1.0)
                                    .prefix("Pitch: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {
                            updated_camera = true;
                        }

                        if ui
                            .add(
                                DragValue::new(&mut pitch)
                                    .speed(1.0)
                                    .prefix("Yaw: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {
                            updated_camera = true;
                        }

                        if updated_camera {
                            roll = roll.to_radians();
                            pitch = pitch.to_radians();
                            yaw = yaw.to_radians();
                            self.camera.rotation =
                                na::UnitQuaternion::from_euler_angles(roll, pitch, yaw);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Other: ");

                        if ui
                            .add(
                                DragValue::new(&mut self.camera.speed)
                                    .speed(1.0)
                                    .prefix("Speed: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {}
                        if ui
                            .add(
                                DragValue::new(&mut self.camera.fov)
                                    .speed(1.0)
                                    .prefix("Fov: ")
                                    .custom_formatter(|val, _| format!("{:.2}", val)),
                            )
                            .changed()
                        {}
                    });

                    ui.separator();
                    ui.heading("UI Settings");
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Pixels per point: {}",
                            egui_renderer.context().pixels_per_point()
                        ));
                        if ui.button("-").clicked() {
                            self.scale_factor = (self.scale_factor - 0.1).max(0.3);
                        }
                        if ui.button("+").clicked() {
                            self.scale_factor = (self.scale_factor + 0.1).min(3.0);
                        }
                    });
                });

            egui_renderer.end_frame_and_draw(&mut encoder, &view, screen_descriptor);

            if cycle_renderer {
                renderer.cycle_compute_render_mode();
            }

            if toggle_sdf {
                renderer.compute_uniforms.toggle_sdf();
            }

            if updated_camera {
                self.camera.force_udpate();
                renderer.update_uniforms(&self.camera, self.delta_time);
            }
        }
    }
}