# Standalone Emulator

This guide covers installing and running the standalone `spmp8000-emu` binary,
loading games, keyboard controls, display scaling, headless mode, and all
command-line options.

## Supported Platforms

| Platform | Architecture | Status |
|----------|-------------|--------|
| Windows | x86_64 | ✅ |
| macOS | x86_64, aarch64 | ✅ |
| Linux | x86_64, aarch64 | ✅ |

## Installation

Download the latest standalone binary for your platform from the
[Releases](https://github.com/jiangxincode/SPMP8000Emu/releases) page.

You can also build it from source:

```bash
cargo build -p spmp8000-emu --release
```

The binary is produced at `target/release/spmp8000-emu` (`.exe` on Windows).

## Synopsis

```text
spmp8000-emu [OPTIONS] <GAME_PATH>
```

## Options

| Option | Value | Default | Description |
|---|---|---|---|
| `<GAME_PATH>` | path | *required* | Path to the game file (`.bin`). |
| `-s, --scale <N>` | `1`–`8` | `2` | Integer scaling factor for the window. |
| `-f, --fullscreen` | flag | off | Run in fullscreen mode. |
| `-v, --volume <N>` | `0`–`100` | `100` | Volume level (`0` = mute, `100` = original). |
| `--headless` | flag | off | Run without opening a window (for testing/batch processing). |
| `--frames <N>` | integer | `60` | Number of frames to run in headless mode. |
| `-S, --screenshot <PATH>` | path | — | Run N frames headlessly, save a PNG screenshot, then exit. |
| `--screenshot-frames <N>` | integer | `30` | Number of frames to run before the screenshot is taken. |

`--screenshot-frames` only has an effect together with `--screenshot`.

## Default Key Mappings

| Physical Key | Action |
|---|---|
| Arrow Up | D-pad Up |
| Arrow Down | D-pad Down |
| Arrow Left | D-pad Left |
| Arrow Right | D-pad Right |
| Z | O button (A / Cross) |
| X | X button (B / Circle) |
| Enter | START |
| Backspace | SELECT |
| Escape | Exit |

## Loading Games

The standalone emulator accepts `.bin` files in NGame1.0 format:

```bash
# Load a game directly
spmp8000-emu path/to/game.bin

# Load with 3x scaling and 80% volume
spmp8000-emu --scale 3 --volume 80 path/to/game.bin

# Fullscreen mode
spmp8000-emu --fullscreen path/to/game.bin
```

## Headless Mode

Run the emulator without a window — useful for automated testing and batch
processing:

```bash
# Run 120 frames silently
spmp8000-emu --headless --frames 120 path/to/game.bin
```

## Screenshot Mode

Capture a PNG screenshot after a number of frames, then exit:

```bash
# Take a screenshot after 90 frames (3 seconds at 30fps)
spmp8000-emu --screenshot screenshot.png --screenshot-frames 90 path/to/game.bin
```

This is used by the batch screenshot script (`scripts/batch-screenshots.ps1`)
to generate screenshots for all games at once.

## Examples

```bash
# Basic usage
spmp8000-emu path/to/game.bin

# 4x scaling
spmp8000-emu --scale 4 path/to/game.bin

# Fullscreen with 50% volume
spmp8000-emu --fullscreen --volume 50 path/to/game.bin

# Take a screenshot and exit
spmp8000-emu --screenshot shot.png --screenshot-frames 90 path/to/game.bin

# Batch screenshot (PowerShell)
scripts/batch-screenshots.ps1 -Frames 120
```
