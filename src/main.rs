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
    sync::{Arc, Mutex},
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
enum State {
    Paused,
    Buffering,
    Saving,
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

struct Program {
    state: Arc<Mutex<State>>,
    tray: Arc<Mutex<Tray>>,

    frames: Vec<Frame>,
    current_frame_index: usize,

    dupl_api: DesktopDuplicationApi,
    texture_reader: TextureReader,

    fps_counter: FpsCounter,
}

struct Tray {
    icon_off: Icon,
    icon_saving: Icon,
    icon_on: Icon,

    tray_icon: TrayIcon,
    save_item: MenuItem,
    toggle_item: MenuItem,
    quit_item: MenuItem,
}

impl Tray {
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

        Self {
            icon_off,
            icon_saving,
            icon_on,

            tray_icon,
            save_item,
            toggle_item,
            quit_item,
        }
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

    set_process_dpi_awareness();
    co_init();

    let adapter = AdapterFactory::new()
        .get_adapter_by_idx(0)
        .expect("Adapter not found");
    let display = adapter.get_display_by_idx(0).expect("Display not found");
    let mut dupl_api = DesktopDuplicationApi::new(adapter, display).expect("Duplication API error");
    // TODO : check options for duplication api
    let (device, ctx) = dupl_api.get_device_and_ctx();

    let state = Arc::new(Mutex::new(State::Buffering));
    let state_arc = state.clone();
    let tray = Arc::new(Mutex::new(Tray::new()));

    let mut frames: Vec<Vec<u8>> = vec![vec![0; *BUFFER_FRAME_BIT_COUNT]; *BUFFER_FRAME_COUNT];
    let mut current_frame_index = 0;
    let mut texture_reader = TextureReader::new(device, ctx);
    let mut fps_counter = FpsCounter::new();

    let manager = GlobalHotKeyManager::new().expect("Failed to create manager");
    manager
        .register(*HOTKEY)
        .expect("Failed to register hotkey");

    let event_loop = EventLoopBuilder::new()
        .build()
        .expect("Failed to build event loop");

    let hotkey_channel = GlobalHotKeyEvent::receiver();
    let menu_channel = MenuEvent::receiver();

    let _handle = std::thread::spawn(move || {
        let tick_duration =
            Duration::from_secs_f64(1.0 / f64::from(BUFFER_FORMAT.framerate_per_second));
        let mut last_tick_time = Instant::now();

        loop {
            fps_counter.update();
            let state_guard = state.lock().expect("Failed to lock state");
            let state_clone = *state_guard;
            drop(state_guard);
            match state_clone {
                State::Buffering => {
                    let start_time = Instant::now();

                    let result = dupl_api.acquire_next_frame_now();
                    if let Ok(tex) = result {
                        let buffer = &mut frames[current_frame_index];

                        texture_reader
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
                            current_frame_index,
                            *BUFFER_FRAME_COUNT,
                            start_time.elapsed().as_millis(),
                            fps_counter.get_fps(),
                        );

                        current_frame_index = (current_frame_index + 1) % *BUFFER_FRAME_COUNT;
                    }
                }
                State::Saving => {
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
                    let starting_index = current_frame_index % frames.len();
                    for i in 0..frames.len() {
                        let start_time_frame = Instant::now();
                        let index = (starting_index + i) % frames.len();
                        let frame = &frames[index];
                        raw_file.write_all(&frame).expect("Unable to write to file");
                        println!(
                            "Wrote frame {}/{} in {:.2}ms",
                            i,
                            *BUFFER_FRAME_COUNT,
                            start_time_frame.elapsed().as_millis(),
                        );
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
                    let expected_bit_size =
                        *BUFFER_FRAME_COUNT as u64 * *BUFFER_FRAME_BIT_COUNT as u64; // TODO : replace RECORDING_FRAME_BIT_COUNT by RECORDING_FRAME_BYTE_COUNT
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

                    let mut state_guard = state.lock().expect("Failed to lock state");
                    *state_guard = State::Buffering;
                }
                State::Paused => {}
                State::Exiting => std::process::exit(0),
            }

            // Check if we're late
            let elapsed = last_tick_time.elapsed();
            if elapsed > tick_duration {
                println!("We're late: {} ms", elapsed.as_millis());
            } else {
                let sleep_duration = tick_duration
                    .checked_sub(elapsed)
                    .unwrap_or_else(|| Duration::from_secs(0));
                std::thread::sleep(sleep_duration);
            }

            last_tick_time = Instant::now();
        }
    });

    event_loop
        .run(
            move |_event, _elwt: &winit::event_loop::EventLoopWindowTarget<()>| {
                if let Ok(hotkey_event) = hotkey_channel.try_recv() {
                    match hotkey_event.state {
                        global_hotkey::HotKeyState::Pressed => {
                            let tray = tray.lock().expect("Failed to lock tray");
                            tray.tray_icon
                                .set_icon(Some(tray.icon_saving.clone()))
                                .expect("Failed to set icon");
                            let mut state_arc = state_arc.lock().expect("Failed to lock state");
                            *state_arc = State::Saving;
                        }
                        _ => {}
                    }
                }

                if let Ok(menu_event) = menu_channel.try_recv() {
                    let tray = tray.lock().expect("Failed to lock tray");
                    match menu_event.id {
                        id if id == tray.toggle_item.id().0 => {
                            let mut state_arc = state_arc.lock().expect("Failed to lock state");
                            match *state_arc {
                                State::Paused => {
                                    *state_arc = {
                                        tray.tray_icon
                                            .set_icon(Some(tray.icon_on.clone()))
                                            .expect("Failed to set icon");
                                        tray.toggle_item.set_text(MENU_ITEM_TOGGLE_TEXT_PAUSE);
                                        State::Buffering
                                    }
                                }
                                State::Buffering => {
                                    tray.tray_icon
                                        .set_icon(Some(tray.icon_off.clone()))
                                        .expect("Failed to set icon");
                                    tray.toggle_item.set_text(MENU_ITEM_TOGGLE_TEXT_RESUME);
                                    *state_arc = State::Paused
                                }
                                _ => {}
                            }
                        }
                        id if id == tray.quit_item.id().0 => {
                            tray.tray_icon.set_show_menu_on_left_click(false);
                            tray.tray_icon
                                .set_visible(false)
                                .expect("Failed to hide tray icon");
                            let mut state_arc = state_arc.lock().expect("Failed to lock state");
                            *state_arc = State::Exiting
                        }
                        id if id == tray.save_item.id().0 => {
                            let mut state_arc = state_arc.lock().expect("Failed to lock state");
                            *state_arc = State::Saving
                        }
                        _ => (),
                    }
                }
            },
        )
        .expect("Event loop failed");
}
