// emuIf graphics API implementation

use super::{GraphicsTransformation, NGameApi, Surface};
use crate::memory::{Memory, VRAM_BASE};

const SPRITE_TRANSPARENT_COLOR: u16 = 0xF81F;

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
        log::debug!("MCatchDisableFeature: {}", feature);
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

    /// MCatchSetTransformation - Transform the next blit around a reference point.
    pub fn mcatch_set_transformation(&mut self, memory: &mut Memory) {
        let reference_addr = memory.get_register(crate::memory::REG_R0);
        let kind = memory.get_register(crate::memory::REG_R1) as u8;

        if reference_addr == 0 || kind > 7 {
            self.pending_transformation = None;
            memory.set_register(crate::memory::REG_R0, 1);
            return;
        }

        self.pending_transformation = Some(GraphicsTransformation {
            reference_x: memory.read_u16(reference_addr).unwrap_or(0) as i16 as i32,
            reference_y: memory.read_u16(reference_addr + 2).unwrap_or(0) as i16 as i32,
            kind,
        });
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
        let transformation = self.pending_transformation.take();

        let Some(surface) = self.surfaces.get(&img_id).cloned() else {
            log::debug!("MCatch blit skipped: missing image id={}", img_id);
            memory.set_register(crate::memory::REG_R0, 1);
            return;
        };

        let src_x = memory.read_u16(rect_addr).unwrap_or(0) as u32;
        let src_y = memory.read_u16(rect_addr + 2).unwrap_or(0) as u32;
        let width = memory.read_u16(rect_addr + 4).unwrap_or(0) as u32;
        let height = memory.read_u16(rect_addr + 6).unwrap_or(0) as u32;
        let dst_x = memory.read_u16(at_addr).unwrap_or(0) as i16 as i32;
        let dst_y = memory.read_u16(at_addr + 2).unwrap_or(0) as i16 as i32;

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
        let (output_w, output_h) = transformed_dimensions(copy_w, copy_h, transformation);
        let (reference_x, reference_y) = transformed_reference(copy_w, copy_h, transformation);
        let origin_x = dst_x - reference_x;
        let origin_y = dst_y - reference_y;

        for y in 0..output_h {
            let py = origin_y + y as i32;
            if py < 0 || py >= self.framebuffer_height as i32 {
                continue;
            }
            for x in 0..output_w {
                let px = origin_x + x as i32;
                if px < 0 || px >= self.framebuffer_width as i32 {
                    continue;
                }

                let (source_x, source_y) = transformed_source(x, y, copy_w, copy_h, transformation);
                let idx =
                    self.read_surface_index(memory, &surface, src_x + source_x, src_y + source_y);
                if let Some(color) = self.read_surface_color(memory, &surface, idx) {
                    if sprite && color == SPRITE_TRANSPARENT_COLOR {
                        continue;
                    }
                    let offset = py as u32 * self.framebuffer_pitch + px as u32 * 2;
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
                    byte & 0x0F
                } else {
                    byte >> 4
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
}

fn transformed_dimensions(
    width: u32,
    height: u32,
    transformation: Option<GraphicsTransformation>,
) -> (u32, u32) {
    match transformation.map(|value| value.kind) {
        Some(4..=7) => (height, width),
        _ => (width, height),
    }
}

fn transformed_reference(
    width: u32,
    height: u32,
    transformation: Option<GraphicsTransformation>,
) -> (i32, i32) {
    let Some(transformation) = transformation else {
        return (0, 0);
    };
    let max_x = width.saturating_sub(1) as i32;
    let max_y = height.saturating_sub(1) as i32;
    let x = transformation.reference_x;
    let y = transformation.reference_y;

    match transformation.kind {
        0 => (x, y),
        1 => (x, max_y - y),
        2 => (max_x - x, y),
        3 => (max_x - x, max_y - y),
        4 => (y, x),
        5 => (max_y - y, x),
        6 => (y, max_x - x),
        7 => (max_y - y, max_x - x),
        _ => (0, 0),
    }
}

fn transformed_source(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    transformation: Option<GraphicsTransformation>,
) -> (u32, u32) {
    match transformation.map(|value| value.kind).unwrap_or(0) {
        0 => (x, y),
        1 => (x, height - 1 - y),
        2 => (width - 1 - x, y),
        3 => (width - 1 - x, height - 1 - y),
        4 => (y, x),
        5 => (y, height - 1 - x),
        6 => (width - 1 - y, x),
        7 => (width - 1 - y, height - 1 - x),
        _ => (x, y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Permission, REG_R0, REG_R1, REG_R2, VRAM_BASE};

    #[test]
    fn reads_four_bit_pixels_low_nibble_first() {
        const DATA_ADDR: u32 = 0x1000;

        let api = NGameApi::new();
        let mut memory = Memory::new();
        memory
            .map_region(DATA_ADDR, 0x1000, Permission::ALL, "RAM")
            .unwrap();
        memory.write_u8(DATA_ADDR, 0xA3).unwrap();
        let surface = Surface {
            data_addr: DATA_ADDR,
            width: 2,
            height: 1,
            img_type: 2,
            palette_addr: 0,
            palette_entries: 0,
        };

        assert_eq!(api.read_surface_index(&memory, &surface, 0, 0), 0x03);
        assert_eq!(api.read_surface_index(&memory, &surface, 1, 0), 0x0A);
    }

    #[test]
    fn display_screen_dimensions_are_not_used_as_framebuffer_address() {
        const DISPLAY_ADDR: u32 = 0x2000;

        let mut api = NGameApi::new();
        api.framebuffer_addr = Some(VRAM_BASE);
        let mut memory = Memory::new();
        memory
            .map_region(0x1000, 0x40000, Permission::ALL, "RAM")
            .unwrap();
        memory.write_u16(DISPLAY_ADDR + 4, 2).unwrap();
        memory.write_u16(DISPLAY_ADDR + 6, 1).unwrap();
        memory.set_register(REG_R0, DISPLAY_ADDR);

        api.mcatch_set_display_screen(&mut memory);

        assert_eq!(api.display_screen_addr, Some(DISPLAY_ADDR));
        assert_eq!(api.framebuffer_addr, Some(VRAM_BASE));
        assert_eq!(memory.get_register(REG_R0), 0);
    }

    #[test]
    fn disabling_features_ignores_stale_second_argument() {
        const STALE_ADDR: u32 = 0x2000;

        let mut api = NGameApi::new();
        api.framebuffer_addr = Some(VRAM_BASE);
        let mut memory = Memory::new();
        memory
            .map_region(0x1000, 0x40000, Permission::ALL, "RAM")
            .unwrap();
        memory.write_u16(STALE_ADDR + 4, 2).unwrap();
        memory.write_u16(STALE_ADDR + 6, 1).unwrap();
        memory.set_register(REG_R0, 12);
        memory.set_register(REG_R1, STALE_ADDR);

        api.mcatch_disable_feature(&mut memory);

        assert_eq!(api.framebuffer_addr, Some(VRAM_BASE));
        assert_eq!(memory.get_register(REG_R0), 0);
    }

    #[test]
    fn sprite_uses_magenta_palette_entry_as_transparent() {
        const DATA_ADDR: u32 = 0x1000;
        const PALETTE_ADDR: u32 = 0x1100;
        const RECT_ADDR: u32 = 0x1200;
        const AT_ADDR: u32 = 0x1300;

        let mut api = NGameApi::new();
        api.framebuffer_addr = Some(VRAM_BASE);
        api.framebuffer_width = 2;
        api.framebuffer_height = 1;
        api.framebuffer_pitch = 4;
        api.surfaces.insert(
            1,
            Surface {
                data_addr: DATA_ADDR,
                width: 2,
                height: 1,
                img_type: 1,
                palette_addr: PALETTE_ADDR,
                palette_entries: 3,
            },
        );

        let mut memory = Memory::new();
        memory
            .map_region(DATA_ADDR, 0x1000, Permission::ALL, "RAM")
            .unwrap();
        memory
            .map_region(VRAM_BASE, 0x1000, Permission::ALL, "VRAM")
            .unwrap();
        memory.write_u8(DATA_ADDR, 2).unwrap();
        memory.write_u8(DATA_ADDR + 1, 0).unwrap();
        memory.write_u16(PALETTE_ADDR, 0x001F).unwrap();
        memory.write_u16(PALETTE_ADDR + 4, 0xF81F).unwrap();
        memory.write_u16(RECT_ADDR + 4, 2).unwrap();
        memory.write_u16(RECT_ADDR + 6, 1).unwrap();
        memory.write_u16(VRAM_BASE, 0xFFFF).unwrap();
        memory.write_u16(VRAM_BASE + 2, 0xFFFF).unwrap();
        memory.set_register(REG_R0, 1);
        memory.set_register(REG_R1, RECT_ADDR);
        memory.set_register(REG_R2, AT_ADDR);

        api.mcatch_sprite(&mut memory);

        assert_eq!(memory.read_u16(VRAM_BASE).unwrap(), 0xFFFF);
        assert_eq!(memory.read_u16(VRAM_BASE + 2).unwrap(), 0x001F);
    }

    #[test]
    fn sprite_mirror_keeps_reference_point_at_destination() {
        const DATA_ADDR: u32 = 0x1000;
        const PALETTE_ADDR: u32 = 0x1100;
        const RECT_ADDR: u32 = 0x1200;
        const AT_ADDR: u32 = 0x1300;
        const REFERENCE_ADDR: u32 = 0x1400;

        let mut api = NGameApi::new();
        api.framebuffer_addr = Some(VRAM_BASE);
        api.framebuffer_width = 3;
        api.framebuffer_height = 2;
        api.framebuffer_pitch = 6;
        api.surfaces.insert(
            1,
            Surface {
                data_addr: DATA_ADDR,
                width: 3,
                height: 2,
                img_type: 1,
                palette_addr: PALETTE_ADDR,
                palette_entries: 7,
            },
        );

        let mut memory = Memory::new();
        memory
            .map_region(DATA_ADDR, 0x1000, Permission::ALL, "RAM")
            .unwrap();
        memory
            .map_region(VRAM_BASE, 0x1000, Permission::ALL, "VRAM")
            .unwrap();
        memory.write_block(DATA_ADDR, &[1, 2, 3, 4, 5, 6]).unwrap();
        for index in 1..=6 {
            memory
                .write_u16(PALETTE_ADDR + index * 2, index as u16)
                .unwrap();
        }
        memory.write_u16(RECT_ADDR + 4, 3).unwrap();
        memory.write_u16(RECT_ADDR + 6, 2).unwrap();
        memory.write_u16(AT_ADDR, 2).unwrap();
        memory.write_u16(AT_ADDR + 2, 1).unwrap();
        memory.write_u16(REFERENCE_ADDR, 0).unwrap();
        memory.write_u16(REFERENCE_ADDR + 2, 1).unwrap();

        memory.set_register(REG_R0, REFERENCE_ADDR);
        memory.set_register(REG_R1, 2);
        api.mcatch_set_transformation(&mut memory);
        memory.set_register(REG_R0, 1);
        memory.set_register(REG_R1, RECT_ADDR);
        memory.set_register(REG_R2, AT_ADDR);
        api.mcatch_sprite(&mut memory);

        let pixels = (0..6)
            .map(|index| memory.read_u16(VRAM_BASE + index * 2).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(pixels, vec![3, 2, 1, 6, 5, 4]);
        assert!(api.pending_transformation.is_none());
    }
}
