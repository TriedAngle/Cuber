use glow::*;
use glutin::event::{Event, WindowEvent};
use glutin::event_loop::ControlFlow;
use glyphers;
use image::{GrayImage, Luma};
use liverking::natty;

fn main() {
    natty! {
        let (gl, _shader_version, window, event_loop) = {
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
            r#"#version 460
            void main() {
              float x = -1.0 + float((gl_VertexID & 1) << 2);
              float y = -1.0 + float((gl_VertexID & 2) << 1);
              gl_Position = vec4(x, y, 0.0, 1.0);
            }"#,
            r#"#version 460
            uniform uint text_offset;
            uniform vec2 text_pos;
            uniform vec2 text_dim;

            layout(std430, binding = 0) buffer Texts {
                float texts[];
            };

            out vec4 FragColor;

            void main() {
                vec2 text_top_right = text_pos + text_dim;
                if (gl_FragCoord.x >= text_pos.x && gl_FragCoord.x <= text_top_right.x && gl_FragCoord.y >= text_pos.y && gl_FragCoord.y <= text_top_right.y) {
                    uint x_index = uint(gl_FragCoord.x - text_pos.x);
                    uint y_index = uint(text_dim.y - (gl_FragCoord.y - text_pos.y) - 1);

                    uint index = text_offset + x_index + y_index * uint(text_dim.x);
                    
                    float grayscale = texts[index];
                    FragColor = vec4(vec3(grayscale), 1.0);
                } else {
                    FragColor = vec4(0.0, 0.0, 0.0, 0.0);
                }
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
            gl.shader_source(shader,  shader_source);
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
        let fonts = ["Arial.ttf", "msyh.ttc", "seguiemj.ttf"];
        let (out, width, height) = glyphers::rasterize(text, &fonts);

        let mut img = GrayImage::new(width as u32, height as u32);
        for y in 0..height {
            for x in 0..width {
                let pixel_index = y * width + x;
                let pixel_value = out[pixel_index];
                img.put_pixel(x as u32, y as u32, Luma([pixel_value]));
            }
        }
        img.save("out/merge.png").unwrap();

        let normalized: Vec<f32> = out.iter().map(|&val| val as f32 / 256.0).collect();

        let buffer = gl.create_named_buffer().unwrap();
        gl.named_buffer_data_u8_slice(buffer, bytemuck::cast_slice(&normalized), glow::DYNAMIC_DRAW);

        let text_offset_loc = gl.get_uniform_location(program, "text_offset").unwrap();
        let text_pos_loc = gl.get_uniform_location(program, "text_pos").unwrap();
        let text_dim_loc = gl.get_uniform_location(program, "text_dim").unwrap();

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
                    gl.clear_color(0.1, 0.2, 0.3, 1.0);
                    gl.clear(glow::COLOR_BUFFER_BIT);

                    gl.use_program(Some(program));
                    gl.bind_buffer_base(glow::SHADER_STORAGE_BUFFER, 0, Some(buffer));
                    gl.uniform_1_u32(Some(&text_offset_loc), 0);
                    gl.uniform_2_f32(Some(&text_pos_loc), 600.0, 400.0);
                    gl.uniform_2_f32(Some(&text_dim_loc), width as f32, height as f32);

                    gl.draw_arrays(glow::TRIANGLES, 0, 4);

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
