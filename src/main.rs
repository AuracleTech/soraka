#[allow(dead_code)]
mod format;

use format::BYTE;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use std::{
    fs::OpenOptions,
    io::Write,
    path::Path,
    time::{Duration, Instant},
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};
use win_desktop_duplication::{devices::*, tex_reader::*, *};
use winit::event_loop::EventLoopBuilder;

const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

const MENU_ITEM_SAVE_TEXT: &'static str = "Save buffer";
const MENU_ITEM_TOGGLE_TEXT_PAUSE: &'static str = "Pause buffering";
const MENU_ITEM_TOGGLE_TEXT_RESUME: &'static str = "Resume buffering";
const MENU_ITEM_QUIT_TEXT: &'static str = "Quit";

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
        Frame {
            data: vec![0; *BUFFER_FRAME_BIT_COUNT],
        }
    }
}

#[derive(Clone, Copy)]
enum VideoBufferState {
    Paused,
    Recording,
    Exiting,
}

struct FpsCounter {
    frame_count: usize,
    start_time: Instant,
    fps: f64,
}

impl FpsCounter {
    fn new() -> Self {
        FpsCounter {
            frame_count: 0,
            start_time: Instant::now(),
            fps: 0.0,
        }
    }

    fn update(&mut self) {
        self.frame_count += 1;

        let elapsed_time = self.start_time.elapsed().as_secs_f64();
        if elapsed_time >= 1.0 {
            self.fps = self.frame_count as f64 / elapsed_time;
            self.frame_count = 0;
            self.start_time = Instant::now();
        }
    }

    fn get_fps(&self) -> f64 {
        self.fps
    }
}

struct VideoBuffer {
    frames: Vec<Frame>,
    current_frame_index: usize,
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

    fps_counter: FpsCounter,
}

impl VideoBuffer {
    fn new() -> Self {
        let icon_off_path = concat!(env!("CARGO_MANIFEST_DIR"), "./assets/off.png");
        let icon_off = load_icon(Path::new(icon_off_path));

        let icon_saving_path = concat!(env!("CARGO_MANIFEST_DIR"), "./assets/saving.png");
        let icon_saving = load_icon(Path::new(icon_saving_path));

        let icon_on_path = concat!(env!("CARGO_MANIFEST_DIR"), "./assets/on.png");
        let icon_on = load_icon(Path::new(icon_on_path));

        let save_item: MenuItem = MenuItem::new(MENU_ITEM_SAVE_TEXT, true, None); // TODO
        let toggle_item = MenuItem::new(MENU_ITEM_TOGGLE_TEXT_PAUSE, true, None);
        let quit_item = MenuItem::new(MENU_ITEM_QUIT_TEXT, true, None);

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
            .with_tooltip(CRATE_NAME)
            .with_icon(icon_on.clone())
            .build()
            .expect("Failed to create tray icon");

        set_process_dpi_awareness();
        co_init();

        let adapter = AdapterFactory::new()
            .get_adapter_by_idx(0)
            .expect("Adapter not found");
        let display = adapter.get_display_by_idx(0).expect("Display not found");
        let dupl_api = DesktopDuplicationApi::new(adapter, display).expect("Duplication API error");
        // TODO : check options for duplication api
        let (device, ctx) = dupl_api.get_device_and_ctx();

        Self {
            frames: (0..*BUFFER_FRAME_COUNT).map(|_| Frame::new()).collect(),
            current_frame_index: 0,
            state: VideoBufferState::Recording,

            dupl_api,
            texture_reader: TextureReader::new(device, ctx),

            icon_off,
            icon_saving,
            icon_on,

            tray_icon,
            save_item,
            toggle_item,
            quit_item,

            fps_counter: FpsCounter::new(),
        }
    }

    fn set_state(&mut self, state: VideoBufferState) {
        self.state = state;
        match self.state {
            VideoBufferState::Recording => {
                self.tray_icon
                    .set_icon(Some(self.icon_on.clone()))
                    .expect("Failed to set icon");
                self.toggle_item.set_text(MENU_ITEM_TOGGLE_TEXT_PAUSE);
            }
            VideoBufferState::Paused => {
                self.tray_icon
                    .set_icon(Some(self.icon_off.clone()))
                    .expect("Failed to set icon");
                self.toggle_item.set_text(MENU_ITEM_TOGGLE_TEXT_RESUME);
            }
            VideoBufferState::Exiting => {
                self.tray_icon.set_show_menu_on_left_click(false);
                self.tray_icon
                    .set_visible(false)
                    .expect("Failed to hide tray icon");
                std::process::exit(0);
            }
        };
    }

    fn tray_event(&mut self, menu_event: MenuEvent) {
        match menu_event.id {
            id if id == self.toggle_item.id().0 => match self.state {
                VideoBufferState::Paused => self.set_state(VideoBufferState::Recording),
                VideoBufferState::Recording => self.set_state(VideoBufferState::Paused),
                _ => {}
            },
            id if id == self.quit_item.id().0 => self.set_state(VideoBufferState::Exiting),
            id if id == self.save_item.id().0 => self.save(),
            _ => (),
        }
    }

