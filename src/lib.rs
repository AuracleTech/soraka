pub mod format;
use format::BYTE;
use std::fs::File;
use std::io::Write;
use std::ptr;
use std::time::{Duration, Instant};
use winapi::um::wingdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, GetDIBits, SRCCOPY};
use winapi::um::winuser::{GetDC, ReleaseDC};

const RESOLUTION_WIDTH: u32 = 2560;
const RESOLUTION_HEIGHT: u32 = 1440;
const FRAME_PER_SEC: u32 = 120;
const COLOR_CHANNEL_COUNT: u8 = 3;
const COLOR_BIT_DEPTH: u8 = 8;
const REPLAY_BUFFER_DURATION: Duration = Duration::from_secs(1);
const _MAX_RAM_USAGE: u64 = 8 * format::GIGABYTE;

const FRAME_BUFFER_BYTES_COUNT: usize = RESOLUTION_WIDTH as usize
    * RESOLUTION_HEIGHT as usize
    * COLOR_CHANNEL_COUNT as usize
    * (COLOR_BIT_DEPTH / BYTE) as usize;
const FRAME_BUFFER_BITS_COUNT: usize = FRAME_BUFFER_BYTES_COUNT * BYTE as usize;
const OMEGA_FRAME_COUNT: usize = FRAME_PER_SEC as usize * REPLAY_BUFFER_DURATION.as_secs() as usize;

pub fn record() -> Result<(), std::io::Error> {
    let delay_between_frames = Duration::from_millis(1000 / FRAME_PER_SEC as u64);

    let mut file = File::create("C:\\Users\\DREAD\\Desktop\\_\\recordings\\output.raw")
        .expect("Unable to create file");

    println!("Resolution: {}x{}", RESOLUTION_WIDTH, RESOLUTION_HEIGHT);
    println!("Channel count: {}", COLOR_CHANNEL_COUNT);
    println!("Color bit depth: {}", COLOR_BIT_DEPTH);

    println!(
        "Bits per frame: {}",
        format::bytes(FRAME_BUFFER_BITS_COUNT as u64)
    );

    println!("Frame rate: {}", FRAME_PER_SEC);
    println!(
        "Replay buffer duration: {}",
        format::duration(REPLAY_BUFFER_DURATION)
    );
    println!(
        "Frame bit size: {}",
        format::bytes(FRAME_BUFFER_BYTES_COUNT as u64)
    );
    println!(
        "Replay buffer size: {}",
        format::bytes(FRAME_BUFFER_BYTES_COUNT as u64 * OMEGA_FRAME_COUNT as u64)
    );

    let mut frame_data = vec![0; FRAME_BUFFER_BYTES_COUNT];

    let start_time = Instant::now();
    let end_time = start_time + REPLAY_BUFFER_DURATION;

    let mut next_frame = start_time;

    while Instant::now() < end_time {
        if Instant::now() < next_frame {
            continue;
        }

        if Instant::now() > next_frame + delay_between_frames {
            println!(
                "Frame was supposed to be at {}, but it's now {}",
                format::duration(next_frame - start_time),
                format::duration(Instant::now() - start_time)
            );
        }

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
        next_frame += delay_between_frames;
    }

    file.flush()?;
    file.sync_all()?;

    let file_size = file.metadata()?.len();
    let expected_file_size = FRAME_BUFFER_BYTES_COUNT as u64 * OMEGA_FRAME_COUNT as u64;
    assert_eq!(file_size, expected_file_size);

    Ok(())
}
