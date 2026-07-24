// SPMP8000 Emulator - standalone front-end (minifb window + CLI).
//
// This binary reuses the shared emulator core from the `spmp8000emu-core` library
// crate and only adds the platform layer: window management, command-line
// argument parsing, and keyboard input.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use minifb::{Key, Window, WindowOptions};
use spmp8000emu_core::config::{CoreConfig, UnknownInstructionPolicy};
use spmp8000emu_core::emulator::Emulator;

mod audio_output;
mod standalone;

use audio_output::AudioOutput;
use standalone::scaler::{rgba_to_xrgb, DisplayScaler, ScaleFilter};

#[cfg(target_os = "windows")]
mod screen {
    extern "system" {
        fn GetSystemMetrics(index: i32) -> i32;
    }

    const SM_CXSCREEN: i32 = 0;
    const SM_CYSCREEN: i32 = 1;

    pub fn get_screen_size() -> (usize, usize) {
        unsafe {
            (
                GetSystemMetrics(SM_CXSCREEN) as usize,
                GetSystemMetrics(SM_CYSCREEN) as usize,
            )
        }
    }
}

#[cfg(target_os = "linux")]
mod screen {
    type Display = *mut core::ffi::c_void;

    #[link(name = "X11")]
    extern "system" {
        fn XOpenDisplay(display_name: *const u8) -> Display;
        fn XCloseDisplay(display: Display) -> i32;
        fn XDisplayWidth(display: Display, screen_number: i32) -> i32;
        fn XDisplayHeight(display: Display, screen_number: i32) -> i32;
    }

    pub fn get_screen_size() -> (usize, usize) {
        unsafe {
            let display = XOpenDisplay(std::ptr::null());
            if display.is_null() {
                return (800, 600);
            }
            let width = XDisplayWidth(display, 0) as usize;
            let height = XDisplayHeight(display, 0) as usize;
            let _ = XCloseDisplay(display);
            (width, height)
        }
    }
}