    fn tick(&mut self) {
        self.fps_counter.update();

        match self.state {
            VideoBufferState::Recording => {
                let start_time = Instant::now();

                let result = self.dupl_api.acquire_next_frame_now();
                if let Ok(tex) = result {
                    let buffer = &mut self.frames[self.current_frame_index].data;

                    self.texture_reader
                        .get_data(buffer, &tex)
                        .expect("Error getting data");

                    // TODO : trim if necessary
                    // let buffer_bit_size = buffer.len() * BYTE as usize;
                    // if buffer_bit_size < *RECORDING_FRAME_BIT_COUNT {
                    //     println!(
                    //         "resized buffer from {} to {}",
                    //         buffer_bit_size, *RECORDING_FRAME_BIT_COUNT
                    //     );
                    //     buffer.resize(*RECORDING_FRAME_BIT_COUNT, 0);
                    // } else if buffer_bit_size > *RECORDING_FRAME_BIT_COUNT {
                    //     println!(
                    //         "truncated buffer from {} to {}",
                    //         buffer_bit_size, *RECORDING_FRAME_BIT_COUNT
                    //     );
                    //     buffer.truncate(*RECORDING_FRAME_BIT_COUNT);
                    // }

                    println!(
                        "Buffered frame {}/{} in {:.2}ms, FPS: {:.2}",
                        self.current_frame_index,
                        *BUFFER_FRAME_COUNT,
                        start_time.elapsed().as_millis(),
                        self.fps_counter.get_fps(),
                    );

                    self.current_frame_index = (self.current_frame_index + 1) % *BUFFER_FRAME_COUNT;
                }
            }
            _ => {}
        }
    }

    fn save(&mut self) {
        let saved_state = self.state.clone();
        self.set_state(VideoBufferState::Paused);
        self.tray_icon
            .set_icon(Some(self.icon_saving.clone()))
            .expect("Failed to set icon");

        println!("Captured {} frames", *BUFFER_FRAME_COUNT);

        let timestamp = chrono::Utc::now().timestamp();
        let raw_file_name = format!("raw{}.raw", timestamp);
        let raw_full_path = RECORDING_FOLDER.join(raw_file_name);
        println!("Raw file path: {}", raw_full_path.display());

        let start_time = Instant::now();
        let mut raw_file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&raw_full_path)
        {
            Ok(file) => file,
            Err(e) => panic!("Error creating file: {}", e),
        };
        println!("Raw file opened");
        println!(
            "Raw file opened in {}",
            format::duration(start_time.elapsed())
        );

        let start_time = Instant::now();
        let starting_index = self.current_frame_index % self.frames.len();
        for i in 0..self.frames.len() {
            let index = (starting_index + i) % self.frames.len();
            let frame = &self.frames[index];
            raw_file
                .write_all(&frame.data)
                .expect("Unable to write to file");
        }
        println!("Raw file written");
        raw_file.flush().expect("Unable to flush file");
        println!("Raw file flushed");
        println!(
            "Raw file saved in {}",
            format::duration(start_time.elapsed())
        );

        let file_bit_size = raw_file
            .metadata()
            .expect("Unable to get file metadata")
            .len()
            * BYTE as u64;
        println!("Raw file size: {}", file_bit_size);
        let expected_bit_size = *BUFFER_FRAME_COUNT as u64 * *BUFFER_FRAME_BIT_COUNT as u64; // TODO : replace RECORDING_FRAME_BIT_COUNT by RECORDING_FRAME_BYTE_COUNT
        println!("Expected file size: {}", expected_bit_size);
        assert_eq!(file_bit_size, expected_bit_size);
        println!("File passed size check");

        let crafted_file_name = format!("crafted{}.mp4", timestamp);
        let crafted_full_path = RECORDING_FOLDER.join(crafted_file_name);
        println!("Crafted file path: {}", crafted_full_path.display());

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

        self.set_state(saved_state);
    }
}

const HOTKEY_MODIFIER_OPTION: Option<Modifiers> = None; // Some(Modifiers::SHIFT)
const HOTKEY_KEY: Code = Code::PageDown;

const PREFERRED_MAX_RAM_USAGE_BIT: usize = 5 * format::GIGABYTE as usize;
const BUFFER_AMNESIA: Duration = Duration::from_secs(4);
const RECORDING_FOLDER_STR: &str = "C:\\Users\\DREAD\\Desktop\\_\\recordings\\";

lazy_static::lazy_static! {
    static ref HOTKEY: HotKey = HotKey::new(HOTKEY_MODIFIER_OPTION, HOTKEY_KEY);

    static ref RECORDING_FOLDER: &'static Path = Path::new(RECORDING_FOLDER_STR);
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
    static ref BUFFER_TOTAL_RAM_USAGE: usize = *BUFFER_FRAME_COUNT * *BUFFER_FRAME_BIT_COUNT;
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
    println!(
        "Max memory usage: {}",
        format::bits(*BUFFER_TOTAL_RAM_USAGE as f64)
    );

    let manager = GlobalHotKeyManager::new().expect("Failed to create manager");
    manager
        .register(*HOTKEY)
        .expect("Failed to register hotkey");

    let event_loop = EventLoopBuilder::new()
        .build()
        .expect("Failed to build event loop");

    let hotkey_channel = GlobalHotKeyEvent::receiver();
    let menu_channel = MenuEvent::receiver();
    let mut video_buffer = VideoBuffer::new();

    event_loop
        .run(
            move |_event, elwt: &winit::event_loop::EventLoopWindowTarget<()>| {
                if let Ok(hotkey_event) = hotkey_channel.try_recv() {
                    match hotkey_event.state {
                        global_hotkey::HotKeyState::Pressed => video_buffer.save(),
                        global_hotkey::HotKeyState::Released => {}
                    }
                }

                if let Ok(menu_event) = menu_channel.try_recv() {
                    video_buffer.tray_event(menu_event);
                }

                video_buffer.tick();

                let resume_instant = Instant::now()
                    + Duration::from_millis(1000 / RECORDING_FORMAT.framerate_per_second as u64); // TODO  negate the time it took to process the frame to get a steady framerate, a better solution would be to use an async thread

                // let resume_instant = Instant::now();

                elwt.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(resume_instant));
            },
        )
        .expect("Event loop failed");
}
