// spmp8000-libretro - the libretro core front-end for SPMP8000Emu.
//
// This crate is built as a `cdylib` named `spmp8000`, producing
// `spmp8000.{dll,so,dylib}` which RetroArch can load directly.
// All emulation logic lives in the shared `spmp8000-core` crate;
// this crate only implements the libretro C API.

pub mod libretro;
