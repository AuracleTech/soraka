pub mod format;
use format::BYTE;

use std::time::Duration;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use win_desktop_duplication::*;
use win_desktop_duplication::{devices::*, tex_reader::*};

enum ChannelKind {
    Red,
    Green,
    Blue,
    Alpha,
}

struct Channel {
    _kind: ChannelKind,
    depth: u8,
}

struct Resolution {
    width: u32,
    height: u32,
}

struct VideoFormat {
    resolution: Resolution,
    framerate_per_second: u32,
    channels: Vec<Channel>,
}

impl Frame {
    fn new() -> Self {
        let data = Vec::new();
        Frame { data }
    }
}

impl VideoBuffer {
    fn new() -> Self {
        VideoBuffer {
            frames: (0..*BUFFER_FRAME_COUNT).map(|_| Frame::new()).collect(),
            current_index: 0,
            total_frames_count: 0,
        }
    }
}

// Settings
const RECORDING_RAW_PATH: &str = "C:\\Users\\DREAD\\Desktop\\_\\recordings\\output.raw";
const RECORDING_CRAFTED_PATH: &str = "C:\\Users\\DREAD\\Desktop\\_\\recordings\\output.mp4";

lazy_static::lazy_static! {
    static ref RECORDING_FORMAT: VideoFormat = VideoFormat {
        resolution: Resolution {
            width: 2560,
            height: 1440,
        },
        framerate_per_second: 120,
        channels: vec![
            Channel {
                _kind: ChannelKind::Red,
                depth: 8,
            },
            Channel {
                _kind: ChannelKind::Green,
                depth: 8,
            },
            Channel {
                _kind: ChannelKind::Blue,
                depth: 8,
            },
        ],
    };
    static ref RECORDING_BUFFER_COLOR_CHANNEL_COUNT: u8 = RECORDING_FORMAT.channels.len() as u8;
    static ref RECORDING_BUFFER_TOTAL_BIT_DEPTH: u8 = RECORDING_FORMAT.channels.iter().map(|c| c.depth).sum();
    static ref RECORDING_FRAME_COUNT: u32 = RECORDING_FORMAT.framerate_per_second * BUFFER_AMNESIA.as_secs() as u32;
    static ref RECORDING_FRAME_BIT_COUNT: usize = (RECORDING_FORMAT.resolution.width * RECORDING_FORMAT.resolution.height) as usize * *RECORDING_BUFFER_TOTAL_BIT_DEPTH as usize;
    static ref RECORDING_FRAME_BYTE_COUNT: usize = *RECORDING_FRAME_BIT_COUNT / BYTE as usize;

    static ref BUFFER_FORMAT: VideoFormat = VideoFormat {
        resolution: Resolution {
            width: 2560,
            height: 1440,
        },
        framerate_per_second: 120,
        channels: vec![
            Channel {
                _kind: ChannelKind::Red,
                depth: 8,
            },
            Channel {
                _kind: ChannelKind::Green,
                depth: 8,
            },
            Channel {
                _kind: ChannelKind::Blue,
                depth: 8,
            },
            Channel {
                _kind: ChannelKind::Alpha,
                depth: 8,
            },
        ],
    };
    static ref BUFFER_COLOR_CHANNEL_COUNT: u8 = BUFFER_FORMAT.channels.len() as u8;
    static ref BUFFER_TOTAL_BIT_DEPTH: u8 = BUFFER_FORMAT.channels.iter().map(|c| c.depth).sum();
    static ref BUFFER_FRAME_BIT_COUNT: usize = (BUFFER_FORMAT.resolution.width * BUFFER_FORMAT.resolution.height) as usize * *BUFFER_TOTAL_BIT_DEPTH as usize;
    static ref BUFFER_FRAME_BYTE_COUNT: usize = *BUFFER_FRAME_BIT_COUNT / BYTE as usize;
    static ref FRAME_BUFFER_OMEGA_FRAME_COUNT: usize =
        BUFFER_FORMAT.framerate_per_second as usize * BUFFER_AMNESIA.as_secs() as usize;
    static ref BUFFER_FRAME_COUNT: usize = RECORDING_FORMAT.framerate_per_second as usize / 4; // TODO based on MAX_RAM_USAGE

}

const TEMP_SAVE_WHEN_BUFFER_FULL: bool = true; // TEMP

const ALLOW_OVERRIDE_RAW_FILE: bool = true;
const ALLOW_OVERRIDE_CRAFTED_FILE: bool = true;

const BUFFER_AMNESIA: Duration = Duration::from_secs(3);
const _MAX_RAM_USAGE: u64 = 8 * format::GIGABYTE;

#[derive(Clone)]
struct Frame {
    data: Vec<u8>,
}

struct VideoBuffer {
    frames: Vec<Frame>,
    current_index: usize,
    total_frames_count: usize,
}

