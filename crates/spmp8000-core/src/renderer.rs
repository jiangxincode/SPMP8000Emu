// Framebuffer renderer
//
// Converts the emulator's framebuffer (RGB565) to XRGB8888 for display

use crate::memory::Memory;

/// Framebuffer format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    RGB565,
    XRGB8888,
}

/// Renderer state
#[derive(Debug)]
pub struct Renderer {
    /// Display width
    pub width: u32,
    /// Display height
    pub height: u32,
    /// Pixel format of the source framebuffer
    pub format: PixelFormat,
    /// Output framebuffer (XRGB8888)
    framebuffer: Vec<u8>,
    /// Framebuffer address in emulated memory
    pub fb_addr: Option<u32>,
}

impl Renderer {
    /// Create a new renderer
    pub fn new(width: u32, height: u32) -> Self {
        let fb_size = (width * height * 4) as usize; // XRGB8888
        Self {
            width,
            height,
            format: PixelFormat::RGB565,
            framebuffer: vec![0; fb_size],
            fb_addr: None,
        }
    }

    /// Update the framebuffer from emulated memory
    pub fn update_from_memory(&mut self, memory: &Memory) {
        if let Some(addr) = self.fb_addr {
            match self.format {
                PixelFormat::RGB565 => self.convert_rgb565_to_xrgb8888(memory, addr),
                PixelFormat::XRGB8888 => self.copy_xrgb8888(memory, addr),
            }
        }
    }

    /// Convert RGB565 to XRGB8888
    fn convert_rgb565_to_xrgb8888(&mut self, memory: &Memory, addr: u32) {
        let pixel_count = (self.width * self.height) as usize;

        for i in 0..pixel_count {
            let offset = (i * 2) as u32;
            if let Ok(pixel) = memory.read_u16(addr + offset) {
                let r = ((pixel >> 11) & 0x1F) as u8;
                let g = ((pixel >> 5) & 0x3F) as u8;
                let b = (pixel & 0x1F) as u8;

                // Scale to 8-bit
                let r8 = (r << 3) | (r >> 2);
                let g8 = (g << 2) | (g >> 4);
                let b8 = (b << 3) | (b >> 2);

                let out_offset = i * 4;
                self.framebuffer[out_offset] = r8;
                self.framebuffer[out_offset + 1] = g8;
                self.framebuffer[out_offset + 2] = b8;
                self.framebuffer[out_offset + 3] = 0xFF; // Alpha
            }
        }
    }

    /// Copy XRGB8888 framebuffer
    fn copy_xrgb8888(&mut self, memory: &Memory, addr: u32) {
        let byte_count = (self.width * self.height * 4) as usize;

        for i in 0..byte_count {
            if let Ok(byte) = memory.read_u8(addr + i as u32) {
                self.framebuffer[i] = byte;
            }
        }
    }

    /// Get the framebuffer data
    pub fn get_framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    /// Get mutable framebuffer data
    pub fn get_framebuffer_mut(&mut self) -> &mut [u8] {
        &mut self.framebuffer
    }

    /// Clear the framebuffer
    pub fn clear(&mut self) {
        for byte in self.framebuffer.iter_mut() {
            *byte = 0;
        }
    }

    /// Set framebuffer address in emulated memory
    pub fn set_framebuffer_address(&mut self, addr: Option<u32>) {
        self.fb_addr = addr;
    }

    /// Set pixel format
    pub fn set_format(&mut self, format: PixelFormat) {
        self.format = format;
    }

    /// Set display dimensions
    pub fn set_dimensions(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let fb_size = (width * height * 4) as usize;
        self.framebuffer.resize(fb_size, 0);
    }

    /// Fill a rectangle with a color (for debugging/testing)
    #[allow(clippy::too_many_arguments)]
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, r: u8, g: u8, b: u8) {
        for dy in 0..h {
            for dx in 0..w {
                let px = x + dx;
                let py = y + dy;
                if px < self.width && py < self.height {
                    let offset = ((py * self.width + px) * 4) as usize;
                    if offset + 3 < self.framebuffer.len() {
                        self.framebuffer[offset] = r;
                        self.framebuffer[offset + 1] = g;
                        self.framebuffer[offset + 2] = b;
                        self.framebuffer[offset + 3] = 0xFF;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;

    #[test]
    fn test_renderer_creation() {
        let renderer = Renderer::new(320, 240);
        assert_eq!(renderer.width, 320);
        assert_eq!(renderer.height, 240);
        assert_eq!(renderer.framebuffer.len(), 320 * 240 * 4);
    }

    #[test]
    fn test_rgb565_conversion() {
        let mut renderer = Renderer::new(2, 1);
        let mut memory = Memory::new();
        memory
            .map_region(0x1000, 4096, crate::memory::Permission::ALL, "test")
            .unwrap();

        // White pixel in RGB565: 0xFFFF
        memory.write_u16(0x1000, 0xFFFF).unwrap();

        renderer.set_framebuffer_address(Some(0x1000));
        renderer.update_from_memory(&memory);

        let fb = renderer.get_framebuffer();
        assert_eq!(fb[0], 255); // R
        assert_eq!(fb[1], 255); // G
        assert_eq!(fb[2], 255); // B
        assert_eq!(fb[3], 255); // A
    }

    #[test]
    fn test_fill_rect() {
        let mut renderer = Renderer::new(4, 4);
        renderer.fill_rect(1, 1, 2, 2, 255, 0, 0);

        let fb = renderer.get_framebuffer();
        // Check pixel at (1,1)
        let offset = (1 * 4 + 1) * 4;
        assert_eq!(fb[offset], 255); // R
        assert_eq!(fb[offset + 1], 0); // G
        assert_eq!(fb[offset + 2], 0); // B
    }
}
