# SPMP8000 Emulator

A SPMP8000 game emulator written in Rust, supporting both standalone mode and RetroArch integration.

## Overview

SPMP8000 is a Sunplus multimedia SoC commonly found in portable gaming devices. This emulator implements the NGame1.0 binary format used by SPMP8000 games, with HLE (High Level Emulation) of the system API.

## Features

- **NGame1.0 format support** - File loading, header parsing, decompression
- **ARM CPU emulation** - Using Unicorn Engine
- **HLE system API** - emuIf, NativeGE, and eCos interfaces
- **Graphics rendering** - RGB565 to XRGB8888 conversion
- **Audio emulation** - PCM audio output
- **Input handling** - Keyboard input with configurable mappings
- **RetroArch integration** - libretro core for RetroArch frontend
- **Standalone mode** - minifb window with CLI

## Building

Requires [Rust](https://www.rust-lang.org/tools/install) (stable).

### Standalone Mode

```bash
cargo build -p spmp8000-emu --release
```

### RetroArch Core

```bash
cargo build -p spmp8000-libretro --release
```

## Usage

### Standalone Mode

```bash
cargo run -p spmp8000-emu --release -- path/to/game.bin
cargo run -p spmp8000-emu --release -- --scale 3 path/to/game.bin
```

### RetroArch Mode

1. Build the libretro core
2. Copy `target/release/spmp8000.dll` (or `.so`/`.dylib`) to RetroArch's `cores/` directory
3. Rename to `spmp8000_libretro.dll`
4. Load a game through RetroArch's "Load Content" menu

## Architecture

```
spmp8000-emu/
├── crates/
│   ├── spmp8000-core/        # Platform-independent emulator engine
│   ├── spmp8000-emu/         # Standalone frontend (minifb)
│   └── spmp8000-libretro/    # RetroArch core
```

## Key Mappings (Standalone)

| Key | Button |
|-----|--------|
| Arrow Up/Down/Left/Right | D-pad |
| Z | O (A/Cross) |
| X | X (B/Circle) |
| Enter | START |
| Backspace | SELECT |
| Escape | Exit |

## Supported Games

The emulator supports games in NGame1.0 format (.bin files) for SPMP8000 and SPCA556 chips, including:

- BattleGround
- BumperCars
- BurningTetris
- DeepKiller
- EggSwallower
- ElementalSpirit
- FruitParty
- GhostWorm
- GoBang
- Incoming
- JetGirl
- Lucky21
- MahJong
- MoleHunting
- Paradise777
- Racer
- ShowHand
- SmartBlocks
- SpaceBattleBall
- And many more...

## Technical Details

### BIN File Format

- Magic: "NGame1.0"
- Chip ID: "SPCA556" or "SPMP8000"
- Compressed ARM code and resources

### Memory Map

- 0x00000000 - 0x00FFFFFF: 16MB RAM
- 0x00A00000: Code load address
- 0x01000000 - 0x010FFFFF: 1MB Video RAM
- 0x00100000: Function table

## License

BSD-3-Clause
