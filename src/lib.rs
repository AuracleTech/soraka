pub mod format;
use crate::format::{format_bytes, format_duration};
use format::BYTE;
use std::fs::File;
use std::io::Write;
use std::ptr;
use std::time::Duration;
use winapi::um::wingdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, GetDIBits, SRCCOPY};
use winapi::um::winuser::{GetDC, ReleaseDC};

const RESOLUTION_WIDTH: u32 = 2560;
const RESOLUTION_HEIGHT: u32 = 1440;
const COLOR_CHANNEL_COUNT: u8 = 3;
const COLOR_BIT_DEPTH: u8 = 8;
const FRAME_RATE: u32 = 120;
const REPLAY_BUFFER_DURATION: Duration = Duration::from_secs(30);
const MAX_RAM_USAGE: u64 = 8 * format::GIGABYTE;

const FRAME_BUFFER_BYTES_COUNT: usize = RESOLUTION_WIDTH as usize
    * RESOLUTION_HEIGHT as usize
    * COLOR_CHANNEL_COUNT as usize
    * (COLOR_BIT_DEPTH / BYTE) as usize;
const FRAME_BUFFER_BITS_COUNT: usize = FRAME_BUFFER_BYTES_COUNT * BYTE as usize;
const OMEGA_FRAME_COUNT: usize = FRAME_RATE as usize * REPLAY_BUFFER_DURATION.as_secs() as usize;

pub fn record() -> Result<(), std::io::Error> {
    let mut file = File::create("C:\\Users\\DREAD\\Desktop\\_\\recordings\\output.raw")
        .expect("Unable to create file");

    println!("Resolution: {}x{}", RESOLUTION_WIDTH, RESOLUTION_HEIGHT);
    println!("Channel count: {}", COLOR_CHANNEL_COUNT);
    println!("Color bit depth: {}", COLOR_BIT_DEPTH);

    println!(
        "Bits per frame: {}",
        format_bytes(FRAME_BUFFER_BITS_COUNT as u64)
    );

    println!("Frame rate: {}", FRAME_RATE);
    println!(
        "Replay buffer duration: {}",
        format_duration(REPLAY_BUFFER_DURATION)
    );
    println!(
        "Frame bit size: {}",
        format_bytes(FRAME_BUFFER_BYTES_COUNT as u64)
    );
    println!(
        "Replay buffer size: {}",
        format_bytes(FRAME_BUFFER_BYTES_COUNT as u64 * OMEGA_FRAME_COUNT as u64)
    );

    let mut frame_data = vec![0; FRAME_BUFFER_BYTES_COUNT];

    let temporary = 240;

    let start_time = std::time::Instant::now();

    for _i in 0..temporary {
        unsafe {
            let desktop_dc = GetDC(ptr::null_mut());
            let compatible_dc = CreateCompatibleDC(desktop_dc);
            let bitmap = CreateCompatibleBitmap(
                desktop_dc,
                RESOLUTION_WIDTH as i32,
                RESOLUTION_HEIGHT as i32,
            );
            let old_bitmap = winapi::um::wingdi::SelectObject(compatible_dc, bitmap as *mut _);

            BitBlt(
                compatible_dc,
                0,
                0,
                RESOLUTION_WIDTH as i32,
                RESOLUTION_HEIGHT as i32,
                desktop_dc,
                0,
                0,
                SRCCOPY,
            );

            let mut bitmap_info = std::mem::zeroed::<winapi::um::wingdi::BITMAPINFO>();
            bitmap_info.bmiHeader.biSize =
                std::mem::size_of::<winapi::um::wingdi::BITMAPINFOHEADER>() as u32;
            bitmap_info.bmiHeader.biSizeImage = FRAME_BUFFER_BYTES_COUNT as u32;
            bitmap_info.bmiHeader.biWidth = RESOLUTION_WIDTH as i32;
            bitmap_info.bmiHeader.biHeight = -(RESOLUTION_HEIGHT as i32);
            bitmap_info.bmiHeader.biPlanes = 1;
            bitmap_info.bmiHeader.biBitCount = (COLOR_CHANNEL_COUNT * COLOR_BIT_DEPTH) as u16;
            bitmap_info.bmiHeader.biCompression = winapi::um::wingdi::BI_RGB;

            let result = GetDIBits(
                desktop_dc,
                bitmap,
                0,
                RESOLUTION_HEIGHT as u32,
                frame_data.as_mut_ptr() as *mut _,
                &mut bitmap_info,
                winapi::um::wingdi::DIB_RGB_COLORS,
            );

            if result == 0 {
                println!(
                    "GetDIBits failed with error: {}",
                    std::io::Error::last_os_error()
                );
            }

            winapi::um::wingdi::SelectObject(compatible_dc, old_bitmap);
            ReleaseDC(ptr::null_mut(), desktop_dc);
        }

        file.write_all(&frame_data)?;
    }

    // ASSERT ELAPSED TIME IS PROPER
    let elapsed_time = start_time.elapsed();
    let expected_time = temporary as u32 / FRAME_RATE;
    let expected_time = Duration::from_secs(expected_time as u64);
    assert_eq!(elapsed_time, expected_time);

    // ASSERT FILE SIZE
    let file_size = file.metadata()?.len();
    let expected_file_size = FRAME_BUFFER_BYTES_COUNT as u64 * temporary;
    assert_eq!(file_size, expected_file_size);

    // ffmpeg -f rawvideo -pixel_format bgr24 -video_size 2560x1440 -framerate 120 -i "!input_file!" -c:v libx264 -pix_fmt yuv444p -bf 0 -g 30 "!output_file!"

    Ok(())
}
