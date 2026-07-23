// spmp8000emu-core - the platform-independent SPMP8000 emulator engine.
//
// This crate contains the shared emulator (`emulator::Emulator`) plus all the
// format parsing, rendering, audio and input logic. It has no dependency on any
// windowing or audio output device; the front-ends (the standalone binary and
// the libretro core) link against this crate and add the platform layer.

pub mod api;
pub mod arm_cpu;
pub mod audio_engine;
mod audio_resource;
pub mod bin_loader;
pub mod decompressor;
pub mod emulator;
pub mod error;
pub mod function_table;
pub mod input_handler;
pub mod memory;
pub mod renderer;
pub mod save_state;
