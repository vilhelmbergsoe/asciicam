use crossterm::execute;
use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEvent},
    terminal,
};
use image::{DynamicImage, GrayImage};
use nokhwa::{Camera, CameraFormat, FrameFormat};
use std::fs::File;
use std::io::{stdout, Write};

const CHARSET: &[char] = &[' ', ' ', ' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

fn get_char(l: u8) -> char {
    let idx: usize = ((l as usize * (CHARSET.len() - 1)) as f32 / 255.0).round() as usize;

    CHARSET[idx]
}

fn write_image_buffer(
    image_buffer: &GrayImage,
    out: &mut dyn Write,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf: String = String::with_capacity(
        image_buffer.width() as usize * image_buffer.height() as usize
            + (2 * image_buffer.height()) as usize,
    );

    for y in 0..image_buffer.height() {
        for x in 0..image_buffer.width() {
            let pixel = image::ImageBuffer::get_pixel(image_buffer, x, y).0;

            let l = pixel[0];

            let c = get_char(l);

            buf.push(c);
        }
        buf.push('\r');
        buf.push('\n');
    }

    write!(out, "{buf}")?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let camera_result = Camera::new(
        0,
        Some(CameraFormat::new_from(640, 480, FrameFormat::MJPEG, 30)),
    );

    let mut camera = match camera_result {
        Ok(camera) => camera,
        Err(e) => {
            eprintln!("Problem initializing camera: {e}");
            std::process::exit(1);
        }
    };

    camera.open_stream()?;

    let mut stdout = stdout();

    terminal::enable_raw_mode()?;

    loop {
        let (term_width, term_height) = terminal::size()?;

        let frame: DynamicImage = DynamicImage::ImageRgb8(camera.frame()?);
        let frame: GrayImage = frame
            .resize_exact(
                term_width.into(),
                (term_height - 1).into(),
                image::imageops::FilterType::Nearest,
            )
            .to_luma8();

        if poll(std::time::Duration::from_secs(0))? {
            let event = read()?;

            if let Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                ..
            }) = event
            {
                match c {
                    'q' => break,
                    's' => {
                        let dt = chrono::Utc::now();
                        let mut file = File::create(format!(
                            "asciicam-{}.txt",
                            dt.format("%Y-%m-%d_%H:%M:%S")
                        ))?;
                        write_image_buffer(&frame, &mut file)?;
                    }
                    _ => (),
                }
            };
        }

        execute!(
            stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0)
        )?;

        write_image_buffer(&frame, &mut stdout)?;

        stdout.flush()?;
    }

    terminal::disable_raw_mode()?;

    Ok(())
}
