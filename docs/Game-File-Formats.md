# Game File Format — NGame1.0

SPMP8000 games use the **NGame1.0** binary format. This document describes the
file structure, header fields, and memory layout.

## Overview

Each game is a single `.bin` file containing:

1. A fixed-size header with metadata (magic, chip type, game name, etc.)
2. An encrypted/compressed payload containing ARM code and resources

## Header Layout

The header is 128 bytes (0x00–0x7F):

```
Offset   Size   Field
─────────────────────────────────────────────────
0x00     8      Magic "NGame1.0"
0x08     4      Flags (typically 0x80000000)
0x0C     8      Vendor "Sunplus"
0x1C     8      Chip ID ("SPCA556" or "SPMP8000")
0x2C     24     Game name (null-terminated)
0x44     16     Media type ("Sunmedia" or "Punmedia")
0x70     4      Version string
0x74     4      Code size (little-endian)
0x78     8      Alignment values
0x80+            Compressed game payload
```

### Chip Types

| Chip ID | Resolution | CPU Freq | Description |
|---------|-----------|----------|-------------|
| `SPCA556` | 320×240 | 7.37 MHz | Sunplus SPCA556 (portable gaming) |
| `SPMP8000` | 320×240 | 7.37 MHz | Sunplus SPMP8000 (multimedia SoC) |

### Payload

The payload at offset 0x80+ is DES-encrypted and then compressed (LZ77 or raw
depending on the game). After decryption and decompression, the data contains
ARM machine code that is loaded at address `0x00A00000`.

## Memory Map

```
Address Range              Size    Description
──────────────────────────────────────────────────
0x00000000 – 0x00FFFFFF   16 MB   RAM (code + data)
0x00200000                 4 KB   Key state register
0x009FF000                 4 KB   Function table (HLE trampolines)
0x00A00000                 —      Code load address
0x00EFFEB0                 —      Stack top (grows downward)
0x01000000 – 0x01FFFFFF   16 MB   Video RAM (VRAM)
0x02000000 – 0x02FFFFFF   16 MB   Peripheral registers
```

### Key Addresses

| Address | Name | Description |
|---------|------|-------------|
| `0x00A00000` | CODE_LOAD_ADDR | ARM code load address |
| `0x009FF000` | FUNC_TABLE_BASE | HLE function table base |
| `0x00200000` | KEY_STATE_ADDR | Button state register |
| `0x01000000` | VRAM_BASE | Video RAM base |
| `0x00F00000` | SP initial | Stack pointer initial value |

## Graphics Format

- **Pixel format**: RGB565 (16-bit, 5-6-5 bit layout)
- **Resolution**: 320×240 (typical)
- **Framebuffer**: Located at a game-selected RAM or VRAM address, converted to XRGB8888 for display

The renderer reads RGB565 pixels from the framebuffer address in emulated memory and
converts them to XRGB8888 (32-bit) for output:

```
RGB565 pixel → R = (pixel >> 11) & 0x1F  → scale to 8-bit
                G = (pixel >> 5)  & 0x3F  → scale to 8-bit
                B = pixel & 0x1F          → scale to 8-bit
```

## Audio Format

- **Sample rate**: 22050 Hz
- **Format**: PCM 16-bit signed
- **Channels**: Mono (duplicated to stereo for output)

## System API (HLE)

The emulator implements High Level Emulation for the SPMP8000 system API,
intercepting SVC (Supervisor Call) instructions. The API modules include:

- **emuIf** — Basic I/O, memory, timing
- **NativeGE** — Graphics engine (framebuffer, sprites, text)
- **eCos** — RTOS-like services (threads, mutexes)

The function table at `0x009FF000` contains trampolines that redirect SVC
calls to the emulator's HLE implementation.
