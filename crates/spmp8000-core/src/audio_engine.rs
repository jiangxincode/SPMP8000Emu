// Audio engine for SPMP8000 emulator
//
// Handles audio buffer management and sample conversion

use crate::memory::Memory;

/// Audio sample format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    S16LE, // Signed 16-bit little-endian
    U8,    // Unsigned 8-bit
}

/// Audio engine state
#[derive(Debug)]
pub struct AudioEngine {
    /// Sample rate (Hz)
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo)
    pub channels: u32,
    /// Sample format
    pub format: SampleFormat,
    /// Audio buffer (samples ready for output)
    buffer: Vec<i16>,
    /// Maximum buffer size in samples
    max_buffer_size: usize,
}

impl AudioEngine {
    /// Create a new audio engine
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            channels: 1,
            format: SampleFormat::S16LE,
            buffer: Vec::new(),
            max_buffer_size: 4096,
        }
    }

    /// Set audio parameters
    pub fn set_params(&mut self, sample_rate: u32, channels: u32) {
        self.sample_rate = sample_rate;
        self.channels = channels;
        log::info!("Audio: {}Hz, {} channels", sample_rate, channels);
    }

    /// Update audio buffer from emulated memory
    pub fn update_from_memory(&mut self, memory: &Memory, addr: u32, size: u32) {
        if addr == 0 || size == 0 {
            return;
        }

        self.buffer.clear();

        let num_samples = (size / 2) as usize; // 16-bit samples
        let samples_to_read = num_samples.min(self.max_buffer_size);

        for i in 0..samples_to_read {
            if let Ok(sample) = memory.read_u16(addr + (i as u32 * 2)) {
                self.buffer.push(sample as i16);
            }
        }
    }

    /// Get the audio buffer
    pub fn get_buffer(&self) -> &[i16] {
        &self.buffer
    }

    /// Get mutable audio buffer
    pub fn get_buffer_mut(&mut self) -> &mut Vec<i16> {
        &mut self.buffer
    }

    /// Clear the audio buffer
    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
    }

    /// Get the number of samples in the buffer
    pub fn samples_available(&self) -> usize {
        self.buffer.len()
    }

    /// Get the duration of buffered audio in milliseconds
    pub fn buffer_duration_ms(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        (self.buffer.len() as f64 / self.sample_rate as f64) * 1000.0
    }

    /// Mix audio from multiple sources
    pub fn mix(&mut self, other: &[i16]) {
        let max_len = self.buffer.len().max(other.len());
        self.buffer.resize(max_len, 0);

        for (i, &sample) in other.iter().enumerate() {
            let mixed = self.buffer[i] as i32 + sample as i32;
            self.buffer[i] = mixed.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        }
    }

    /// Apply volume scaling
    pub fn apply_volume(&mut self, volume: f32) {
        for sample in self.buffer.iter_mut() {
            let scaled = *sample as f32 * volume;
            *sample = scaled.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        }
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new(22050)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;

    #[test]
    fn test_audio_engine_creation() {
        let engine = AudioEngine::new(22050);
        assert_eq!(engine.sample_rate, 22050);
        assert_eq!(engine.channels, 1);
    }

    #[test]
    fn test_audio_buffer_update() {
        let mut engine = AudioEngine::new(22050);
        let mut memory = Memory::new();
        memory.map_region(0x1000, 4096, crate::memory::Permission::ALL, "test").unwrap();

        // Write some samples
        memory.write_u16(0x1000, 1000).unwrap();
        memory.write_u16(0x1002, -1000i16 as u16).unwrap();
        memory.write_u16(0x1004, 500).unwrap();

        engine.update_from_memory(&memory, 0x1000, 6);

        assert_eq!(engine.samples_available(), 3);
        assert_eq!(engine.get_buffer()[0], 1000);
        assert_eq!(engine.get_buffer()[1], -1000);
        assert_eq!(engine.get_buffer()[2], 500);
    }

    #[test]
    fn test_audio_mix() {
        let mut engine = AudioEngine::new(22050);
        engine.buffer = vec![100, 200, 300];

        let other = vec![50, 60, 70, 80];
        engine.mix(&other);

        assert_eq!(engine.buffer, vec![150, 260, 370, 80]);
    }

    #[test]
    fn test_audio_volume() {
        let mut engine = AudioEngine::new(22050);
        engine.buffer = vec![1000, -1000, 500];

        engine.apply_volume(0.5);

        assert_eq!(engine.buffer[0], 500);
        assert_eq!(engine.buffer[1], -500);
        assert_eq!(engine.buffer[2], 250);
    }
}
