// Audio engine for SPMP8000 emulator

use std::collections::HashMap;

use crate::audio_resource::{decode_resource, AudioCommand};
use crate::memory::Memory;

/// Audio sample format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    S16LE,
    U8,
}

#[derive(Debug)]
struct Playback {
    samples: Vec<i16>,
    position: usize,
    remaining_plays: u32,
}

impl Playback {
    fn new(samples: Vec<i16>, repeat: u32) -> Self {
        Self {
            samples,
            position: 0,
            remaining_plays: repeat,
        }
    }

    fn next_stereo_frame(&mut self) -> Option<(i16, i16)> {
        if self.samples.len() < 2 {
            return None;
        }
        if self.position >= self.samples.len() {
            if self.remaining_plays == 1 {
                return None;
            }
            if self.remaining_plays > 1 {
                self.remaining_plays -= 1;
            }
            self.position = 0;
        }

        let frame = (self.samples[self.position], self.samples[self.position + 1]);
        self.position += 2;
        Some(frame)
    }
}

/// Audio engine state
#[derive(Debug)]
pub struct AudioEngine {
    /// Output sample rate in Hz
    pub sample_rate: u32,
    /// Output channel count
    pub channels: u32,
    /// Output sample format
    pub format: SampleFormat,
    /// Interleaved stereo samples generated for the current video frame
    buffer: Vec<i16>,
    resource_playbacks: HashMap<u32, Playback>,
    volume: f32,
}

impl AudioEngine {
    /// Create a new audio engine
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            channels: 2,
            format: SampleFormat::S16LE,
            buffer: Vec::new(),
            resource_playbacks: HashMap::new(),
            volume: 1.0,
        }
    }

    /// Set streamed PCM parameters
    pub fn set_params(&mut self, sample_rate: u32, channels: u32) {
        self.sample_rate = sample_rate;
        self.channels = channels;
        log::info!("Audio: {}Hz, {} channels", sample_rate, channels);
    }

    /// Set output volume as a percentage
    pub fn set_volume(&mut self, volume: u32) {
        self.volume = volume.min(100) as f32 / 100.0;
    }

    pub(crate) fn handle_command(&mut self, command: AudioCommand) {
        match command {
            AudioCommand::Play {
                resource_type,
                repeat,
                data,
            } => match decode_resource(resource_type, &data, self.sample_rate) {
                Ok(samples) if !samples.is_empty() => {
                    log::debug!(
                        "Decoded audio resource type {} to {} stereo frames",
                        resource_type,
                        samples.len() / 2
                    );
                    self.resource_playbacks
                        .insert(resource_type, Playback::new(samples, repeat));
                }
                Ok(_) => {
                    log::warn!("Audio resource type {} is empty", resource_type);
                }
                Err(error) => {
                    log::warn!(
                        "Failed to decode audio resource type {}: {}",
                        resource_type,
                        error
                    );
                }
            },
            AudioCommand::Stop { resource_type } => {
                self.resource_playbacks.remove(&resource_type);
            }
        }
    }

    /// Generate one video frame of interleaved stereo audio
    pub fn render_frame(&mut self, memory: &Memory, streamed_pcm: Option<(u32, u32, u32)>) {
        let frames = (self.sample_rate / 30) as usize;
        let mut mixed = vec![0i32; frames * 2];

        if let Some((address, size, channels)) = streamed_pcm {
            mix_streamed_pcm(&mut mixed, memory, address, size, channels);
        }

        let mut finished = Vec::new();
        for (resource_type, playback) in &mut self.resource_playbacks {
            for frame in 0..frames {
                let Some((left, right)) = playback.next_stereo_frame() else {
                    finished.push(*resource_type);
                    break;
                };
                mixed[frame * 2] += i32::from(left);
                mixed[frame * 2 + 1] += i32::from(right);
            }
        }
        for resource_type in finished {
            self.resource_playbacks.remove(&resource_type);
        }

        self.buffer.clear();
        self.buffer.reserve(mixed.len());
        self.buffer.extend(mixed.into_iter().map(|sample| {
            (sample as f32 * self.volume)
                .round()
                .clamp(i16::MIN as f32, i16::MAX as f32) as i16
        }));
    }

    /// Update the output buffer from signed 16-bit mono memory
    pub fn update_from_memory(&mut self, memory: &Memory, address: u32, size: u32) {
        let frames = (size / 2) as usize;
        self.buffer.clear();
        self.buffer.reserve(frames * 2);
        for index in 0..frames {
            if let Ok(sample) = memory.read_u16(address + index as u32 * 2) {
                let sample = sample as i16;
                self.buffer.push(sample);
                self.buffer.push(sample);
            }
        }
    }

    /// Get the current interleaved stereo output buffer
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

    /// Get the number of interleaved samples in the buffer
    pub fn samples_available(&self) -> usize {
        self.buffer.len()
    }

    /// Get the duration of buffered audio in milliseconds
    pub fn buffer_duration_ms(&self) -> f64 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0.0;
        }
        self.buffer.len() as f64 / (self.sample_rate * self.channels) as f64 * 1000.0
    }

    /// Mix interleaved samples into the current buffer
    pub fn mix(&mut self, other: &[i16]) {
        let max_len = self.buffer.len().max(other.len());
        self.buffer.resize(max_len, 0);

        for (index, &sample) in other.iter().enumerate() {
            let mixed = i32::from(self.buffer[index]) + i32::from(sample);
            self.buffer[index] = mixed.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        }
    }

    /// Apply volume scaling to the current buffer
    pub fn apply_volume(&mut self, volume: f32) {
        for sample in &mut self.buffer {
            let scaled = *sample as f32 * volume;
            *sample = scaled.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        }
    }
}

