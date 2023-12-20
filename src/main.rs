#[allow(dead_code)]
mod format;

use format::BYTE;
use global_hotkey::GlobalHotKeyEvent;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use win_desktop_duplication::*;
use win_desktop_duplication::{devices::*, tex_reader::*};
use winit::event_loop::EventLoopBuilder;

const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
const CRATE_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

const SAVE_TEXT: &'static str = "Save buffer";
const TOGGLE_TEXT_PAUSE: &'static str = "Pause buffering";
const TOGGLE_TEXT_RESUME: &'static str = "Resume buffering";
const TOGGLE_TEXT_EXIT: &'static str = "Exitting...";
const QUIT_TEXT: &'static str = "Quit";

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
        let data = vec![0; *RECORDING_FRAME_BIT_COUNT];
        Frame { data }
    }
}

enum VideoBufferState {
    Paused,
    Recording,
    Exiting,
}

struct VideoBuffer {
    frames: Vec<Frame>,
    current_index: usize,
    state: VideoBufferState,

    dupl_api: DesktopDuplicationApi,
    texture_reader: TextureReader,

    icon_off: Icon,
    icon_saving: Icon,
    icon_on: Icon,

    tray_icon: TrayIcon,
    save_item: MenuItem,
    toggle_item: MenuItem,
    quit_item: MenuItem,
}

impl VideoBuffer {
    fn new() -> Self {
        let state: VideoBufferState = VideoBufferState::Recording;

        let icon_off_path = concat!(env!("CARGO_MANIFEST_DIR"), "./assets/off.png");
        let icon_off = load_icon(Path::new(icon_off_path));

        let icon_saving_path = concat!(env!("CARGO_MANIFEST_DIR"), "./assets/saving.png");
        let icon_saving = load_icon(Path::new(icon_saving_path));

        let icon_on_path = concat!(env!("CARGO_MANIFEST_DIR"), "./assets/on.png");
        let icon_on = load_icon(Path::new(icon_on_path));

        let save_item: MenuItem = MenuItem::new(SAVE_TEXT, true, None); // TODO
        let text = match VideoBufferState::Recording {
            VideoBufferState::Recording => TOGGLE_TEXT_PAUSE,
            VideoBufferState::Paused => TOGGLE_TEXT_RESUME,
            VideoBufferState::Exiting => TOGGLE_TEXT_EXIT,
        };
        let toggle_item = MenuItem::new(text, true, None);
        let quit_item = MenuItem::new(QUIT_TEXT, true, None);

        let tray_menu = Menu::new();

        tray_menu
            .append_items(&[
                &save_item,
                &PredefinedMenuItem::separator(),
                &toggle_item,
                &PredefinedMenuItem::separator(),
                &quit_item,
            ])
            .expect("Failed to add menu items");

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip(format!("{} - {}", CRATE_NAME, CRATE_DESCRIPTION))
            .with_icon(icon_on.clone())
            .build()
            .expect("Failed to create tray icon");

        // SECTION DISPLAY CAPTURE

        set_process_dpi_awareness();
        co_init();

        let adapter = AdapterFactory::new()
            .get_adapter_by_idx(0)
            .expect("Adapter not found");
        let display = adapter.get_display_by_idx(0).expect("Display not found");
        let dupl_api = DesktopDuplicationApi::new(adapter, display).expect("Duplication API error");
        let (device, ctx) = dupl_api.get_device_and_ctx();

        Self {
            frames: (0..*BUFFER_FRAME_COUNT).map(|_| Frame::new()).collect(),
            current_index: 0,
            state,

            dupl_api,
            texture_reader: TextureReader::new(device, ctx),

            icon_off,
            icon_saving,
            icon_on,

            tray_icon,
            save_item,
            toggle_item,
            quit_item,
        }
    }

    fn pause(&mut self) {
        self.state = VideoBufferState::Paused;
        self.tray_icon
            .set_icon(Some(self.icon_off.clone()))
            .expect("Failed to set icon");
        self.toggle_item.set_text(TOGGLE_TEXT_RESUME);
    }

    fn resume(&mut self) {
        self.state = VideoBufferState::Recording;
        self.tray_icon
            .set_icon(Some(self.icon_on.clone()))
            .expect("Failed to set icon");
        self.toggle_item.set_text(TOGGLE_TEXT_PAUSE);
    }

    fn exit(&mut self) {
        self.state = VideoBufferState::Exiting;
        self.tray_icon.set_show_menu_on_left_click(false);
        self.tray_icon
            .set_visible(false)
            .expect("Failed to hide tray icon");
        std::process::exit(0);
    }

