use glow::*;
use glutin::event::{Event, WindowEvent};
use glutin::event_loop::ControlFlow;

use fontdue::{Font, layout::{ Layout, CoordinateSystem, LayoutSettings, TextStyle } };
use image::{GrayImage, Luma};
use liverking::natty;
use std::fs;

fn main() {
    natty! {
        let (gl, shader_version, window, event_loop) = {
            let event_loop = glutin::event_loop::EventLoop::new();
            let window_builder = glutin::window::WindowBuilder::new()
                .with_title("Hello triangle!")
                .with_inner_size(glutin::dpi::LogicalSize::new(1024.0, 768.0));
            let window = glutin::ContextBuilder::new()
                .with_vsync(true)
                .build_windowed(window_builder, &event_loop)
                .unwrap()
                .make_current()
                .unwrap();
            let gl =
                glow::Context::from_loader_function(|s| window.get_proc_address(s) as *const _);
            (gl, "#version 410", window, event_loop)
        };

        let vertex_array = gl
            .create_vertex_array()
            .expect("Cannot create vertex array");
        gl.bind_vertex_array(Some(vertex_array));

        let program = gl.create_program().expect("Cannot create program");

        let (vertex_shader_source, fragment_shader_source) = (
            r#"const vec2 verts[3] = vec2[3](
                vec2(0.5f, 1.0f),
                vec2(0.0f, 0.0f),
                vec2(1.0f, 0.0f)
            );
            out vec2 vert;
            void main() {
                vert = verts[gl_VertexID];
                gl_Position = vec4(vert - 0.5, 0.0, 1.0);
            }"#,
            r#"precision mediump float;
            in vec2 vert;
            out vec4 color;
            void main() {
                color = vec4(vert, 0.5, 1.0);
            }"#,
        );

        let shader_sources = [
            (glow::VERTEX_SHADER, vertex_shader_source),
            (glow::FRAGMENT_SHADER, fragment_shader_source),
        ];

        let mut shaders = Vec::with_capacity(shader_sources.len());

        for (shader_type, shader_source) in shader_sources.iter() {
            let shader = gl
                .create_shader(*shader_type)
                .expect("Cannot create shader");
            gl.shader_source(shader, &format!("{}\n{}", shader_version, shader_source));
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                panic!("{}", gl.get_shader_info_log(shader));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }

        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            panic!("{}", gl.get_program_info_log(program));
        }

        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }

        
        let text = "meowwy 猫🐱 XD";
        let font_file = load_font_file("Arial.ttf");
        let font = Font::from_bytes(font_file, fontdue::FontSettings::default()).unwrap();
        let fonts = &[&font];

        let mut layout = Layout::new(CoordinateSystem::PositiveYUp);
        layout.append(fonts, &TextStyle::new(text, 42.0, 0));
        let glyphs = layout.glyphs();

        let ( mut total_width, mut total_height) = (0usize, 0usize);
        for glyph in glyphs {
            let padding = glyph.x as usize - total_width;
            total_width += glyph.width;
            total_width += padding;
            if glyph.height > total_height { total_height = glyph.height }; 
        }
        
        println!("total_width: {}, total_height: {}", total_width, total_height);
        let mut render_text = vec![0u8; total_width * total_height];
        for glyph in glyphs {
            let (_metrics, bitmap) = font.rasterize(glyph.parent, glyph.key.px);
            let (width, _) = (glyph.width, glyph.height);
            
            println!("glyph: {}, x: {}, y: {}, width: {}, height: {}", glyph.parent, glyph.x, glyph.y, glyph.width, glyph.height);
            for sub_y in 0..glyph.height {
                for sub_x in 0..glyph.width {
                    let image_index = (sub_y) * total_width + (glyph.x as usize + sub_x);
                    let glyph_index = sub_y * width + sub_x;
                    render_text[image_index] = bitmap[glyph_index];
                }
            }
        }


        let mut img = GrayImage::new(total_width as u32, total_height as u32);
        for y in 0..total_height {
            for x in 0..total_width {
                let pixel_index = y * total_width + x;
                let pixel_value = render_text[pixel_index];
                img.put_pixel(x as u32, y as u32, Luma([pixel_value]));
            }
        }
        img.save("out/merge.png").unwrap();
        

        
        gl.clear_color(0.1, 0.2, 0.3, 1.0);
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::LoopDestroyed => {
                    return;
                }
                Event::MainEventsCleared => {
                    window.window().request_redraw();
                }
                Event::RedrawRequested(_) => {
                    gl.clear(glow::COLOR_BUFFER_BIT);
                    gl.use_program(Some(program));
                    gl.clear_color(0.1, 0.2, 0.3, 1.0);
                    gl.draw_arrays(glow::TRIANGLES, 0, 3);

                    window.swap_buffers().unwrap();
                }
                Event::WindowEvent { ref event, .. } => match event {
                    WindowEvent::Resized(physical_size) => {
                        window.resize(*physical_size);
                    }
                    WindowEvent::CloseRequested => {
                        gl.delete_program(program);
                        gl.delete_vertex_array(vertex_array);
                        *control_flow = ControlFlow::Exit
                    }
                    _ => (),
                },
                _ => (),
            }
        });
    }

}

fn load_font_file(path: &str) -> Vec<u8> {
    if path.starts_with(".") || path.starts_with("/") {
        return fs::read(path).unwrap();
    } else {
        #[cfg(target_os = "windows")]
        let path = format!("C:/Windows/Fonts/{}", path);
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let path = format!("/usr/share/fonts/truetype/{}", path);
        return fs::read(&path).unwrap();
    }
}