// emuIf audio API implementation

use super::NGameApi;
use crate::memory::Memory;

/// emuIf audio parameter structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct EmuSoundParams {
    pub buf: u32,           // Audio buffer address
    pub buf_size: u32,      // Buffer size in bytes
    pub rate: u32,          // Sample rate (Hz)
    pub depth: u8,          // Bit depth (usually 16)
    pub channels: u8,       // Number of channels (1 or 2)
    pub _padding: u16,
    pub callback: u32,      // Callback function address
}

impl NGameApi {
    /// emuIfSoundInit - Initialize audio subsystem
    pub fn emu_if_sound_init(&mut self, memory: &mut Memory) {
        let params_addr = memory.get_register(crate::memory::REG_R0);

        if params_addr != 0 {
            let rate = memory.read_u32(params_addr + 8).unwrap_or(22050);
            let channels = memory.read_u8(params_addr + 13).unwrap_or(1) as u32;

            log::info!("emuIfSoundInit: {}Hz, {} channels", rate, channels);

            self.audio_sample_rate = rate;
            self.audio_channels = channels;
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// emuIfSoundPlay - Play audio buffer
    pub fn emu_if_sound_play(&mut self, memory: &mut Memory) {
        let params_addr = memory.get_register(crate::memory::REG_R0);

        if params_addr != 0 {
            let buf = memory.read_u32(params_addr).unwrap_or(0);
            let buf_size = memory.read_u32(params_addr + 4).unwrap_or(0);

            if buf != 0 && buf_size > 0 {
                self.audio_buffer_addr = Some(buf);
                self.audio_buffer_size = buf_size;

                log::debug!("emuIfSoundPlay: {} bytes at 0x{:08X}", buf_size, buf);
            }
        }

        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// emuIfSoundCleanup - Cleanup audio subsystem
    pub fn emu_if_sound_cleanup(&mut self, memory: &mut Memory) {
        log::info!("emuIfSoundCleanup");
        self.audio_buffer_addr = None;
        self.audio_buffer_size = 0;
        memory.set_register(crate::memory::REG_R0, 0);
    }

    /// Get the current audio buffer data
    pub fn get_audio_buffer(&self, memory: &Memory) -> Option<Vec<i16>> {
        if let Some(addr) = self.audio_buffer_addr {
            if self.audio_buffer_size > 0 {
                let num_samples = (self.audio_buffer_size / 2) as usize;
                let mut samples = Vec::with_capacity(num_samples);

                for i in 0..num_samples {
                    let sample = memory.read_u16(addr + (i as u32 * 2)).unwrap_or(0);
                    samples.push(sample as i16);
                }

                return Some(samples);
            }
        }
        None
    }
}
