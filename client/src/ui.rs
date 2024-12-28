use crate::{render::RenderContext, ClientState};

impl ClientState {
    pub fn render_ui(&self, render: &mut RenderContext) {
        let egui = &mut render.egui;
        egui.begin_frame(&render.window);
        egui::Window::new("Debug")
            .resizable(true)
            .show(&egui.ctx, |ui| {
                ui.label("Hello from egui!");
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
