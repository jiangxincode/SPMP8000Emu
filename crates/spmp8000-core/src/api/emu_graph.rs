// emuIf graphics API implementation

use super::NGameApi;
use crate::memory::{Memory, VRAM_BASE};

/// emuIf graphics parameter structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct EmuGraphParams {
    pub pixels: u32,      // Source framebuffer address
    pub width: u32,       // Source width
    pub height: u32,      // Source height
    pub has_palette: u32, // Whether palette is used
    pub palette: u32,     // Palette address
    pub _unused_14: u32,
    pub src_clip_x: u32,
    pub src_clip_y: u32,
    pub src_clip_w: u32,
    pub src_clip_h: u32,
}

impl NGameApi {
    /// emuIfGraphInit - Initialize graphics subsystem
    pub fn emu_if_graph_init(&mut self, memory: &mut Memory) {
        let params_addr = memory.get_register(crate::memory::REG_R0);

        // Read parameters from memory
        let pixels = memory.read_u32(params_addr).unwrap_or(0);
        let width = memory.read_u32(params_addr + 4).unwrap_or(320);
        let height = memory.read_u32(params_addr + 8).unwrap_or(240);

        log::info!(
            "emuIfGraphInit: {}x{} framebuffer at 0x{:08X}",
            width,
            height,
            pixels
        );

        self.framebuffer_addr = Some(pixels);
        self.framebuffer_width = width;
        self.framebuffer_height = height;
        self.framebuffer_pitch = width * 2; // RGB565

        // Set return value (0 = success)
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// emuIfGraphShow - Update display (flip framebuffer)
    pub fn emu_if_graph_show(&mut self, memory: &mut Memory) {
        // The framebuffer has been updated by the game
        // We just need to signal success
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// emuIfGraphChgView - Change graphics settings
    pub fn emu_if_graph_chg_view(&mut self, memory: &mut Memory) {
        let params_addr = memory.get_register(crate::memory::REG_R0);

        if params_addr != 0 {
            let pixels = memory.read_u32(params_addr).unwrap_or(0);
            let width = memory.read_u32(params_addr + 4).unwrap_or(320);
            let height = memory.read_u32(params_addr + 8).unwrap_or(240);

            log::info!("emuIfGraphChgView: {}x{}", width, height);

            if pixels != 0 {
                self.framebuffer_addr = Some(pixels);
            }
            self.framebuffer_width = width;
            self.framebuffer_height = height;
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// emuIfGraphCleanup - Cleanup graphics subsystem
    pub fn emu_if_graph_cleanup(&mut self, memory: &mut Memory) {
        log::info!("emuIfGraphCleanup");
        self.framebuffer_addr = None;
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchInitGraph - Initialize MCatch graphics
    pub fn mcatch_init_graph(&mut self, memory: &mut Memory) {
        log::info!("MCatchInitGraph");
        self.framebuffer_addr = Some(VRAM_BASE);
        self.framebuffer_width = 320;
        self.framebuffer_height = 240;
        self.framebuffer_pitch = self.framebuffer_width * 2;
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchSetFGColor - Set foreground drawing color.
    pub fn mcatch_set_fg_color(&mut self, memory: &mut Memory) {
        let color_arg = memory.get_register(crate::memory::REG_R0);

        if let (Ok(r), Ok(g), Ok(b)) = (
            memory.read_u8(color_arg),
            memory.read_u8(color_arg + 1),
            memory.read_u8(color_arg + 2),
        ) {
            self.fg_color = [r, g, b];
        } else {
            self.fg_color = [
                ((color_arg >> 16) & 0xFF) as u8,
                ((color_arg >> 8) & 0xFF) as u8,
                (color_arg & 0xFF) as u8,
            ];
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchGetFrameBuffer - Return the active framebuffer address.
    pub fn mcatch_get_framebuffer(&mut self, memory: &mut Memory) {
        let fb_addr = self.framebuffer_addr.unwrap_or(VRAM_BASE);
        self.framebuffer_addr = Some(fb_addr);
        memory.set_register(crate::memory::REG_R0, fb_addr);
    }
    /// MCatchSetFrameBuffer - Set framebuffer dimensions
    pub fn mcatch_set_framebuffer(&mut self, memory: &mut Memory) {
        let width = memory.get_register(crate::memory::REG_R0);
        let height = memory.get_register(crate::memory::REG_R1);

        log::info!("MCatchSetFrameBuffer: {}x{}", width, height);

        if (1..=640).contains(&width) && (1..=480).contains(&height) {
            self.framebuffer_width = width;
            self.framebuffer_height = height;
            self.framebuffer_pitch = width * 2;
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }
    /// MCatchFillRect - Fill a rectangle with foreground color
    pub fn mcatch_fill_rect(&mut self, memory: &mut Memory) {
        let rect_addr = memory.get_register(crate::memory::REG_R0);

        // Read rectangle coordinates
        let x = memory.read_u16(rect_addr).unwrap_or(0) as u32;
        let y = memory.read_u16(rect_addr + 2).unwrap_or(0) as u32;
        let w = memory.read_u16(rect_addr + 4).unwrap_or(0) as u32;
        let h = memory.read_u16(rect_addr + 6).unwrap_or(0) as u32;

        log::debug!("MCatchFillRect: ({},{}) {}x{}", x, y, w, h);

        // Fill the rectangle in the framebuffer
        if let Some(fb_addr) = self.framebuffer_addr {
            let r = self.fg_color[0];
            let g = self.fg_color[1];
            let b = self.fg_color[2];

            // Convert to RGB565
            let color565 = ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | ((b as u16) >> 3);

            for dy in 0..h {
                for dx in 0..w {
                    let px = x + dx;
                    let py = y + dy;
                    if px < self.framebuffer_width && py < self.framebuffer_height {
                        let offset = py * self.framebuffer_pitch + px * 2;
                        let _ = memory.write_u16(fb_addr + offset, color565);
                    }
                }
            }
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }
}
