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
    TrayIconBuilder,
};
use win_desktop_duplication::{devices::*, tex_reader::*, *};
use winit::event_loop::EventLoopBuilder;

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

const HOTKEY_MODIFIER_OPTION: Option<Modifiers> = None; // Some(Modifiers::SHIFT)
const HOTKEY_KEY: Code = Code::PageDown;

const PREFERRED_MAX_RAM_USAGE_BIT: usize = 5 * format::GIGABYTE as usize;
const BUFFER_AMNESIA: Duration = Duration::from_secs(4);
const RECORDING_FOLDER_STR: &str = "C:\\Users\\DREAD\\Desktop\\_\\recordings\\";

lazy_static::lazy_static! {
    static ref HOTKEY: HotKey = HotKey::new(HOTKEY_MODIFIER_OPTION, HOTKEY_KEY);

    static ref CRAFTED_FOLDER: &'static Path = Path::new(RECORDING_FOLDER_STR);
    static ref CRAFTED_FORMAT: VideoFormat = VideoFormat {
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
    static ref CRAFTED_TICK_RATE: Duration = Duration::from_secs_f64(1.0 / f64::from(CRAFTED_FORMAT.framerate_per_second));
    static ref CRAFTED_BUFFER_COLOR_CHANNEL_COUNT: u8 = CRAFTED_FORMAT.channels.len() as u8;
    static ref CRAFTED_BUFFER_TOTAL_BIT_DEPTH: u8 = CRAFTED_FORMAT.channels.iter().map(|c| c.depth).sum();
    static ref CRAFTED_FRAME_COUNT: u32 = CRAFTED_FORMAT.framerate_per_second * BUFFER_AMNESIA.as_secs() as u32;
    static ref CRAFTED_FRAME_BIT_COUNT: usize = (CRAFTED_FORMAT.resolution.width * CRAFTED_FORMAT.resolution.height) as usize * *CRAFTED_BUFFER_TOTAL_BIT_DEPTH as usize;
    static ref CRAFTED_FRAME_BYTE_COUNT: usize = *CRAFTED_FRAME_BIT_COUNT / BYTE as usize;

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
    static ref BUFFER_TICK_RATE: Duration = Duration::from_secs_f64(1.0 / f64::from(BUFFER_FORMAT.framerate_per_second));
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

    let icon_paused_path = concat!(env!("CARGO_MANIFEST_DIR"), "./assets/paused.png");
    let icon_paused = load_icon(Path::new(icon_paused_path));

    let icon_saving_path = concat!(env!("CARGO_MANIFEST_DIR"), "./assets/saving.png");
    let icon_saving = load_icon(Path::new(icon_saving_path));

    let icon_buffering_path = concat!(env!("CARGO_MANIFEST_DIR"), "./assets/buffering.png");
    let icon_buffering = load_icon(Path::new(icon_buffering_path));

    let toggle_item = MenuItem::new(MENU_ITEM_TOGGLE_TEXT_PAUSE, true, None);
    let quit_item = MenuItem::new(MENU_ITEM_QUIT_TEXT, true, None);

    let tray_menu = Menu::new();
    tray_menu
        .append_items(&[&toggle_item, &PredefinedMenuItem::separator(), &quit_item])
        .expect("Failed to add menu items");

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip(env!("CARGO_PKG_NAME"))
        .with_icon(icon_buffering.clone())
        .build()
        .expect("Failed to create tray icon");

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

    let _handle = std::thread::spawn(move || loop {
        let tick_start = Instant::now();

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

                    let buffer_bit_size = buffer.len() * BYTE as usize;
                    if buffer_bit_size < *CRAFTED_FRAME_BIT_COUNT {
                        println!(
                            "resized buffer from {} to {}",
                            buffer_bit_size, *CRAFTED_FRAME_BIT_COUNT
                        );
                        buffer.resize(*CRAFTED_FRAME_BIT_COUNT, 0);
                    } else if buffer_bit_size > *CRAFTED_FRAME_BIT_COUNT {
                        println!(
                            "truncated buffer from {} to {}",
                            buffer_bit_size, *CRAFTED_FRAME_BIT_COUNT
                        );
                        buffer.truncate(*CRAFTED_FRAME_BIT_COUNT);
                    }

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
                let raw_full_path = CRAFTED_FOLDER.join(raw_file_name);
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
                let expected_bit_size = *BUFFER_FRAME_COUNT as u64 * *BUFFER_FRAME_BIT_COUNT as u64;
                println!("Expected file size: {}", expected_bit_size);
                assert_eq!(file_bit_size, expected_bit_size);
                println!("File passed size check");

                let crafted_file_name = format!("crafted{}.mp4", timestamp);
                let crafted_full_path = CRAFTED_FOLDER.join(crafted_file_name);
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

                *state.lock().expect("Failed to lock state") = State::Buffering;
            }
            State::Paused => {}
            State::Exiting => std::process::exit(0),
        }

        let elapsed = tick_start.elapsed();
        if elapsed > *BUFFER_TICK_RATE {
            println!("We're late: {} ms", elapsed.as_millis());
            println!("SHOULD NOT HAPPEN, IT WOULD MEAN WE MISSED A FRAME");
        } else {
            let sleep_duration = *BUFFER_TICK_RATE - elapsed;
            std::thread::sleep(sleep_duration);
        }
    });

    event_loop
        .run(
            move |_event, _elwt: &winit::event_loop::EventLoopWindowTarget<()>| {
                if let Ok(hotkey_event) = hotkey_channel.try_recv() {
                    match hotkey_event.state {
                        global_hotkey::HotKeyState::Pressed => {
                            tray.set_icon(Some(icon_saving.clone()))
                                .expect("Failed to set icon");
                            let mut state_arc = state_arc.lock().expect("Failed to lock state");
                            *state_arc = State::Saving;
                        }
                        _ => {}
                    }
                }

                if let Ok(menu_event) = menu_channel.try_recv() {
                    match menu_event.id {
                        id if id == toggle_item.id().0 => {
                            let mut state_arc = state_arc.lock().expect("Failed to lock state");
                            match *state_arc {
                                State::Paused => {
                                    *state_arc = {
                                        tray.set_icon(Some(icon_buffering.clone()))
                                            .expect("Failed to set icon");
                                        toggle_item.set_text(MENU_ITEM_TOGGLE_TEXT_PAUSE);
                                        State::Buffering
                                    }
                                }
                                State::Buffering => {
                                    tray.set_icon(Some(icon_paused.clone()))
                                        .expect("Failed to set icon");
                                    toggle_item.set_text(MENU_ITEM_TOGGLE_TEXT_RESUME);
                                    *state_arc = State::Paused
                                }
                                _ => {}
                            }
                        }
                        id if id == quit_item.id().0 => {
                            tray.set_show_menu_on_left_click(false);
                            tray.set_visible(false).expect("Failed to hide tray icon");
                            let mut state_arc = state_arc.lock().expect("Failed to lock state");
                            *state_arc = State::Exiting
                        }
                        _ => (),
                    }
                }
            },
        )
        .expect("Event loop failed");
}