#[cfg(target_os = "macos")]
mod screen {
    type CGDirectDisplayId = u32;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGMainDisplayID() -> CGDirectDisplayId;
        fn CGDisplayPixelsWide(display: CGDirectDisplayId) -> usize;
        fn CGDisplayPixelsHigh(display: CGDirectDisplayId) -> usize;
    }

    pub fn get_screen_size() -> (usize, usize) {
        unsafe {
            let display = CGMainDisplayID();
            (CGDisplayPixelsWide(display), CGDisplayPixelsHigh(display))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum UnknownInstructionMode {
    Stop,
    Skip,
}

impl From<UnknownInstructionMode> for UnknownInstructionPolicy {
    fn from(value: UnknownInstructionMode) -> Self {
        match value {
            UnknownInstructionMode::Stop => Self::Stop,
            UnknownInstructionMode::Skip => Self::Skip,
        }
    }
}

/// SPMP8000 Game Emulator
#[derive(Parser)]
#[command(name = "spmp8000-emu")]
#[command(about = "A SPMP8000 game emulator written in Rust")]
#[command(version)]
struct Cli {
    /// Path to the game BIN file
    game_path: std::path::PathBuf,

    /// Window scale factor (1-8)
    #[arg(short, long, default_value = "2", value_parser = clap::value_parser!(u32).range(1..=8))]
    scale: u32,

    /// Fullscreen mode
    #[arg(short, long)]
    fullscreen: bool,

    /// Pixel scaling filter for display output
    #[arg(long, value_enum, default_value_t = ScaleFilter::Nearest)]
    filter: ScaleFilter,

    /// Audio volume (0-100)
    #[arg(short, long, default_value = "100", value_parser = clap::value_parser!(u32).range(0..=100))]
    volume: u32,

    /// Swap the emulated O and X buttons
    #[arg(long = "swap-ox")]
    swap_o_x: bool,

    /// Enable CPU and HLE debug logging
    #[arg(long)]
    debug_logging: bool,

    /// Behavior when an unknown ARM instruction is encountered
    #[arg(long, value_enum, default_value_t = UnknownInstructionMode::Stop)]
    unknown_instruction_policy: UnknownInstructionMode,

    /// Run without opening a window
    #[arg(long)]
    headless: bool,

    /// Number of frames to run in headless mode
    #[arg(long, default_value = "60")]
    frames: u32,

    /// Take a screenshot after N frames and exit (saves as PNG)
    #[arg(short = 'S', long = "screenshot", value_name = "PATH")]
    screenshot: Option<std::path::PathBuf>,

    /// Number of frames to run before taking screenshot
    #[arg(long = "screenshot-frames", default_value = "30")]
    screenshot_frames: u32,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let default_log_filter = if cli.debug_logging { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_log_filter))
        .format_timestamp_millis()
        .init();

    // Validate game path
    if !cli.game_path.exists() {
        eprintln!("Error: Game file not found: {}", cli.game_path.display());
        std::process::exit(1);
    }

    log::info!("Loading game: {}", cli.game_path.display());

    // Create the emulator
    let config = CoreConfig {
        volume: cli.volume,
        swap_o_x: cli.swap_o_x,
        debug_logging: cli.debug_logging,
        unknown_instruction_policy: cli.unknown_instruction_policy.into(),
    };
    let mut emu = Emulator::from_path_with_config(cli.game_path.clone(), config)
        .context("Failed to create emulator")?;

    let (width, height) = emu.get_resolution();
    let display_width = width * cli.scale;
    let display_height = height * cli.scale;

    log::info!(
        "Resolution: {}x{} (display: {}x{})",
        width,
        height,
        display_width,
        display_height
    );

    if cli.headless || cli.screenshot.is_some() {
        emu.start();
        let frames = cli
            .screenshot
            .as_ref()
            .map_or(cli.frames, |_| cli.screenshot_frames);
        for frame in 0..frames {
            emu.tick();
            if !emu.is_running() && !emu.should_exit() {
                anyhow::bail!("Emulation stopped before frame {}", frame + 1);
            }
        }
        if let Some(path) = &cli.screenshot {
            emu.renderer
                .save_screenshot(path)
                .context("Failed to save screenshot")?;
            log::info!("Screenshot saved to: {}", path.display());
        } else {
            log::info!("Headless run completed: {} frames", frames);
        }
        return Ok(());
    }

    let (window_width, window_height) = if cli.fullscreen {
        screen::get_screen_size()
    } else {
        (display_width as usize, display_height as usize)
    };

    // Create window
    let mut window = Window::new(
        &format!("SPMP8000 Emulator - {}", cli.game_path.display()),
        window_width,
        window_height,
        WindowOptions {
            resize: !cli.fullscreen,
            borderless: cli.fullscreen,
            scale_mode: minifb::ScaleMode::Stretch,
            ..Default::default()
        },
    )
    .context("Failed to create window")?;

    if cli.fullscreen {
        window.topmost(true);
        window.set_position(0, 0);
    }

    // Limit to ~30fps
    window.set_target_fps(30);

    // Start emulation
    emu.start();

    let audio_output = match AudioOutput::new(emu.get_audio_sample_rate() as u32) {
        Ok(output) => Some(output),
        Err(error) => {
            log::warn!("Audio output is unavailable: {}", error);
            None
        }
    };

    // Main loop
    let frame_duration = Duration::from_secs_f64(1.0 / 30.0);
    let mut frame_count = 0u32;
    let mut source_buffer = Vec::with_capacity((width * height) as usize);
    let mut display_scaler = DisplayScaler::new(cli.filter);

    while window.is_open() && !window.is_key_down(Key::Escape) && !emu.should_exit() {
        let start = Instant::now();

        // Read keyboard input
        let mut buttons: u32 = 0;
        if window.is_key_down(Key::Up) {
            buttons |= 1 << 0;
        }
        if window.is_key_down(Key::Down) {
            buttons |= 1 << 1;
        }
        if window.is_key_down(Key::Left) {
            buttons |= 1 << 2;
        }
        if window.is_key_down(Key::Right) {
            buttons |= 1 << 3;
        }
        if window.is_key_down(Key::Z) {
            buttons |= 1 << 4; // O button
        }
        if window.is_key_down(Key::X) {
            buttons |= 1 << 5; // X button
        }
        if window.is_key_down(Key::Enter) {
            buttons |= 1 << 11; // START
        }
        if window.is_key_down(Key::Backspace) {
            buttons |= 1 << 10; // SELECT
        }

        emu.set_buttons(buttons);

        // Execute one frame
        emu.tick();
        if let Some(output) = &audio_output {
            output.submit(emu.get_audio_samples());
        }

        // Update window with framebuffer
        rgba_to_xrgb(emu.get_framebuffer(), &mut source_buffer);
        let (window_width, window_height) = window.get_size();
        let buffer =
            display_scaler.render(&source_buffer, width, height, window_width, window_height);

        window
            .update_with_buffer(buffer, window_width.max(1), window_height.max(1))
            .context("Failed to update window")?;

        frame_count += 1;
        if let Some(path) = &cli.screenshot {
            if frame_count >= cli.screenshot_frames {
                emu.renderer
                    .save_screenshot(path)
                    .context("Failed to save screenshot")?;
                log::info!("Screenshot saved to: {}", path.display());
                break;
            }
        }

        // Frame rate control
        let elapsed = start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }

    log::info!("Emulator shutdown");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_core_config_defaults_match_libretro_defaults() {
        let cli = Cli::try_parse_from(["spmp8000-emu", "game.bin"]).unwrap();

        assert_eq!(cli.volume, CoreConfig::default().volume);
        assert!(!cli.swap_o_x);
        assert!(!cli.debug_logging);
        assert_eq!(cli.unknown_instruction_policy, UnknownInstructionMode::Stop);
        assert_eq!(cli.filter, ScaleFilter::Nearest);
    }

    #[test]
    fn cli_parses_all_core_config_overrides() {
        let cli = Cli::try_parse_from([
            "spmp8000-emu",
            "--volume",
            "35",
            "--swap-ox",
            "--debug-logging",
            "--unknown-instruction-policy",
            "skip",
            "game.bin",
        ])
        .unwrap();

        assert_eq!(cli.volume, 35);
        assert!(cli.swap_o_x);
        assert!(cli.debug_logging);
        assert_eq!(cli.unknown_instruction_policy, UnknownInstructionMode::Skip);
    }

    #[test]
    fn cli_parses_every_display_filter() {
        for (name, expected) in [
            ("nearest", ScaleFilter::Nearest),
            ("bilinear", ScaleFilter::Bilinear),
            ("bicubic", ScaleFilter::Bicubic),
            ("xbrz", ScaleFilter::Xbrz),
        ] {
            let cli = Cli::try_parse_from(["spmp8000-emu", "--filter", name, "game.bin"]).unwrap();
            assert_eq!(cli.filter, expected);
        }
    }
}