    fn tray_event(&mut self, event: MenuEvent) {
        match event.id {
            id if id == self.toggle_item.id().0 => match self.state {
                VideoBufferState::Paused => {
                    self.resume();
                }
                VideoBufferState::Recording => {
                    self.pause();
                }
                _ => {}
            },
            id if id == self.quit_item.id().0 => {
                self.exit();
            }
            _ => (),
        }
    }

    fn tick(&mut self) {
        match self.state {
            VideoBufferState::Recording => {
                let start_time = Instant::now();

                let result = self.dupl_api.acquire_next_frame_now();
                if let Ok(tex) = result {
                    let buffer = &mut self.frames[self.current_index].data;

                    self.texture_reader
                        .get_data(buffer, &tex)
                        .expect("Error getting data");

                    let buffer_bit_size = buffer.len() * BYTE as usize;

                    if buffer_bit_size < *RECORDING_FRAME_BIT_COUNT {
                        buffer.resize(*RECORDING_FRAME_BIT_COUNT, 0);
                    } else if buffer_bit_size > *RECORDING_FRAME_BIT_COUNT {
                        buffer.truncate(*RECORDING_FRAME_BIT_COUNT);
                    }

                    // TODO : double buffer matrix for async / threadding ? maybe
                    self.current_index = (self.current_index + 1) % *BUFFER_FRAME_COUNT;

                    println!(
                        "Buffered frame {}/{} in {}",
                        self.current_index,
                        *BUFFER_FRAME_COUNT,
                        format::duration(start_time.elapsed()),
                    );
                }
            }
            _ => {}
        }
    }

    fn save_buffer(&self) {
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
        {
            Ok(file) => file,
            Err(e) => panic!("Error creating file: {}", e),
        };
        println!("Raw file opened");

        // FIX loop from current_index to end then from 0 to current_index - 1
        for frame in &self.frames {
            raw.write_all(&frame.data).expect("Unable to write to file");
        }
        println!("Raw file written");

        raw.flush().expect("Unable to flush file");
        raw.sync_all().expect("Unable to sync file");
        println!("Raw file flushed");

        let file_bit_size =
            raw.metadata().expect("Unable to get file metadata").len() * BYTE as u64;
        println!("Raw file size: {}", file_bit_size);
        let expected = *BUFFER_FRAME_COUNT as u64 * *BUFFER_FRAME_BIT_COUNT as u64; // TODO : replace RECORDING_FRAME_BIT_COUNT by RECORDING_FRAME_BYTE_COUNT
        println!("Expected file size: {}", expected);
        assert_eq!(file_bit_size, expected);
        println!("File passed size check");

        let output = std::process::Command::new("ffmpeg")
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
            .output();

        match output {
            Ok(output) => {
                println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
                println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
                println!("exit status: {}", output.status);
            }
            Err(e) => println!("error: {}", e),
        }
    }
}

const HOTKEY_MODIFIER: Option<Modifiers> = Some(Modifiers::SHIFT);
const HOTKEY_KEY: Code = Code::KeyX;
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
}

fn load_icon(path: &Path) -> tray_icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}

#[tokio::main]
pub async fn main() {
    let manager = GlobalHotKeyManager::new().expect("Failed to create manager");
    let hotkey = HotKey::new(HOTKEY_MODIFIER, HOTKEY_KEY);
    manager.register(hotkey).expect("Failed to register hotkey");

    let event_loop = EventLoopBuilder::new()
        .build()
        .expect("Failed to build event loop");

    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    let menu_channel = MenuEvent::receiver();
    let mut video_buffer = VideoBuffer::new();

    event_loop
        .run(
            move |event, elwt: &winit::event_loop::EventLoopWindowTarget<()>| {
                if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
                    println!("{:?}", event);
                    video_buffer.save_buffer();
                }

                match menu_channel.try_recv() {
                    Ok(menu_event) => video_buffer.tray_event(menu_event),
                    _ => {}
                }

                match event {
                    winit::event::Event::LoopExiting => {
                        video_buffer.exit();
                        return;
                    }

                    winit::event::Event::NewEvents(
                        winit::event::StartCause::ResumeTimeReached {
                            start: _,            // TODO
                            requested_resume: _, // TODO
                        },
                    ) => {
                        video_buffer.tick();
                    }
                    _ => {}
                }

                elwt.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                    Instant::now()
                        + Duration::from_secs_f64(
                            1.0 / RECORDING_FORMAT.framerate_per_second as f64,
                        ),
                ));
            },
        )
        .expect("Event loop failed");
}