#[tokio::main(flavor = "current_thread")]
pub async fn record() {
    assert_eq!(RECORDING_FORMAT.resolution.width, 2560);
    assert_eq!(RECORDING_FORMAT.resolution.height, 1440);
    assert_eq!(RECORDING_FORMAT.framerate_per_second, 120);
    assert_eq!(RECORDING_FORMAT.channels.len(), 3);
    assert_eq!(RECORDING_FORMAT.channels[0].depth, 8);
    assert_eq!(RECORDING_FORMAT.channels[1].depth, 8);
    assert_eq!(RECORDING_FORMAT.channels[2].depth, 8);
    assert_eq!(*RECORDING_BUFFER_COLOR_CHANNEL_COUNT, 3);
    assert_eq!(*RECORDING_BUFFER_TOTAL_BIT_DEPTH, 24);
    assert_eq!(*RECORDING_FRAME_COUNT, 360);
    assert_eq!(*RECORDING_FRAME_BIT_COUNT, 2560 * 1440 * 3 * 8);
    assert_eq!(*RECORDING_FRAME_BYTE_COUNT, 2560 * 1440 * 3);

    assert_eq!(BUFFER_FORMAT.resolution.width, 2560);
    assert_eq!(BUFFER_FORMAT.resolution.height, 1440);
    assert_eq!(BUFFER_FORMAT.framerate_per_second, 120);
    assert_eq!(BUFFER_FORMAT.channels.len(), 4);
    assert_eq!(BUFFER_FORMAT.channels[0].depth, 8);
    assert_eq!(BUFFER_FORMAT.channels[1].depth, 8);
    assert_eq!(BUFFER_FORMAT.channels[2].depth, 8);
    assert_eq!(BUFFER_FORMAT.channels[3].depth, 8);
    assert_eq!(*BUFFER_COLOR_CHANNEL_COUNT, 4);
    assert_eq!(*BUFFER_TOTAL_BIT_DEPTH, 32);
    assert_eq!(*BUFFER_FRAME_BIT_COUNT, 2560 * 1440 * 4 * 8);
    assert_eq!(*BUFFER_FRAME_BYTE_COUNT, 2560 * 1440 * 4);

    assert_eq!(*FRAME_BUFFER_OMEGA_FRAME_COUNT, 360);
    assert_eq!(*BUFFER_FRAME_COUNT, 30);

    set_process_dpi_awareness();
    co_init();

    let adapter = AdapterFactory::new().get_adapter_by_idx(0).unwrap();
    let display = adapter.get_display_by_idx(0).unwrap();
    println!("Display name: {}", display.name());

    let mut dupl_api = DesktopDuplicationApi::new(adapter, display).unwrap();

    let (device, ctx) = dupl_api.get_device_and_ctx();
    let mut texture_reader = TextureReader::new(device, ctx);

    let mut omega_buffer = VideoBuffer::new();

    if ALLOW_OVERRIDE_RAW_FILE {
        let _ = std::fs::remove_file(RECORDING_RAW_PATH);
        println!("Deleted raw file: {}", RECORDING_RAW_PATH);
    }

    loop {
        let result = dupl_api.acquire_next_vsync_frame().await;
        if let Ok(tex) = result {
            let frame_buffer = &mut omega_buffer.frames[omega_buffer.current_index].data;

            texture_reader
                .get_data(frame_buffer, &tex)
                .expect("Error getting data");

            frame_buffer.resize(*RECORDING_FRAME_BYTE_COUNT, 0);

            // TODO : double omega for async / threadding
            omega_buffer.current_index += 1;
            omega_buffer.total_frames_count += 1;
            if omega_buffer.current_index >= *BUFFER_FRAME_COUNT {
                omega_buffer.current_index = 0;

                // TEMP : stop recording when buffer is full

                println!("Captured {} frames", omega_buffer.total_frames_count);

                let mut file = match OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(RECORDING_RAW_PATH)
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
                let expected_file_size = if TEMP_SAVE_WHEN_BUFFER_FULL {
                    *RECORDING_FRAME_BIT_COUNT as u64 * *BUFFER_FRAME_COUNT as u64
                } else {
                    *RECORDING_FRAME_BIT_COUNT as u64 * *RECORDING_FRAME_COUNT as u64
                };
                assert_eq!(file_size, expected_file_size);
                println!("File passed size check");

                if ALLOW_OVERRIDE_CRAFTED_FILE {
                    let _ = std::fs::remove_file(RECORDING_CRAFTED_PATH);
                    println!("Deleted ffmpeg file: {}", RECORDING_CRAFTED_PATH);
                }

                let output = tokio::process::Command::new("ffmpeg")
                    .arg("-f")
                    .arg("rawvideo")
                    .arg("-pixel_format")
                    .arg("bgra")
                    .arg("-video_size")
                    .arg(format!(
                        "{}x{}",
                        BUFFER_FORMAT.resolution.width, BUFFER_FORMAT.resolution.height
                    ))
                    .arg("-framerate")
                    .arg(format!("{}", BUFFER_FORMAT.framerate_per_second))
                    .arg("-i")
                    .arg(RECORDING_RAW_PATH)
                    .arg("-c:v")
                    .arg("libx264")
                    .arg("-pix_fmt")
                    .arg("yuv444p")
                    .arg(RECORDING_CRAFTED_PATH)
                    .output()
                    .await;

                match output {
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
