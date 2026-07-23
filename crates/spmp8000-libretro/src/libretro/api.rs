// libretro API implementation.

#![allow(static_mut_refs)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use super::callbacks;
use super::constants::*;
use super::types::*;
use spmp8000_core::emulator::Emulator;
use std::ffi::{c_void, CStr};
use std::ptr;

/// Global emulator instance
static mut EMULATOR: Option<Emulator> = None;

/// Get a reference to the emulator
unsafe fn get_emulator() -> &'static Emulator {
    EMULATOR.as_ref().expect("Emulator not initialized")
}

/// Get a mutable reference to the emulator
unsafe fn get_emulator_mut() -> &'static mut Emulator {
    EMULATOR.as_mut().expect("Emulator not initialized")
}

// ============================================================
// Startup functions
// ============================================================

#[no_mangle]
pub extern "C" fn retro_set_environment(cb: retro_environment_t) {
    callbacks::set_environment(cb);
}

#[no_mangle]
pub extern "C" fn retro_set_video_refresh(cb: retro_video_refresh_t) {
    callbacks::set_video_refresh(cb);
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample(cb: retro_audio_sample_t) {
    callbacks::set_audio_sample(cb);
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample_batch(cb: retro_audio_sample_batch_t) {
    callbacks::set_audio_sample_batch(cb);
}

#[no_mangle]
pub extern "C" fn retro_set_input_poll(cb: retro_input_poll_t) {
    callbacks::set_input_poll(cb);
}

#[no_mangle]
pub extern "C" fn retro_set_input_state(cb: retro_input_state_t) {
    callbacks::set_input_state(cb);
}

#[no_mangle]
pub extern "C" fn retro_api_version() -> u32 {
    RETRO_API_VERSION
}

#[no_mangle]
pub extern "C" fn retro_init() {
    callbacks::init_log();
    log::info!("SPMP8000Emu libretro core initialized");
}

#[no_mangle]
pub extern "C" fn retro_deinit() {
    unsafe {
        EMULATOR = None;
    }
    log::info!("SPMP8000Emu libretro core deinitialized");
}

#[no_mangle]
pub extern "C" fn retro_get_system_info(info: *mut retro_system_info) {
    unsafe {
        (*info) = retro_system_info {
            library_name: c"SPMP8000Emu".as_ptr(),
            library_version: c"0.1.0".as_ptr(),
            valid_extensions: c"bin".as_ptr(),
            need_fullpath: true,
            block_extract: false,
        };
    }
}

#[no_mangle]
pub extern "C" fn retro_set_controller_port_device(_port: u32, _device: u32) {
    // SPMP8000 only supports basic joypad
}

// ============================================================
// Running functions
// ============================================================

#[no_mangle]
pub extern "C" fn retro_load_game(info: *const retro_game_info) -> bool {
    unsafe {
        let game_info = &*info;

        if game_info.path.is_null() {
            log::error!("Game path is null");
            return false;
        }

        let path = match CStr::from_ptr(game_info.path).to_str() {
            Ok(p) => p,
            Err(e) => {
                log::error!("Invalid game path: {}", e);
                return false;
            }
        };

        // Set pixel format to XRGB8888
        let pixel_format = retro_pixel_format::RETRO_PIXEL_FORMAT_XRGB8888;
        let success = callbacks::environment(
            RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
            &pixel_format as *const _ as *mut c_void,
        );
        if !success {
            log::error!("Failed to set pixel format");
            return false;
        }

        // Create emulator instance
        match Emulator::from_path(std::path::PathBuf::from(path), 100) {
            Ok(mut emu) => {
                let (width, height) = emu.get_resolution();
                log::info!("Game loaded: {} ({}x{})", path, width, height);
                emu.start();
                EMULATOR = Some(emu);
                true
            }
            Err(e) => {
                log::error!("Failed to load game: {}", e);
                false
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_unload_game() {
    unsafe {
        EMULATOR = None;
    }
    log::info!("Game unloaded");
}

#[no_mangle]
pub extern "C" fn retro_get_system_av_info(info: *mut retro_system_av_info) {
    unsafe {
        let emu = get_emulator();
        let (width, height) = emu.get_resolution();
        let sample_rate = emu.get_audio_sample_rate();

        (*info) = retro_system_av_info {
            geometry: retro_game_geometry {
                base_width: width,
                base_height: height,
                max_width: width,
                max_height: height,
                aspect_ratio: width as f32 / height as f32,
            },
            timing: retro_system_timing {
                fps: 30.0,
                sample_rate,
            },
        };
    }
}

#[no_mangle]
pub extern "C" fn retro_run() {
    unsafe {
        let emu = get_emulator_mut();

        // Poll input
        callbacks::input_poll();

        // Read button states
        let mut buttons: u32 = 0;
        if callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_UP) != 0 {
            buttons |= 1 << 0;
        }
        if callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_DOWN) != 0 {
            buttons |= 1 << 1;
        }
        if callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_LEFT) != 0 {
            buttons |= 1 << 2;
        }
        if callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_RIGHT) != 0 {
            buttons |= 1 << 3;
        }
        if callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_A) != 0 {
            buttons |= 1 << 4; // O button
        }
        if callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_B) != 0 {
            buttons |= 1 << 5; // X button
        }
        if callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_START) != 0 {
            buttons |= 1 << 11; // START
        }
        if callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_SELECT) != 0 {
            buttons |= 1 << 10; // SELECT
        }

        emu.set_buttons(buttons);

        // Execute one frame
        emu.tick();

        // Submit framebuffer
        let (width, height) = emu.get_resolution();
        let framebuffer = emu.get_framebuffer();
        callbacks::video_refresh(
            framebuffer.as_ptr() as *const c_void,
            width,
            height,
            (width * 4) as usize, // XRGB8888
        );

        // Submit audio samples
        let samples = emu.get_audio_samples();
        if !samples.is_empty() {
            callbacks::audio_sample_batch(samples.as_ptr(), samples.len() / 2);
        }
    }
}

