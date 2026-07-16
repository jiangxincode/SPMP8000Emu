// emuIf graphics API implementation

use super::{NGameApi, Surface};
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

    /// MCatchSetColorROP - Set the color raster operation mode.
    pub fn mcatch_set_color_rop(&mut self, memory: &mut Memory) {
        self.color_rop = memory.get_register(crate::memory::REG_R0) as u8;
        log::debug!("MCatchSetColorROP: 0x{:02X}", self.color_rop);
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchLoadImage - Register an indexed image surface.
    pub fn mcatch_load_image(&mut self, memory: &mut Memory) {
        let loadimg_addr = memory.get_register(crate::memory::REG_R0);
        let imgid_addr = memory.get_register(crate::memory::REG_R1);

        let surface = Surface {
            data_addr: memory.read_u32(loadimg_addr).unwrap_or(0),
            width: memory.read_u16(loadimg_addr + 4).unwrap_or(0),
            height: memory.read_u16(loadimg_addr + 6).unwrap_or(0),
            img_type: memory.read_u32(loadimg_addr + 8).unwrap_or(0),
            palette_addr: memory.read_u32(loadimg_addr + 0x10).unwrap_or(0),
            palette_entries: memory.read_u16(loadimg_addr + 0x14).unwrap_or(0),
        };

        if surface.data_addr == 0 || surface.width == 0 || surface.height == 0 {
            log::warn!(
                "MCatchLoadImage failed: invalid surface at 0x{:08X}",
                loadimg_addr
            );
            memory.set_register(crate::memory::REG_R0, 1);
            return;
        }

        let img_id = self.allocate_surface_id();
        self.surfaces.insert(img_id, surface.clone());
        let _ = memory.write_u8(imgid_addr, img_id);

        log::debug!(
            "MCatchLoadImage: id={} {}x{} type={} data=0x{:08X} pal=0x{:08X}/{}",
            img_id,
            surface.width,
            surface.height,
            surface.img_type,
            surface.data_addr,
            surface.palette_addr,
            surface.palette_entries
        );
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchFreeImage - Destroy a surface.
    pub fn mcatch_free_image(&mut self, memory: &mut Memory) {
        let img_id = memory.get_register(crate::memory::REG_R0) as u8;
        self.surfaces.remove(&img_id);
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchGetFrameBuffer - Return the active framebuffer address.
    pub fn mcatch_get_framebuffer(&mut self, memory: &mut Memory) {
        let fb_addr = self.framebuffer_addr.unwrap_or(VRAM_BASE);
        self.framebuffer_addr = Some(fb_addr);
        memory.set_register(crate::memory::REG_R0, fb_addr);
    }

    /// MCatchSetDisplayScreen - Select the active display surface.
    pub fn mcatch_set_display_screen(&mut self, memory: &mut Memory) {
        let screen = memory.get_register(crate::memory::REG_R0);
        log::debug!("MCatchSetDisplayScreen: 0x{:08X}", screen);
        self.display_screen_addr = (screen != 0).then_some(screen);

        if let Some(pixel_addr) = self.resolve_display_pixels(memory, screen) {
            self.framebuffer_addr = Some(pixel_addr);
        }
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchGetDisplayScreen - Return the current display surface.
    pub fn mcatch_get_display_screen(&mut self, memory: &mut Memory) {
        let fb_addr = self.framebuffer_addr.unwrap_or(VRAM_BASE);
        self.framebuffer_addr = Some(fb_addr);
        memory.set_register(crate::memory::REG_R0, fb_addr);
    }

    /// MCatchSetAlphaBld - Store alpha-blending configuration.
    pub fn mcatch_set_alpha_blend(&mut self, memory: &mut Memory) {
        let mode = memory.get_register(crate::memory::REG_R0);
        log::debug!("MCatchSetAlphaBld: {}", mode);
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchGetAlphaBld - Return alpha blending disabled.
    pub fn mcatch_get_alpha_blend(&mut self, memory: &mut Memory) {
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchEnableFeature - Acknowledge optional graphics features.
    pub fn mcatch_enable_feature(&mut self, memory: &mut Memory) {
        let feature = memory.get_register(crate::memory::REG_R0);
        log::debug!("MCatchEnableFeature: {}", feature);
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchDisableFeature - Acknowledge optional graphics features.
    pub fn mcatch_disable_feature(&mut self, memory: &mut Memory) {
        let feature = memory.get_register(crate::memory::REG_R0);
        let arg = memory.get_register(crate::memory::REG_R1);
        log::debug!("MCatchDisableFeature: {} arg=0x{:08X}", feature, arg);
        if feature == 12 {
            if let Some(pixel_addr) = self.resolve_display_pixels(memory, arg) {
                self.framebuffer_addr = Some(pixel_addr);
            }
        }
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchSetCameraMode - Record camera mode selection.
    pub fn mcatch_set_camera_mode(&mut self, memory: &mut Memory) {
        let mode = memory.get_register(crate::memory::REG_R0);
        log::debug!("MCatchSetCameraMode: {}", mode);
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchEnableDoubleBuffer - Acknowledge double-buffer setup.
    pub fn mcatch_enable_double_buffer(&mut self, memory: &mut Memory) {
        log::debug!("MCatchEnableDoubleBuffer");
        let fb_addr = self.framebuffer_addr.unwrap_or(VRAM_BASE);
        self.framebuffer_addr = Some(fb_addr);
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchUpdateScreen - Present the active framebuffer.
    pub fn mcatch_update_screen(&mut self, memory: &mut Memory) {
        let fb_addr = self.framebuffer_addr.unwrap_or(VRAM_BASE);
        self.framebuffer_addr = Some(fb_addr);
        memory.set_register(crate::memory::REG_R0, 0);
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
    /// MCatchQueryImage - Query surface state.
    pub fn mcatch_query_image(&mut self, memory: &mut Memory) {
        let img_id = memory.get_register(crate::memory::REG_R0) as u8;
        let query = memory.get_register(crate::memory::REG_R1) as u8;
        let out = memory.get_register(crate::memory::REG_R2);

        let Some(surface) = self.surfaces.get(&img_id) else {
            log::debug!("MCatchQueryImage: missing id={} query={}", img_id, query);
            memory.set_register(crate::memory::REG_R0, 1);
            return;
        };

        let value = match query {
            1 => surface.width,
            2 => surface.height,
            3 => surface.img_type as u16,
            _ => 0,
        };
        if out != 0 {
            let _ = memory.write_u16(out, value);
        }
        log::debug!(
            "MCatchQueryImage: id={} query={} -> {}",
            img_id,
            query,
            value
        );

        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// MCatchBitblt - Copy a surface rectangle to the framebuffer.
    pub fn mcatch_bitblt(&mut self, memory: &mut Memory) {
        self.blit_surface(memory, false);
    }

    /// MCatchSprite - Copy a surface rectangle with sprite semantics.
    pub fn mcatch_sprite(&mut self, memory: &mut Memory) {
        self.blit_surface(memory, true);
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

        let fb_addr = self.framebuffer_addr.unwrap_or(VRAM_BASE);
        self.framebuffer_addr = Some(fb_addr);

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

        memory.set_register(crate::memory::REG_R0, 0);
    }

    fn allocate_surface_id(&mut self) -> u8 {
        for _ in 0..=u8::MAX {
            let id = self.next_surface_id;
            self.next_surface_id = self.next_surface_id.wrapping_add(1);
            if self.next_surface_id == 0 {
                self.next_surface_id = 1;
            }
            if !self.surfaces.contains_key(&id) {
                return id;
            }
        }
        1
    }

    fn blit_surface(&mut self, memory: &mut Memory, sprite: bool) {
        let img_id = memory.get_register(crate::memory::REG_R0) as u8;
        let rect_addr = memory.get_register(crate::memory::REG_R1);
        let at_addr = memory.get_register(crate::memory::REG_R2);

        let Some(surface) = self.surfaces.get(&img_id).cloned() else {
            log::debug!("MCatch blit skipped: missing image id={}", img_id);
            memory.set_register(crate::memory::REG_R0, 1);
            return;
        };

        let src_x = memory.read_u16(rect_addr).unwrap_or(0) as u32;
        let src_y = memory.read_u16(rect_addr + 2).unwrap_or(0) as u32;
        let width = memory.read_u16(rect_addr + 4).unwrap_or(0) as u32;
        let height = memory.read_u16(rect_addr + 6).unwrap_or(0) as u32;
        let dst_x = memory.read_u16(at_addr).unwrap_or(0) as u32;
        let dst_y = memory.read_u16(at_addr + 2).unwrap_or(0) as u32;

        log::debug!(
            "MCatch{}: id={} src=({}, {}) {}x{} dst=({}, {})",
            if sprite { "Sprite" } else { "Bitblt" },
            img_id,
            src_x,
            src_y,
            width,
            height,
            dst_x,
            dst_y
        );

        let fb_addr = self.framebuffer_addr.unwrap_or(VRAM_BASE);
        self.framebuffer_addr = Some(fb_addr);

        let surface_width = surface.width as u32;
        let surface_height = surface.height as u32;
        let copy_w = width.min(surface_width.saturating_sub(src_x));
        let copy_h = height.min(surface_height.saturating_sub(src_y));
        let transparent = sprite || self.color_rop == 0xCC;

        for y in 0..copy_h {
            let py = dst_y + y;
            if py >= self.framebuffer_height {
                continue;
            }
            for x in 0..copy_w {
                let px = dst_x + x;
                if px >= self.framebuffer_width {
                    continue;
                }

                let idx = self.read_surface_index(memory, &surface, src_x + x, src_y + y);
                if transparent && idx == 0 {
                    continue;
                }
                if let Some(color) = self.read_surface_color(memory, &surface, idx) {
                    let offset = py * self.framebuffer_pitch + px * 2;
                    let _ = memory.write_u16(fb_addr + offset, color);
                }
            }
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }

    fn read_surface_index(&self, memory: &Memory, surface: &Surface, x: u32, y: u32) -> u8 {
        let pixel_index = y * surface.width as u32 + x;
        match surface.img_type {
            2 => {
                let byte = memory
                    .read_u8(surface.data_addr + pixel_index / 2)
                    .unwrap_or(0);
                if pixel_index & 1 == 0 {
                    byte >> 4
                } else {
                    byte & 0x0F
                }
            }
            _ => memory.read_u8(surface.data_addr + pixel_index).unwrap_or(0),
        }
    }

    fn read_surface_color(&self, memory: &Memory, surface: &Surface, index: u8) -> Option<u16> {
        if surface.palette_addr == 0 {
            let v = index as u16;
            return Some(((v & 0xF8) << 8) | ((v & 0xFC) << 3) | (v >> 3));
        }
        if surface.palette_entries != 0 && index as u16 >= surface.palette_entries {
            return None;
        }
        memory
            .read_u16(surface.palette_addr + index as u32 * 2)
            .ok()
    }

    fn resolve_display_pixels(&self, memory: &Memory, screen: u32) -> Option<u32> {
        if screen == 0 {
            return None;
        }

        let framebuffer_bytes = self.framebuffer_width * self.framebuffer_height * 2;
        for offset in (0..=0x30).step_by(4) {
            let Ok(candidate) = memory.read_u32(screen + offset) else {
                continue;
            };
            if candidate < 0x0001_0000 || candidate == screen || candidate & 1 != 0 {
                continue;
            }
            if memory.read_u16(candidate).is_ok()
                && memory
                    .read_u16(candidate + framebuffer_bytes.saturating_sub(2))
                    .is_ok()
            {
                log::debug!(
                    "MCatch display pixels: screen=0x{:08X} offset=0x{:02X} pixels=0x{:08X}",
                    screen,
                    offset,
                    candidate
                );
                return Some(candidate);
            }
        }

        None
    }
}
