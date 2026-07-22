// SPMP8000 Emulator - standalone front-end (minifb window + CLI).
//
// This binary reuses the shared emulator core from the `spmp8000-core` library
// crate and only adds the platform layer: window management, command-line
// argument parsing, and keyboard input.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use minifb::{Key, Window, WindowOptions};
use spmp8000_core::emulator::Emulator;

mod audio_output;

use audio_output::AudioOutput;

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

    /// Audio volume (0-100)
    #[arg(short, long, default_value = "100", value_parser = clap::value_parser!(u32).range(0..=100))]
    volume: u32,

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

    /// Cheat rules to apply (repeatable), e.g. "mem:0x00A00100=0xFF" or "reg:r0=42"
    #[arg(long = "cheat")]
    cheats: Vec<String>,
}

fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let cli = Cli::parse();

    // Validate game path
    if !cli.game_path.exists() {
        eprintln!("Error: Game file not found: {}", cli.game_path.display());
        std::process::exit(1);
    }

    log::info!("Loading game: {}", cli.game_path.display());

    // Create the emulator
    let mut emu = Emulator::from_path(cli.game_path.clone(), cli.volume)
        .context("Failed to create emulator")?;

    // Apply cheats
    for cheat in &cli.cheats {
        if let Err(e) = emu.cheats.add_code(cheat) {
            log::warn!("Ignoring invalid cheat '{}': {}", cheat, e);
        }
    }

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

    // Create window
    let mut window = Window::new(
        &format!("SPMP8000 Emulator - {}", cli.game_path.display()),
        display_width as usize,
        display_height as usize,
        WindowOptions {
            resize: true,
            scale_mode: minifb::ScaleMode::AspectRatioStretch,
            ..Default::default()
        },
    )
    .context("Failed to create window")?;

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
        let framebuffer = emu.get_framebuffer();
        let buffer: Vec<u32> = framebuffer
            .chunks_exact(4)
            .map(|chunk| {
                let r = chunk[0] as u32;
                let g = chunk[1] as u32;
                let b = chunk[2] as u32;
                (r << 16) | (g << 8) | b
            })
            .collect();

        window
            .update_with_buffer(&buffer, width as usize, height as usize)
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
