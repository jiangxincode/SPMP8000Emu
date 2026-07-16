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
cargo build -p spmp8000-libretro --release
```

Cargo names the cdylib after its lib target, so this produces
`spmp8000.dll` on Windows (`libspmp8000.so` on Linux, `libspmp8000.dylib` on
macOS) under `target/release/`.

RetroArch expects the core file to be named `spmp8000_libretro.<ext>`, so rename
it accordingly before placing it into RetroArch's `cores/` directory.

## Loading Games

1. Open RetroArch and select **Load Core > SPMP8000 (SPMP8000Emu)**.
2. Select **Load Content**.
3. Choose a `.bin` file in NGame1.0 format.

## Supported Features

- Video output using the XRGB8888 pixel format
- Stereo audio output (PCM)
- RetroPad input handling
- `.bin` content loading (NGame1.0 format)
- Save states

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
