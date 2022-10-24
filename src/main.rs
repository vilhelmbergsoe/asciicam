use crossterm::execute;
use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEvent},
    terminal,
};
use fast_image_resize as fr;
use image::{DynamicImage, GrayImage};
use std::fs::File;
use std::io::{stdout, Write};
use std::num::NonZeroU32;
use v4l::{
    buffer::Type, io::mmap::Stream, io::traits::CaptureStream, video::Capture, Device, FourCC,
};

// the extra char is to avoid floating point arithmetic and won't be displayed
const CHARSET: &[char] = &[
    ' ', ' ', ' ', '.', ':', '-', '=', '+', '*', '#', '%', '@', '?',
];

const fn get_char(l: u8) -> char {
    // this should always truncate which means the last char in CHARSET won't be reached
    // this is done to avoid floating point arithmetic, which is expensive
    let idx: usize = (l as usize * (CHARSET.len() - 1)) / 255_usize;

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
        for x in (0..image_buffer.width()).rev() {
            // this flips the image
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
    let dev = Device::new(0)?;

    let mut fmt = dev.format()?;

    fmt.fourcc = FourCC::new(b"MJPG");
    dev.set_format(&fmt)?;

    let mut stream = Stream::with_buffers(&dev, Type::VideoCapture, 4)?;

    let mut stdout = stdout();

    terminal::enable_raw_mode()?;

    loop {
        let (term_width, term_height) = terminal::size()?;

        let (buf, _) = stream.next()?;

        let decoder = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS).from_mem(buf)?;
        let mut img = decoder.grayscale()?;

        let raw_pixels = match img.read_scanlines() {
            None => {
                terminal::disable_raw_mode()?;
                return Err("Could not decompress image".into());
            }
            Some(v) => v,
        };

        img.finish_decompress();

        let src_frame = fr::Image::from_vec_u8(
            match NonZeroU32::new(fmt.width) {
                None => {
                    terminal::disable_raw_mode()?;
                    return Err("Could not create NonZeroU32".into());
                }
                Some(v) => v,
            },
            match NonZeroU32::new(fmt.height) {
                None => {
                    terminal::disable_raw_mode()?;
                    return Err("Could not create NonZeroU32".into());
                }
                Some(v) => v,
            },
            raw_pixels,
            fr::PixelType::U8,
        )?;

        let dst_width = match NonZeroU32::new(term_width.into()) {
            None => {
                terminal::disable_raw_mode()?;
                return Err("Could not create NonZeroU32".into());
            }
            Some(v) => v,
        };

        let dst_height = match NonZeroU32::new(term_height.into()) {
            None => {
                terminal::disable_raw_mode()?;
                return Err("Could not create NonZeroU32".into());
            }
            Some(v) => v,
        };

        let mut dst_frame = fr::Image::new(dst_width, dst_height, src_frame.pixel_type());

        let mut dst_view = dst_frame.view_mut();

        let mut resizer = fr::Resizer::new(fr::ResizeAlg::Nearest);

        match resizer.resize(&src_frame.view(), &mut dst_view) {
            Ok(_) => (),
            Err(e) => {
                terminal::disable_raw_mode()?;
                return Err(e.into());
            }
        };

        let frame: GrayImage = DynamicImage::ImageLuma8(
            match image::ImageBuffer::from_raw(
                dst_width.get(),
                dst_height.get(),
                dst_frame.buffer().to_vec(),
            ) {
                None => {
                    terminal::disable_raw_mode()?;
                    return Err("Could not convert raw buffer to image buffer".into());
                }
                Some(v) => v,
            },
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
