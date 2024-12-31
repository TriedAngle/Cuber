use egui::{DragValue, RichText};

use crate::{render::RenderContext, ClientState};

impl ClientState {
    pub fn render_ui(&self, render: &mut RenderContext) {
        let mut pos = { self.camera.lock().position };
        let mut update_camera = false;
        let mut mode = render.ppc.mode;

        let egui = &mut render.egui;
        egui.begin_frame(&render.window);
        egui::Window::new("Debug")
            .resizable(true)
            .default_open(true)
            .default_pos((0., 0.))
            .show(&egui.ctx, |ui| {
                ui.heading("Debug");
                ui.label(RichText::new("Player").underline());
                ui.horizontal(|ui| {
                    ui.label("Position: ");
                    if ui
                        .add(
                            DragValue::new(&mut pos.x)
                                .speed(0.1)
                                .prefix("x: ")
                                .custom_formatter(|val, _| format!("{:.2}", val)),
                        )
                        .changed()
                    {
                        update_camera = true;
                    }

                    if ui
                        .add(
                            DragValue::new(&mut pos.y)
                                .speed(0.1)
                                .prefix("y: ")
                                .custom_formatter(|val, _| format!("{:.2}", val)),
                        )
                        .changed()
                    {
                        update_camera = true;
                    }

                    if ui
                        .add(
                            DragValue::new(&mut pos.z)
                                .speed(0.1)
                                .prefix("z: ")
                                .custom_formatter(|val, _| format!("{:.2}", val)),
                        )
                        .changed()
                    {
                        update_camera = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Render: ");
                    egui::ComboBox::from_id_source("render_mode")
                        .selected_text(match mode {
                            0 => "Normal",
                            1 => "Normals",
                            2 => "Depth",
                            3 => "Steps",
                            _ => "Unknown",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut mode, 0, "Normal");
                            ui.selectable_value(&mut mode, 1, "Normals");
                            ui.selectable_value(&mut mode, 2, "Depth");
                            ui.selectable_value(&mut mode, 3, "Steps");
                        });
                });

                ui.separator();
                ui.label(RichText::new("World").underline());
                if ui.button("Regenerate Terrain").clicked() {
                    self.generate_terrain();
                }
            });

        render.ppc.mode = mode;
        if update_camera {
            let mut camera = self.camera.lock();
            camera.position = pos;
        }
    }
}