// ============================================================
// Stub functions
// ============================================================

#[no_mangle]
pub extern "C" fn retro_load_game_special(
    _type: u32,
    _info: *const retro_game_info,
    _num: usize,
) -> bool {
    false
}

#[no_mangle]
pub extern "C" fn retro_serialize_size() -> usize {
    unsafe {
        match EMULATOR.as_ref() {
            Some(emu) => emu.serialize_size(),
            None => 0,
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_serialize(data: *mut c_void, size: usize) -> bool {
    unsafe {
        match EMULATOR.as_mut() {
            Some(emu) => {
                if size < emu.serialize_size() {
                    return false;
                }
                let buffer = std::slice::from_raw_parts_mut(data as *mut u8, size);
                emu.serialize(buffer).is_ok()
            }
            None => false,
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_unserialize(data: *const c_void, size: usize) -> bool {
    unsafe {
        match EMULATOR.as_mut() {
            Some(emu) => {
                let buffer = std::slice::from_raw_parts(data as *const u8, size);
                emu.deserialize(buffer).is_ok()
            }
            None => false,
        }
    }
}

// ============================================================
// Cheat functions
// ============================================================

#[no_mangle]
pub extern "C" fn retro_cheat_reset() {
    unsafe {
        if let Some(emu) = EMULATOR.as_mut() {
            emu.cheats.clear();
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_cheat_set(index: u32, enabled: bool, code: *const std::ffi::c_char) {
    unsafe {
        let Some(emu) = EMULATOR.as_mut() else {
            return;
        };

        let code = if code.is_null() {
            ""
        } else {
            match CStr::from_ptr(code).to_str() {
                Ok(code) => code,
                Err(e) => {
                    log::warn!("Ignoring invalid UTF-8 cheat at slot {}: {}", index, e);
                    return;
                }
            }
        };

        if let Err(e) = emu.cheats.set_slot(index, enabled, code) {
            log::warn!(
                "Ignoring invalid cheat at slot {} ('{}'): {}",
                index,
                code,
                e
            );
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_reset() {
    unsafe {
        if let Some(emu) = EMULATOR.as_mut() {
            emu.stop();
            emu.start();
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_get_memory_data(_id: u32) -> *mut c_void {
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn retro_get_memory_size(_id: u32) -> usize {
    0
}
