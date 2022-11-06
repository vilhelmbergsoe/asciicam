use crossterm::execute;
use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEvent},
    terminal,
};
use eyre::{eyre, Result};
use fast_image_resize as fr;
use image::GrayImage;
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

fn write_image_buffer(image_buffer: &GrayImage, out: &mut dyn Write) -> Result<()> {
    let mut buf: String = String::with_capacity(
        image_buffer.width() as usize * image_buffer.height() as usize
            + (2 * image_buffer.height()) as usize,
    );

    for y in 0..image_buffer.height() {
        // this flips the image
        for x in (0..image_buffer.width()).rev() {
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

fn get_cam(
    stream_buf: &[u8],
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
) -> Result<GrayImage> {
    let decoder = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS).from_mem(stream_buf)?;
    let mut img = decoder.grayscale()?;

    let raw_pixels = match img.read_scanlines() {
        None => {
            return Err(eyre!("Could not decompress image"));
        }
        Some(v) => v,
    };

    img.finish_decompress();

    let src_frame = fr::Image::from_vec_u8(
        match NonZeroU32::new(src_width) {
            None => {
                return Err(eyre!("Could not create NonZeroU32"));
            }
            Some(v) => v,
        },
        match NonZeroU32::new(src_height) {
            None => {
                return Err(eyre!("Could not create NonZeroU32"));
            }
            Some(v) => v,
        },
        raw_pixels,
        fr::PixelType::U8,
    )?;

    let dst_width = match NonZeroU32::new(dst_width) {
        None => {
            return Err(eyre!("Could not create NonZeroU32"));
        }
        Some(v) => v,
    };

    let dst_height = match NonZeroU32::new(dst_height) {
        None => {
            return Err(eyre!("Could not create NonZeroU32"));
        }
        Some(v) => v,
    };

    let mut dst_frame = fr::Image::new(dst_width, dst_height, src_frame.pixel_type());

    let mut dst_view = dst_frame.view_mut();

    let mut resizer = fr::Resizer::new(fr::ResizeAlg::Nearest);

    match resizer.resize(&src_frame.view(), &mut dst_view) {
        Ok(_) => (),
        Err(e) => {
            return Err(e.into());
        }
    };

    let frame: GrayImage = match image::ImageBuffer::from_raw(
        dst_width.get(),
        dst_height.get(),
        dst_frame.buffer().to_vec(),
    ) {
        None => {
            return Err(eyre!("Could not convert raw buffer to image buffer"));
        }
        Some(v) => v,
    };

    Ok(frame)
}

fn main() -> Result<()> {
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

        let frame: GrayImage = match get_cam(
            buf,
            fmt.width,
            fmt.height,
            term_width.into(),
            term_height.into(),
        ) {
            Ok(frame) => frame,
            Err(e) => {
                terminal::disable_raw_mode()?;
                return Err(e);
            }
        };

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
