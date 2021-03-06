#[macro_use]
extern crate clap;
extern crate find_folder;
extern crate image;
extern crate ocl;
extern crate sdl2;


use clap::{Arg, App, AppSettings};
use ocl::{Buffer, Context, Queue, Device, Platform, Program, Kernel, Image};
use ocl::enums::{ImageChannelOrder, ImageChannelDataType, MemObjectType};
use find_folder::Search;
use sdl2::event::Event;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::keyboard::Keycode;
use std::time::Duration;

//#[allow(dead_code)]
fn run() -> ocl::Result<()> {

    let matches = App::new("mandy")
        .version("v0.1")
        .setting(AppSettings::AllowNegativeNumbers)
        .arg(Arg::with_name("width").short("w").takes_value(true))
        .arg(Arg::with_name("height").short("h").takes_value(true))
        .arg(Arg::with_name("mid_x").short("x").takes_value(true))
        .arg(Arg::with_name("mid_y").short("y").takes_value(true))
        .arg(Arg::with_name("zoom").short("z").takes_value(true))
        .arg(Arg::with_name("max").short("m").takes_value(true))
        .get_matches();

    let width = value_t!(matches, "width", u32).unwrap_or(1024);
    let height = value_t!(matches, "height", u32).unwrap_or(600);
    let mut mid_x = value_t!(matches, "mid_x", f64).unwrap_or(0.75);
    let mut mid_y = value_t!(matches, "mid_y", f64).unwrap_or(0.0);
    let mut zoom = value_t!(matches, "zoom", f64).unwrap_or(1.0);
    let mut max = value_t!(matches, "max", u32).unwrap_or(100);

    let dims = (width * height) as usize;
    let mut x_vec = vec![0.0f64; dims];
    let mut y_vec = vec![0.0f64; dims];

    let kernel_src = Search::ParentsThenKids(3, 3)
        .for_folder("kernel")
        .expect("Error locating 'kernel'")
        .join("mandy.cl");

    // (1) Define which platform and device(s) to use. Create a context,
    // queue, and program then define some dims (compare to step 1 above).
    let platform = Platform::default();
    let device = Device::by_idx_wrap(platform, 2)?;

    println!("device: {:#}", device);
    let context = Context::builder()
        .platform(platform)
        .devices(device.clone())
        .build()?;
    let program = Program::builder()
        .devices(device)
        .src_file(kernel_src)
        .build(&context)?;
    let queue = Queue::new(&context, device, None)?;



    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem.window("Mandy", width, height)
        .position_centered()
        .opengl()
        .build()
        .unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut first = true;

    'running: loop {
        let mut keypress : bool = false;
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running
                },
                Event::KeyDown { keycode: Some(Keycode::W), .. } => {
                    mid_y += 0.05 * zoom;
                    keypress = true;
                },
                Event::KeyDown { keycode: Some(Keycode::S), .. } => {
                    mid_y -= 0.05 * zoom;
                    keypress = true;
                },
                Event::KeyDown { keycode: Some(Keycode::A), .. } => {
                    mid_x += 0.05 * zoom;
                    keypress = true;
                },
                Event::KeyDown { keycode: Some(Keycode::D), .. } => {
                    mid_x -= 0.05 * zoom;
                    keypress = true;
                },
                Event::KeyDown { keycode: Some(Keycode::I), .. } => {
                    zoom *= 0.8;
                    keypress = true;
                },
                Event::KeyDown { keycode: Some(Keycode::O), .. } => {
                    zoom *= 1.2;
                    keypress = true;
                },
                Event::KeyDown { keycode: Some(Keycode::Equals), .. } => {
                    max = max + 1;
                    println!("max: {}", max);
                    keypress = true;
                },
                Event::KeyDown { keycode: Some(Keycode::Minus), .. } => {
                    max -= 1;
                    keypress = true;
                },
                Event::KeyDown { keycode: Some(Keycode::P), .. } => {
                    // write_png(&img);
                },
                _ => {}
            }
        }

        if keypress || first {
            first = false;
            // fill x/y to Mandelbrot coordinates
            let height_vs_width = height as f64 / width as f64;
            let scale_h = height as f64 * 0.5 / height as f64 * 3.5 * height_vs_width * zoom;
            let scale_w =  width as f64 * 0.5 /  width as f64 * 3.5 * zoom;
            let left = -scale_w - mid_x;
            let right = scale_w - mid_x;
            let top = -scale_h - mid_y;
            let bottom = scale_h - mid_y;
            let step_x = (right - left) / (width as f64 - 1.0);
            let step_y = (bottom - top) / (height as f64 - 1.0);

            let mut y = top;
            for h in 0..height {
                let mut x = left;
                for w in 0..width {
                    let offset = (h * width + w) as usize;
                    x_vec[offset] = x;
                    y_vec[offset] = y;
                    x += step_x;
                }
                y += step_y;
            }

            let max_adj = (1.0f64 / zoom).log10().abs() as u32 * 8;

            let x_buffer = unsafe { 
                Buffer::<f64>::builder()
                    .queue(queue.clone())
                    .len(dims)
                    .use_host_slice(&x_vec)
                    .build().unwrap()
            };

            let y_buffer = unsafe { 
                Buffer::<f64>::builder()
                    .queue(queue.clone())
                    .len(dims)
                    .use_host_slice(&y_vec)
                    .build()
                    .unwrap()
            };

            let mut img = image::ImageBuffer::from_pixel(width, height, image::Rgba([0, 0, 0, 255u8]));
            let dst_image = unsafe {
                Image::<u8>::builder()
                    .channel_order(ImageChannelOrder::Rgba)
                    .channel_data_type(ImageChannelDataType::UnormInt8)
                    .image_type(MemObjectType::Image2d)
                    .dims(&img.dimensions())
                    .use_host_slice(&img)
                    .queue(queue.clone())
                    .build().unwrap()
            };
            // run opencl kernel
            // FIXME: we shouldn't recompile the kernel per frame
            // It's this way because changes to `max` aren't available to the kernel
            // for some reason.
            let kernel = Kernel::builder()
                .program(&program)
                .queue(queue.clone())
                .global_work_size(dims)
                .name("mandy").arg(&x_buffer).arg(&y_buffer).arg(max + max_adj).arg(width).arg(&dst_image)
                .build()?;
            unsafe { kernel.enq()? }

            // copy results back to img
            dst_image.read(&mut img).enq().unwrap();
            println!("-x {} -y {} -z {} -m {}", mid_x, mid_y, zoom, max);

            let mut surface = window.surface(&event_pump).unwrap();
            for (x, y, foo) in img.enumerate_pixels() {
                surface.fill_rect(Rect::new(x as i32, y as i32, 1, 1), Color::RGB(foo[0], foo[1], foo[2])).unwrap();
            }
            surface.finish().unwrap();
        }
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
    Ok(())
}

#[allow(dead_code)]
fn write_png(img : &image::ImageBuffer<image::Rgba<u8>, Vec<u8>>) {
    let mut path = std::env::current_dir().unwrap();
    path.push("result.png");
    println!("saving image to {}", path.display());
    img.save(path.to_str().unwrap()).unwrap();
}

fn main() {
    run().unwrap();
}
