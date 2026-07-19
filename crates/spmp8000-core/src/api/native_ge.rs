// NativeGE API implementation

use super::NGameApi;
use crate::memory::Memory;

/// Resource entry structure
#[repr(C)]
#[derive(Debug, Clone)]
pub struct GeResEntry {
    pub name: [u8; 16],
    pub res_data: u32,
}

/// Resource info structure
#[repr(C)]
#[derive(Debug, Clone)]
pub struct GeResInfo {
    pub data: u32,
    pub size: u32,
}

impl NGameApi {
    /// NativeGE_initRes - Initialize resource table
    pub fn native_ge_init_res(&mut self, memory: &mut Memory) {
        let _val = memory.get_register(crate::memory::REG_R0);
        let res_table_addr = memory.get_register(crate::memory::REG_R1);

        log::info!("NativeGE_initRes: table at 0x{:08X}", res_table_addr);

        // Read resource table entries
        // Each entry is 20 bytes: 16 bytes name + 4 bytes pointer
        self.resource_table.clear();

        if res_table_addr != 0 {
            let mut addr = res_table_addr;
            loop {
                let name_bytes = memory.read_block(addr, 16).unwrap_or_default();
                let data_ptr = memory.read_u32(addr + 16).unwrap_or(0);

                if data_ptr == 0 {
                    break;
                }

                let name = String::from_utf8_lossy(&name_bytes)
                    .trim_end_matches('\0')
                    .to_string();

                self.resource_table.push((name, data_ptr));
                addr += 20;

                // Safety limit
                if self.resource_table.len() > 256 {
                    break;
                }
            }
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// NativeGE_getRes - Get resource info by name
    pub fn native_ge_get_res(&mut self, memory: &mut Memory) {
        let name_addr = memory.get_register(crate::memory::REG_R0);
        let res_info_addr = memory.get_register(crate::memory::REG_R1);

        let name = memory.read_string(name_addr, 16).unwrap_or_default();

        log::debug!("NativeGE_getRes: {}", name);

        // Find resource by name
        if let Some((_, data_ptr)) = self.resource_table.iter().find(|(n, _)| n == &name) {
            // Write resource info
            let _ = memory.write_u32(res_info_addr, *data_ptr);
            // Size is typically stored before the data
            let size = memory.read_u32(*data_ptr).unwrap_or(0);
            let _ = memory.write_u32(res_info_addr + 4, size);

            // Return resource type (1 = WAV, 2 = MIDI, etc.)
            memory.set_register(crate::memory::REG_R0, 1);
        } else {
            log::warn!("Resource not found: {}", name);
            memory.set_register(crate::memory::REG_R0, 0);
        }
    }

    /// NativeGE_playRes - Play audio resource
    pub fn native_ge_play_res(&mut self, memory: &mut Memory) {
        let res_type = memory.get_register(crate::memory::REG_R0);
        let repeat = memory.get_register(crate::memory::REG_R1);
        let res_info_addr = memory.get_register(crate::memory::REG_R2);

        log::debug!("NativeGE_playRes: type={}, repeat={}", res_type, repeat);

        // Read resource info
        let data_addr = memory.read_u32(res_info_addr).unwrap_or(0);
        let size = memory.read_u32(res_info_addr + 4).unwrap_or(0);

        if data_addr != 0 && size > 0 {
            // Store audio buffer info for playback
            self.audio_buffer_addr = Some(data_addr);
            self.audio_buffer_size = size;
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// NativeGE_stopRes - Stop audio resource
    pub fn native_ge_stop_res(&mut self, memory: &mut Memory) {
        let res_type = memory.get_register(crate::memory::REG_R0);

        log::debug!("NativeGE_stopRes: type={}", res_type);

        self.audio_buffer_addr = None;
        self.audio_buffer_size = 0;

        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// NativeGE_getTime - Get time since application start
    pub fn native_ge_get_time(&mut self, memory: &mut Memory) {
        memory.set_register(crate::memory::REG_R0, self.emulated_time_ms());
    }

    /// NativeGE_showFPS - Toggle the firmware FPS overlay.
    pub fn native_ge_show_fps(&mut self, memory: &mut Memory) {
        let enabled = memory.get_register(crate::memory::REG_R0);
        log::debug!("NativeGE_showFPS: {}", enabled);
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// NativeGE_gameExit - Exit the game
    pub fn native_ge_game_exit(&mut self, memory: &mut Memory) {
        log::info!("NativeGE_gameExit called");
        // Signal that the game wants to exit
        // The emulator main loop should check for this
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// NativeGE_getTPEvent - Get touchscreen event
    pub fn native_ge_get_tp_event(&mut self, memory: &mut Memory) {
        let event_addr = memory.get_register(crate::memory::REG_R0);

        // No touchscreen support for now
        if event_addr != 0 {
            let _ = memory.write_u32(event_addr, 0); // No event
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// cyg_thread_delay - Delay execution
    pub fn cyg_thread_delay(&mut self, memory: &mut Memory) {
        let ticks = memory.get_register(crate::memory::REG_R0);
        log::debug!("cyg_thread_delay: {} ticks", ticks);

        // We don't actually delay in the emulator
        // The timing is handled by the main loop
        memory.set_register(crate::memory::REG_R0, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::REG_R0;

    #[test]
    fn native_ge_time_tracks_instruction_progress_not_poll_count() {
        let mut api = NGameApi::new();
        let mut memory = Memory::new();
        api.set_cpu_frequency(1_000);
        api.advance_instructions(10);

        api.native_ge_get_time(&mut memory);
        assert_eq!(memory.get_register(REG_R0), 10);

        api.native_ge_get_time(&mut memory);
        assert_eq!(memory.get_register(REG_R0), 10);

        api.advance_instructions(5);
        api.native_ge_get_time(&mut memory);
        assert_eq!(memory.get_register(REG_R0), 15);
    }
}
