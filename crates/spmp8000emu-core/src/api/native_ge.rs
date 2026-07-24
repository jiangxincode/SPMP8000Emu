// NativeGE API implementation

use super::NGameApi;
use crate::audio_resource::{inspect_resource, valid_resource_size, AudioCommand};
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
            if let Some((resource_type, size)) = inspect_resource(memory, *data_ptr) {
                let _ = memory.write_u32(res_info_addr, *data_ptr);
                let _ = memory.write_u32(res_info_addr + 4, size);
                memory.set_register(crate::memory::REG_R0, resource_type);
            } else {
                log::warn!("Unsupported audio resource: {}", name);
                memory.set_register(crate::memory::REG_R0, 0);
            }
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

        if data_addr != 0 && valid_resource_size(size as usize) {
            match memory.read_block(data_addr, size as usize) {
                Ok(data) => self.audio_commands.push(AudioCommand::Play {
                    resource_type: res_type,
                    repeat,
                    data,
                }),
                Err(error) => log::warn!("Failed to read audio resource: {}", error),
            }
        } else if size > 0 {
            log::warn!("Invalid audio resource size: {}", size);
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// NativeGE_stopRes - Stop audio resource
    pub fn native_ge_stop_res(&mut self, memory: &mut Memory) {
        let res_type = memory.get_register(crate::memory::REG_R0);

        log::debug!("NativeGE_stopRes: type={}", res_type);

        self.audio_commands.push(AudioCommand::Stop {
            resource_type: res_type,
        });

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

    fn wave_resource() -> Vec<u8> {
        let mut wave = Vec::new();
        wave.extend_from_slice(b"RIFF");
        wave.extend_from_slice(&38u32.to_le_bytes());
        wave.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
        wave.extend_from_slice(&8_000u32.to_le_bytes());
        wave.extend_from_slice(&8_000u32.to_le_bytes());
        wave.extend_from_slice(&1u16.to_le_bytes());
        wave.extend_from_slice(&8u16.to_le_bytes());
        wave.extend_from_slice(b"data\x02\0\0\0\0\xff");
        wave
    }

    #[test]
    fn native_ge_reports_and_queues_wave_resources() {
        const TABLE: u32 = 0x1000;
        const NAME: u32 = 0x1100;
        const INFO: u32 = 0x1200;
        const DATA: u32 = 0x1400;

        let mut api = NGameApi::new();
        let mut memory = Memory::new();
        memory
            .map_region(0x1000, 4096, crate::memory::Permission::ALL, "audio")
            .unwrap();
        memory
            .write_block(TABLE, b"effect.wav\0\0\0\0\0\0")
            .unwrap();
        memory.write_u32(TABLE + 16, DATA).unwrap();
        memory
            .write_block(TABLE + 20, b"TAEND\0\0\0\0\0\0\0\0\0\0\0")
            .unwrap();
        memory.write_u32(TABLE + 36, 0).unwrap();
        memory.write_block(NAME, b"effect.wav\0").unwrap();
        let wave = wave_resource();
        memory.write_block(DATA, &wave).unwrap();

        memory.set_register(crate::memory::REG_R1, TABLE);
        api.native_ge_init_res(&mut memory);
        memory.set_register(REG_R0, NAME);
        memory.set_register(crate::memory::REG_R1, INFO);
        api.native_ge_get_res(&mut memory);

        assert_eq!(memory.get_register(REG_R0), 1);
        assert_eq!(memory.read_u32(INFO).unwrap(), DATA);
        assert_eq!(memory.read_u32(INFO + 4).unwrap(), wave.len() as u32);

        memory.set_register(REG_R0, 1);
        memory.set_register(crate::memory::REG_R1, 1);
        memory.set_register(crate::memory::REG_R2, INFO);
        api.native_ge_play_res(&mut memory);
        assert!(matches!(
            api.audio_commands.as_slice(),
            [AudioCommand::Play {
                resource_type: 1,
                repeat: 1,
                data
            }] if data == &wave
        ));
    }

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