fn mix_streamed_pcm(mixed: &mut [i32], memory: &Memory, address: u32, size: u32, channels: u32) {
    if address == 0 || size < 2 {
        return;
    }
    let channels = channels.clamp(1, 2) as usize;
    let available_frames = size as usize / (2 * channels);
    let frames = available_frames.min(mixed.len() / 2);

    for frame in 0..frames {
        let first_address = address + (frame * channels * 2) as u32;
        let Ok(left) = memory.read_u16(first_address) else {
            break;
        };
        let right = if channels == 2 {
            memory.read_u16(first_address + 2).unwrap_or(left)
        } else {
            left
        };
        mixed[frame * 2] += i32::from(left as i16);
        mixed[frame * 2 + 1] += i32::from(right as i16);
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
    use crate::audio_resource::RESOURCE_TYPE_WAV;
    use crate::memory::Permission;

    fn wave_resource() -> Vec<u8> {
        let samples = [0u8, 255, 0, 255];
        let mut wave = Vec::new();
        wave.extend_from_slice(b"RIFF");
        wave.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
        wave.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
        wave.extend_from_slice(&22_050u32.to_le_bytes());
        wave.extend_from_slice(&22_050u32.to_le_bytes());
        wave.extend_from_slice(&1u16.to_le_bytes());
        wave.extend_from_slice(&8u16.to_le_bytes());
        wave.extend_from_slice(b"data");
        wave.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        wave.extend_from_slice(&samples);
        wave
    }

    #[test]
    fn test_audio_engine_creation() {
        let engine = AudioEngine::new(22050);
        assert_eq!(engine.sample_rate, 22050);
        assert_eq!(engine.channels, 2);
    }

    #[test]
    fn test_audio_buffer_update_duplicates_mono() {
        let mut engine = AudioEngine::new(22050);
        let mut memory = Memory::new();
        memory
            .map_region(0x1000, 4096, Permission::ALL, "test")
            .unwrap();
        memory.write_u16(0x1000, 1000).unwrap();
        memory.write_u16(0x1002, -1000i16 as u16).unwrap();

        engine.update_from_memory(&memory, 0x1000, 4);

        assert_eq!(engine.get_buffer(), [1000, 1000, -1000, -1000]);
    }

    #[test]
    fn resource_playback_renders_exactly_one_frame() {
        let mut engine = AudioEngine::new(22_050);
        engine.handle_command(AudioCommand::Play {
            resource_type: RESOURCE_TYPE_WAV,
            repeat: 1,
            data: wave_resource(),
        });

        engine.render_frame(&Memory::new(), None);

        assert_eq!(engine.samples_available(), 1_470);
        assert!(engine.get_buffer()[..8].iter().any(|sample| *sample != 0));
        assert!(engine.get_buffer()[8..].iter().all(|sample| *sample == 0));
    }

    #[test]
    fn test_audio_mix() {
        let mut engine = AudioEngine::new(22050);
        engine.buffer = vec![100, 200, 300];
        engine.mix(&[50, 60, 70, 80]);
        assert_eq!(engine.buffer, vec![150, 260, 370, 80]);
    }

    #[test]
    fn test_audio_volume() {
        let mut engine = AudioEngine::new(22050);
        engine.buffer = vec![1000, -1000, 500];
        engine.apply_volume(0.5);
        assert_eq!(engine.buffer, vec![500, -500, 250]);
    }
}
