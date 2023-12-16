pub mod format;
use format::BYTE;

use std::time::Duration;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use win_desktop_duplication::*;
use win_desktop_duplication::{devices::*, tex_reader::*};

// Settings
const RESOLUTION_WIDTH: u32 = 2560;
const RESOLUTION_HEIGHT: u32 = 1440;
const FRAMERATE_PER_SEC: u32 = 120;
const COLOR_CHANNEL_COUNT: u8 = 3;
const _COLOR_BIT_DEPTH: u8 = 8;
const REPLAY_BUFFER_DURATION: Duration = Duration::from_secs(3);
const _MAX_RAM_USAGE: u64 = 8 * format::GIGABYTE;
const RAW_SAVE_PATH: &str = "C:\\Users\\DREAD\\Desktop\\_\\recordings\\output.raw";
const FFMPEG_SAVE_PATH: &str = "C:\\Users\\DREAD\\Desktop\\_\\recordings\\output.mp4";

const MAX_FRAME_BUFFERS: usize = FRAMERATE_PER_SEC as usize / 30;

const ON_START_DELETE_RAW_FILE: bool = true;
const ON_START_DELETE_FFMPEG_FILE: bool = true;

const DEBUG_MODE_SAVE_ONCE_OMEGA_BUFFER_FULL: bool = true;

// Precomputation
// enum ChannelKind {
//     Red,
//     Green,
//     Blue,
//     Alpha,
// }

// struct Channel {
//     kind: ChannelKind,
//     depth: u8,
// }

// struct Resolution {
//     width: u32,
//     height: u32,
// }

// struct VideoFormat {
//     resolution: Resolution,
//     framerate_per_second: u32,
//     channels: Vec<Channel>,
// }

const OMEGA_FRAME_COUNT: usize =
    FRAMERATE_PER_SEC as usize * REPLAY_BUFFER_DURATION.as_secs() as usize;

const FRAME_BUFFER_RESOLUTION_WIDTH: u32 = RESOLUTION_WIDTH;
const FRAME_BUFFER_RESOLUTION_HEIGHT: u32 = RESOLUTION_HEIGHT;
const FRAME_BUFFER_COLOR_CHANNEL_COUNT: u8 = 4;
const FRAME_BUFFER_COLOR_BIT_DEPTH: u8 = 8;
const FRAME_BUFFER_BYTES_COUNT: usize = FRAME_BUFFER_RESOLUTION_WIDTH as usize
    * FRAME_BUFFER_RESOLUTION_HEIGHT as usize
    * FRAME_BUFFER_COLOR_CHANNEL_COUNT as usize
    * (FRAME_BUFFER_COLOR_BIT_DEPTH / BYTE) as usize;
const _FRAME_BUFFER_BITS_COUNT: usize = FRAME_BUFFER_BYTES_COUNT * BYTE as usize;

const TRIMMED_ALPHA_FRAME_BITS_COUNT: usize = (FRAME_BUFFER_BYTES_COUNT / 4) * 3;

#[derive(Clone)]
struct Frame {
    data: Vec<u8>,
}

struct OmegaBuffer {
    frames: Vec<Frame>,
    current_index: usize,
    total_frames_count: usize,
}

#[tokio::main(flavor = "current_thread")]
pub async fn record() {
    set_process_dpi_awareness();
    co_init();

    let adapter = AdapterFactory::new().get_adapter_by_idx(0).unwrap();
    let display = adapter.get_display_by_idx(0).unwrap();
    println!("Display name: {}", display.name());

    let mut dupl_api = DesktopDuplicationApi::new(adapter, display).unwrap();

    let (device, ctx) = dupl_api.get_device_and_ctx();
    let mut texture_reader = TextureReader::new(device, ctx);

    let mut omega_buffer = OmegaBuffer::new();

    if ON_START_DELETE_RAW_FILE {
        let _ = std::fs::remove_file(RAW_SAVE_PATH);
        println!("Deleted raw file: {}", RAW_SAVE_PATH);
    }

    loop {
        let result = dupl_api.acquire_next_vsync_frame().await;
        if let Ok(tex) = result {
            let frame_buffer = &mut omega_buffer.frames[omega_buffer.current_index].data;

            texture_reader
                .get_data(frame_buffer, &tex)
                .expect("Error getting data");

            frame_buffer.resize(TRIMMED_ALPHA_FRAME_BITS_COUNT, 0);

            // TODO : double omega for async / threadding
            omega_buffer.current_index += 1;
            if omega_buffer.current_index >= MAX_FRAME_BUFFERS {
                omega_buffer.current_index = 0;

                // TEMP : stop recording when buffer is full

                println!("Captured {} frames", omega_buffer.total_frames_count);

                let mut file = match OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(RAW_SAVE_PATH)
                    .await
                {
                    Ok(file) => file,
                    Err(e) => panic!("Error creating file: {}", e),
                };

                for frame in &omega_buffer.frames {
                    file.write_all(&frame.data)
                        .await
                        .expect("Unable to write to file");
                }

                file.flush().await.expect("Unable to flush file");
                file.sync_all().await.expect("Unable to sync file");
                println!("File synced");

                let file_size = file
                    .metadata()
                    .await
                    .expect("Unable to get file metadata")
                    .len();
                dbg!(MAX_FRAME_BUFFERS);
                let expected_file_size = if DEBUG_MODE_SAVE_ONCE_OMEGA_BUFFER_FULL {
                    TRIMMED_ALPHA_FRAME_BITS_COUNT as u64 * MAX_FRAME_BUFFERS as u64
                } else {
                    TRIMMED_ALPHA_FRAME_BITS_COUNT as u64 * OMEGA_FRAME_COUNT as u64
                };
                assert_eq!(file_size, expected_file_size);
                println!("File passed size check");

                if ON_START_DELETE_FFMPEG_FILE {
                    let _ = std::fs::remove_file(FFMPEG_SAVE_PATH);
                    println!("Deleted ffmpeg file: {}", FFMPEG_SAVE_PATH);
                }

                let output = tokio::process::Command::new("ffmpeg")
                    .arg("-f")
                    .arg("rawvideo")
                    .arg("-pixel_format")
                    .arg("bgra")
                    .arg("-video_size")
                    .arg(RESOLUTION_WIDTH.to_string() + "x" + &RESOLUTION_HEIGHT.to_string())
                    .arg("-framerate")
                    .arg(FRAMERATE_PER_SEC.to_string())
                    .arg("-i")
                    .arg(RAW_SAVE_PATH)
                    .arg("-c:v")
                    .arg("libx264")
                    .arg("-pix_fmt")
                    .arg("yuv444p")
                    .arg(FFMPEG_SAVE_PATH)
                    .output();

                match output.await {
                    Ok(output) => {
                        println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
                        println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
                        println!("exit status: {}", output.status);
                    }
                    Err(e) => println!("error: {}", e),
                }

                break;
            }
        }
    }
}

impl Frame {
    fn new() -> Self {
        let data = Vec::new();
        Frame { data }
    }
}

impl OmegaBuffer {
    fn new() -> Self {
        OmegaBuffer {
            frames: (0..MAX_FRAME_BUFFERS).map(|_| Frame::new()).collect(),
            current_index: 0,
            total_frames_count: 0,
        }
    }
}
