# RetroArch Core

This guide covers installing and running the SPMP8000Emu libretro core for
RetroArch, loading content, supported features, and controls.

## Supported Platforms

| Platform | Architecture | Standalone | Libretro |
|----------|-------------|------------|----------|
| Windows | x86_64 | ✅ | ✅ |
| macOS | x86_64, aarch64 | ✅ | ✅ |
| Linux | x86_64, aarch64 | ✅ | ✅ |
| Android | arm64-v8a, armeabi-v7a, x86, x86_64 | — | ✅ |
| iOS | arm64, x86_64, arm64-simulator | — | ✅ |
| webOS | armv7 | — | ✅ |

> Android, iOS, and webOS are supported through the libretro core only (use with
> RetroArch).

## Installation

### Manual Installation

Build the libretro core:

```bash
cargo build -p spmp8000emu-libretro --release
```

Cargo names the cdylib after its lib target, so this produces
`spmp8000emu.dll` on Windows (`libspmp8000emu.so` on Linux,
`libspmp8000emu.dylib` on macOS) under `target/release/`.

RetroArch expects the core file to be named `spmp8000emu_libretro.<ext>`, so
rename it accordingly before placing it into RetroArch's `cores/` directory.
Copy `spmp8000emu_libretro.info` into RetroArch's `info/` directory so the
frontend can display the core metadata and supported features.

## Loading Games

1. Open RetroArch and select **Load Core > SPMP8000 (SPMP8000Emu)**.
2. Select **Load Content**.
3. Choose a `.bin` file in NGame1.0 format.

## Supported Features

- Video output using the XRGB8888 pixel format
- Stereo audio output with WAVE effects and synthesized MIDI music
- RetroPad input handling
- Full runtime reset from the cached game boot image
- Versioned and checksummed save states for the complete emulator runtime
- `.bin` content loading (NGame1.0 format)

Cheats and core options are not supported yet.

## Save states

The core exposes a fixed 128 MiB serialization capacity to libretro. State payloads use a
versioned binary format with LZ4 compression, content identity checking, payload length
validation, and a CRC32 checksum. A state includes the CPU, all mapped memory, HLE API,
renderer, active audio playback, input, and runtime flags.

States can only be restored while the same game content is loaded. Invalid, corrupted,
incompatible, or cross-game states are rejected before the active emulator state is changed.
RetroArch may compress the fixed-size state file according to its frontend configuration.

## RetroPad Button Mapping

| RetroPad Button | Action |
|---|---|
| D-Pad Up | Up |
| D-Pad Down | Down |
| D-Pad Left | Left |
| D-Pad Right | Right |
| A (SNES East) | O button (A) |
| B (SNES South) | X button (B) |
| Start | START |
| Select | SELECT |
