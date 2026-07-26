// spmp8000emu-libretro - the libretro core front-end for SPMP8000Emu.
//
// This crate is built as a `cdylib` named `spmp8000emu`, producing
// `spmp8000emu.dll` or `libspmp8000emu.{so,dylib}`. Release packaging renames
// it to the `spmp8000emu_libretro` filename expected by RetroArch.
// All emulation logic lives in the shared `spmp8000emu-core` crate;
// this crate only implements the libretro C API.

pub mod libretro;
