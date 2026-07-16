// emuIf input API implementation

use super::NGameApi;
use crate::memory::Memory;

/// Key map indices (from libgame.h)
pub const EMU_KEY_UP: usize = 0;
pub const EMU_KEY_DOWN: usize = 1;
pub const EMU_KEY_LEFT: usize = 2;
pub const EMU_KEY_RIGHT: usize = 3;
pub const EMU_KEY_O: usize = 4;
pub const EMU_KEY_X: usize = 5;
pub const EMU_KEY_SQUARE: usize = 6;
pub const EMU_KEY_TRIANGLE: usize = 7;
pub const EMU_KEY_R: usize = 8;
pub const EMU_KEY_L: usize = 9;
pub const EMU_KEY_SELECT: usize = 10;
pub const EMU_KEY_START: usize = 11;
pub const EMU_KEY_ESC: usize = 12;

/// NativeGE key bit masks
pub const GE_KEY_UP: u32 = 1;
pub const GE_KEY_DOWN: u32 = 2;
pub const GE_KEY_LEFT: u32 = 4;
pub const GE_KEY_RIGHT: u32 = 8;
pub const GE_KEY_O: u32 = 1 << 16;
pub const GE_KEY_X: u32 = 2 << 16;
pub const GE_KEY_START: u32 = 1 << 13;

impl NGameApi {
    /// emuIfKeyInit - Initialize input subsystem
    pub fn emu_if_key_init(&mut self, memory: &mut Memory) {
        let map_addr = memory.get_register(crate::memory::REG_R0);

        log::info!("emuIfKeyInit: map at 0x{:08X}", map_addr);

        // Set up default key mappings
        // The game writes its key map structure here
        // We provide default scancode mappings
        if map_addr != 0 {
            let default_scancodes: [u32; 12] = [
                0x0001, // UP
                0x0002, // DOWN
                0x0004, // LEFT
                0x0008, // RIGHT
                0x0010, // O (A button)
                0x0020, // X (B button)
                0x0040, // SQUARE
                0x0080, // TRIANGLE
                0x0100, // R
                0x0200, // L
                0x0400, // SELECT
                0x0800, // START
            ];

            // Read the controller ID from the structure
            let controller = memory.read_u32(map_addr).unwrap_or(0);

            // Write scancodes to the structure (offset 4 for scancode array)
            for (i, &scancode) in default_scancodes.iter().enumerate() {
                let _ = memory.write_u32(map_addr + 4 + (i as u32 * 4), scancode);
            }

            // Store key map for this controller
            if (controller as usize) < 2 {
                self.key_map[..12].copy_from_slice(&default_scancodes);
            }
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// emuIfKeyGetInput - Get current key state
    pub fn emu_if_key_get_input(&mut self, memory: &mut Memory) {
        // Return raw key state bitmap
        // The game uses the key map to interpret this
        memory.set_register(crate::memory::REG_R0, self.raw_key_state);
    }

    /// emuIfKeyCleanup - Cleanup input subsystem
    pub fn emu_if_key_cleanup(&mut self, memory: &mut Memory) {
        log::info!("emuIfKeyCleanup");
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// NativeGE_getKeyInput4Ntv - Get translated key state
    pub fn native_ge_get_key_input(&mut self, memory: &mut Memory) {
        let keys_addr = memory.get_register(crate::memory::REG_R0);

        if keys_addr != 0 {
            // Write translated key state
            // The ge_key_data_t structure has: uint32_t unused, uint32_t keys
            let _ = memory.write_u32(keys_addr + 4, self.key_state);
        }
    }

    /// Translate button state to NativeGE key format
    pub fn translate_buttons(&mut self, buttons: u32) {
        self.raw_key_state = buttons;
        let mut key_state = 0u32;

        // Map button bits to NativeGE key bits
        if buttons & (1 << 0) != 0 {
            key_state |= GE_KEY_UP;
        }
        if buttons & (1 << 1) != 0 {
            key_state |= GE_KEY_DOWN;
        }
        if buttons & (1 << 2) != 0 {
            key_state |= GE_KEY_LEFT;
        }
        if buttons & (1 << 3) != 0 {
            key_state |= GE_KEY_RIGHT;
        }
        if buttons & (1 << 4) != 0 {
            key_state |= GE_KEY_O;
        }
        if buttons & (1 << 5) != 0 {
            key_state |= GE_KEY_X;
        }
        if buttons & (1 << 11) != 0 {
            key_state |= GE_KEY_START;
        }

        self.key_state = key_state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Permission;

    #[test]
    fn test_key_formats_are_kept_separate() {
        let mut api = NGameApi::new();
        let mut memory = Memory::new();
        memory
            .map_region(0x1000, 0x100, Permission::ALL, "test")
            .unwrap();

        let buttons = (1 << EMU_KEY_O) | (1 << EMU_KEY_START);
        api.translate_buttons(buttons);

        api.emu_if_key_get_input(&mut memory);
        assert_eq!(memory.get_register(crate::memory::REG_R0), 0x0810);

        memory.set_register(crate::memory::REG_R0, 0x1000);
        api.native_ge_get_key_input(&mut memory);
        assert_eq!(memory.read_u32(0x1004).unwrap(), GE_KEY_O | GE_KEY_START);
    }
}
