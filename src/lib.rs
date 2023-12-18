pub mod format;
use format::BYTE;
use std::path::Path;
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

#[derive(Clone)]
struct Frame {
    data: Vec<u8>,
}

impl Frame {
    fn new() -> Self {
        let data = Vec::new();
        Frame { data }
    }
}

struct VideoBuffer {
    frames: Vec<Frame>,
    current_index: usize,
}

impl VideoBuffer {
    fn new() -> Self {
        VideoBuffer {
            frames: (0..*BUFFER_FRAME_COUNT).map(|_| Frame::new()).collect(),
            current_index: 0,
        }
    }
}

// Settings
const PREFERRED_MAX_RAM_USAGE_BIT: usize = 2 * format::GIGABYTE as usize;
const BUFFER_AMNESIA: Duration = Duration::from_secs(3);

const PATH_STR: &str = "C:\\Users\\DREAD\\Desktop\\_\\recordings\\";

lazy_static::lazy_static! {
    static ref RECORDING_FOLDER: &'static Path = Path::new(PATH_STR);
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
    static ref BUFFER_VIDEO_TOTAL_FRAME_COUNT: usize = BUFFER_FORMAT.framerate_per_second as usize * BUFFER_AMNESIA.as_secs() as usize;
    static ref BUFFER_FRAME_COUNT: usize = if (PREFERRED_MAX_RAM_USAGE_BIT  / *BUFFER_FRAME_BIT_COUNT) >= *BUFFER_VIDEO_TOTAL_FRAME_COUNT {
        *BUFFER_VIDEO_TOTAL_FRAME_COUNT
    } else {
       PREFERRED_MAX_RAM_USAGE_BIT as usize / *BUFFER_FRAME_BIT_COUNT
    };





    // MAX_RAM_USAGE_BIT as usize / *BUFFER_FRAME_BIT_COUNT as usize;

}

#[tokio::main(flavor = "current_thread")]
pub async fn record() {
    set_process_dpi_awareness();
    co_init();

    let adapter = AdapterFactory::new().get_adapter_by_idx(0).unwrap();
    let display = adapter.get_display_by_idx(0).unwrap();
    println!("Display name in use : {}", display.name());

    let mut dupl_api = DesktopDuplicationApi::new(adapter, display).unwrap();
    let (device, ctx) = dupl_api.get_device_and_ctx();
    let mut texture_reader = TextureReader::new(device, ctx);
    let mut omega_buffer = VideoBuffer::new();

    loop {
        let result = dupl_api.acquire_next_vsync_frame().await;
        if let Ok(tex) = result {
            let buffer = &mut omega_buffer.frames[omega_buffer.current_index].data;

            texture_reader
                .get_data(buffer, &tex)
                .expect("Error getting data");

            let buffer_bit_size = buffer.len() * BYTE as usize;

            if buffer_bit_size < *RECORDING_FRAME_BIT_COUNT {
                buffer.resize(*RECORDING_FRAME_BIT_COUNT, 0);
            } else if buffer_bit_size > *RECORDING_FRAME_BIT_COUNT {
                buffer.truncate(*RECORDING_FRAME_BIT_COUNT);
            }

            println!(
                "Frame size: {} bits, {} bytes",
                buffer_bit_size,
                buffer_bit_size / BYTE as usize
            );

            // TODO : double omega for async / threadding
            // if omega_buffer.current_index >= *BUFFER_FRAME_COUNT {
            //     omega_buffer.current_index = 0;
            // }

            omega_buffer.current_index += 1;

            if omega_buffer.current_index >= *BUFFER_FRAME_COUNT {
                println!("Captured {} frames", *BUFFER_FRAME_COUNT);

                let timestamp = chrono::Utc::now().timestamp();
                let raw_file_name = format!("raw{}.raw", timestamp);
                let raw_full_path = RECORDING_FOLDER.join(raw_file_name);
                println!("Raw file path: {}", raw_full_path.display());

                let crafted_file_name = format!("crafted{}.mp4", timestamp);
                let crafted_full_path = RECORDING_FOLDER.join(crafted_file_name);
                println!("Crafted file path: {}", crafted_full_path.display());

                let mut raw = match OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&raw_full_path)
                    .await
                {
                    Ok(file) => file,
                    Err(e) => panic!("Error creating file: {}", e),
                };
                println!("Raw file opened");

                for frame in &omega_buffer.frames {
                    raw.write_all(&frame.data)
                        .await
                        .expect("Unable to write to file");
                }
                println!("Raw file written");

                raw.flush().await.expect("Unable to flush file");
                raw.sync_all().await.expect("Unable to sync file");
                println!("Raw file flushed");

                let file_bit_size = raw
                    .metadata()
                    .await
                    .expect("Unable to get file metadata")
                    .len()
                    * BYTE as u64;
                println!("Raw file size: {}", file_bit_size);
                let expected = *BUFFER_FRAME_COUNT as u64 * *BUFFER_FRAME_BIT_COUNT as u64; // TODO : replace RECORDING_FRAME_BIT_COUNT by RECORDING_FRAME_BYTE_COUNT
                println!("Expected file size: {}", expected);
                assert_eq!(file_bit_size, expected);
                println!("File passed size check");

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
                    .arg(raw_full_path)
                    .arg("-c:v")
                    .arg("libx264")
                    .arg("-pix_fmt")
                    .arg("yuv444p")
                    .arg(crafted_full_path)
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
