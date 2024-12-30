use egui::{DragValue, RichText};

use crate::{render::RenderContext, ClientState};

impl ClientState {
    pub fn render_ui(&self, render: &mut RenderContext) {
        let mut pos = { self.camera.lock().position };
        let mut update_camera = false;

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
                })
            });

        egui::Window::new("egui stuff")
            .resizable(true)
            .show(&egui.ctx, |ui| {
                ui.label("This is window 1");
                if ui.button("Click me!").clicked() {
                    println!("Button clicked!");
                }
                ui.text_edit_multiline(&mut String::new());
            });
    }
}
