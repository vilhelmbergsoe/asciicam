use image::{DynamicImage, GrayImage};
use nokhwa::{Camera, CameraFormat, FrameFormat};
use std::fs::File;
use std::io::{stdout, Read, Result, Write};
use termion::async_stdin;
use termion::raw::IntoRawMode;

const CHARSET: &[char] = &[' ', ' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

fn get_char(l: u8) -> char {
    let idx = ((l as f32 - 0.0) * ((CHARSET.len() as f32 - 1.0 - 0.0) / (255.0 - 0.0)) + 0.0)
        .round() as usize;

    CHARSET[idx]
}

fn write_image_buffer(image_buffer: &GrayImage, out: &mut dyn Write) -> Result<()> {
    for y in 0..image_buffer.height() {
        let mut line = String::new(); // This is to reduce write syscalls

        for x in 0..image_buffer.width() {
            let pixel = image_buffer.get_pixel(x, y).0;

            let l = pixel[0];

            let c = get_char(l);

            line.push(c);
        }
        write!(out, "{}\r\n", line)?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let camera = Camera::new(
        0,                                                              // index
        Some(CameraFormat::new_from(640, 480, FrameFormat::MJPEG, 30)), // format
    );

    let mut camera = match camera {
        Ok(camera) => camera,
        Err(e) => {
            eprintln!("Problem initializing camera: {}", e);
            std::process::exit(1);
        }
    };

    camera.open_stream().expect("Problem opening stream");

    let stdout = stdout();
    let mut stdout = stdout.lock().into_raw_mode().unwrap();
    let mut stdin = async_stdin().bytes();

    loop {
        let term_size = termion::terminal_size().expect("Could not get terminal size");

        let frame: DynamicImage = DynamicImage::ImageRgb8(camera.frame().unwrap());
        let frame: GrayImage = frame
            .resize_exact(
                term_size.0.into(),
                (term_size.1 - 1).into(),
                image::imageops::FilterType::Gaussian,
            )
            .to_luma8();

        let b = stdin.next();

        match b {
            Some(Ok(b'q')) => break,
            Some(Ok(b's')) => {
                let dt = chrono::Utc::now();
                let mut file =
                    File::create(format!("asciicam-{}.txt", dt.format("%Y-%m-%d_%H:%M:%S")))
                        .unwrap();
                write_image_buffer(&frame, &mut file).unwrap();
            }
            _ => (),
        }

        write!(
            stdout,
            "{}{}",
            termion::clear::All,
            termion::cursor::Goto(1, 1)
        )
        .unwrap();

        write_image_buffer(&frame, &mut stdout).unwrap();

        stdout.flush().unwrap();
    }

    Ok(())
}
