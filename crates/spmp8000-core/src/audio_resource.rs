use crate::memory::Memory;

pub(crate) const RESOURCE_TYPE_WAV: u32 = 1;
pub(crate) const RESOURCE_TYPE_MIDI: u32 = 2;

const MAX_RESOURCE_SIZE: usize = 16 * 1024 * 1024;
const MAX_MIDI_SECONDS: usize = 10 * 60;

#[derive(Debug)]
pub(crate) enum AudioCommand {
    Play {
        resource_type: u32,
        repeat: u32,
        data: Vec<u8>,
    },
    Stop {
        resource_type: u32,
    },
}

pub(crate) fn inspect_resource(memory: &Memory, address: u32) -> Option<(u32, u32)> {
    let header = memory.read_block(address, 12).ok()?;
    if header.starts_with(b"RIFF") && &header[8..12] == b"WAVE" {
        let size = read_u32_le(&header, 4)?.checked_add(8)? as usize;
        return valid_resource_size(size).then_some((RESOURCE_TYPE_WAV, size as u32));
    }

    if header.starts_with(b"MThd") {
        let header_size = read_u32_be(&header, 4)? as usize;
        if header_size < 6 {
            return None;
        }
        let full_header_size = 8usize.checked_add(header_size)?;
        if full_header_size > MAX_RESOURCE_SIZE {
            return None;
        }
        let midi_header = memory.read_block(address, full_header_size).ok()?;
        let track_count = read_u16_be(&midi_header, 10)? as usize;
        let mut offset = 8usize.checked_add(header_size)?;

        for _ in 0..track_count {
            let chunk_address = address.checked_add(offset as u32)?;
            let chunk_header = memory.read_block(chunk_address, 8).ok()?;
            if !chunk_header.starts_with(b"MTrk") {
                return None;
            }
            let chunk_size = read_u32_be(&chunk_header, 4)? as usize;
            offset = offset.checked_add(8)?.checked_add(chunk_size)?;
            if !valid_resource_size(offset) {
                return None;
            }
        }

        return Some((RESOURCE_TYPE_MIDI, offset as u32));
    }

    None
}

pub(crate) fn decode_resource(
    resource_type: u32,
    data: &[u8],
    output_rate: u32,
) -> Result<Vec<i16>, String> {
    match resource_type {
        RESOURCE_TYPE_WAV => decode_wav(data, output_rate),
        RESOURCE_TYPE_MIDI => render_midi(data, output_rate),
        _ => Err(format!("unsupported audio resource type {resource_type}")),
    }
}

pub(crate) fn valid_resource_size(size: usize) -> bool {
    (12..=MAX_RESOURCE_SIZE).contains(&size)
}

fn decode_wav(data: &[u8], output_rate: u32) -> Result<Vec<i16>, String> {
    if data.len() < 12 || !data.starts_with(b"RIFF") || &data[8..12] != b"WAVE" {
        return Err("invalid RIFF/WAVE resource".to_string());
    }

    let mut offset = 12usize;
    let mut format = None;
    let mut pcm_data = None;

    while offset.checked_add(8).is_some_and(|end| end <= data.len()) {
        let chunk_id = &data[offset..offset + 4];
        let chunk_size = read_u32_le(data, offset + 4)
            .ok_or_else(|| "truncated WAVE chunk".to_string())? as usize;
        let chunk_start = offset + 8;
        let chunk_end = chunk_start
            .checked_add(chunk_size)
            .ok_or_else(|| "invalid WAVE chunk size".to_string())?;
        if chunk_end > data.len() {
            return Err("truncated WAVE chunk data".to_string());
        }

        match chunk_id {
            b"fmt " if chunk_size >= 16 => {
                format = Some(WaveFormat {
                    encoding: read_u16_le(data, chunk_start).unwrap_or(0),
                    channels: read_u16_le(data, chunk_start + 2).unwrap_or(0),
                    sample_rate: read_u32_le(data, chunk_start + 4).unwrap_or(0),
                    bits_per_sample: read_u16_le(data, chunk_start + 14).unwrap_or(0),
                });
            }
            b"data" => pcm_data = Some(&data[chunk_start..chunk_end]),
            _ => {}
        }

        offset = chunk_end + (chunk_size & 1);
    }

    let format = format.ok_or_else(|| "WAVE resource has no fmt chunk".to_string())?;
    let pcm_data = pcm_data.ok_or_else(|| "WAVE resource has no data chunk".to_string())?;
    if format.encoding != 1 {
        return Err(format!("unsupported WAVE encoding {}", format.encoding));
    }
    if !(format.channels == 1 || format.channels == 2) {
        return Err(format!(
            "unsupported WAVE channel count {}",
            format.channels
        ));
    }
    if !(format.bits_per_sample == 8 || format.bits_per_sample == 16) {
        return Err(format!(
            "unsupported WAVE bit depth {}",
            format.bits_per_sample
        ));
    }
    if format.sample_rate == 0 || output_rate == 0 {
        return Err("invalid WAVE sample rate".to_string());
    }

    let bytes_per_sample = usize::from(format.bits_per_sample / 8);
    let frame_size = bytes_per_sample * usize::from(format.channels);
    let input_frames = pcm_data.len() / frame_size;
    if input_frames == 0 {
        return Ok(Vec::new());
    }

    let output_frames = ((input_frames as u64 * u64::from(output_rate))
        / u64::from(format.sample_rate))
    .max(1) as usize;
    let mut output = Vec::with_capacity(output_frames * 2);

    for output_frame in 0..output_frames {
        let source_position = output_frame as f64 * format.sample_rate as f64 / output_rate as f64;
        let first_frame = (source_position.floor() as usize).min(input_frames - 1);
        let second_frame = (first_frame + 1).min(input_frames - 1);
        let fraction = (source_position - first_frame as f64) as f32;

        for channel in 0..2 {
            let source_channel = channel.min(usize::from(format.channels) - 1);
            let first = wave_sample(
                pcm_data,
                first_frame,
                source_channel,
                format.channels,
                format.bits_per_sample,
            );
            let second = wave_sample(
                pcm_data,
                second_frame,
                source_channel,
                format.channels,
                format.bits_per_sample,
            );
            output.push((first + (second - first) * fraction).round() as i16);
        }
    }

    Ok(output)
}

#[derive(Debug, Clone, Copy)]
struct WaveFormat {
    encoding: u16,
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
}

fn wave_sample(
    data: &[u8],
    frame: usize,
    channel: usize,
    channels: u16,
    bits_per_sample: u16,
) -> f32 {
    let bytes_per_sample = usize::from(bits_per_sample / 8);
    let offset = (frame * usize::from(channels) + channel) * bytes_per_sample;
    match bits_per_sample {
        8 => (i16::from(data[offset]) - 128) as f32 * 256.0,
        16 => i16::from_le_bytes([data[offset], data[offset + 1]]) as f32,
        _ => 0.0,
    }
}

#[derive(Debug, Clone, Copy)]
enum MidiMessage {
    NoteOn { channel: u8, note: u8, velocity: u8 },
    NoteOff { channel: u8, note: u8 },
    Control { channel: u8, control: u8, value: u8 },
    Program { channel: u8, program: u8 },
    Tempo(u32),
}

#[derive(Debug, Clone, Copy)]
struct MidiEvent {
    tick: u64,
    order: usize,
    message: MidiMessage,
}

fn render_midi(data: &[u8], sample_rate: u32) -> Result<Vec<i16>, String> {
    if sample_rate == 0 {
        return Err("invalid MIDI output sample rate".to_string());
    }
    let (ticks_per_quarter, mut events) = parse_midi(data)?;
    events.sort_by_key(|event| (event.tick, event.order));

    let mut synth = MidiSynth::new(sample_rate);
    let mut output = Vec::new();
    let mut current_tick = 0u64;
    let mut tempo = 500_000u32;
    let mut fractional_samples = 0.0f64;
    let max_frames = sample_rate as usize * MAX_MIDI_SECONDS;

    for event in events {
        let delta_ticks = event.tick.saturating_sub(current_tick);
        let exact_samples = delta_ticks as f64 * tempo as f64 * sample_rate as f64
            / (ticks_per_quarter as f64 * 1_000_000.0)
            + fractional_samples;
        let sample_count = exact_samples.floor() as usize;
        fractional_samples = exact_samples - sample_count as f64;
        if sample_count > max_frames.saturating_sub(output.len() / 2) {
            return Err("MIDI resource exceeds playback limit".to_string());
        }
        synth.render(sample_count, &mut output);
        current_tick = event.tick;

        match event.message {
            MidiMessage::Tempo(value) if value > 0 => tempo = value,
            message => synth.handle(message),
        }
    }

    synth.release_all();
    let tail_frames = (sample_rate / 4) as usize;
    synth.render(
        tail_frames.min(max_frames.saturating_sub(output.len() / 2)),
        &mut output,
    );
    Ok(output)
}

fn parse_midi(data: &[u8]) -> Result<(u16, Vec<MidiEvent>), String> {
    if data.len() < 14 || !data.starts_with(b"MThd") {
        return Err("invalid MIDI header".to_string());
    }
    let header_size =
        read_u32_be(data, 4).ok_or_else(|| "truncated MIDI header".to_string())? as usize;
    if header_size < 6 || 8 + header_size > data.len() {
        return Err("invalid MIDI header size".to_string());
    }
    let format = read_u16_be(data, 8).ok_or_else(|| "missing MIDI format".to_string())?;
    if format > 1 {
        return Err(format!("unsupported MIDI format {format}"));
    }
    let track_count = read_u16_be(data, 10).ok_or_else(|| "missing MIDI tracks".to_string())?;
    let division = read_u16_be(data, 12).ok_or_else(|| "missing MIDI division".to_string())?;
    if division == 0 || division & 0x8000 != 0 {
        return Err("unsupported MIDI time division".to_string());
    }

    let mut offset = 8 + header_size;
    let mut events = Vec::new();
    let mut order = 0usize;

    for _ in 0..track_count {
        if offset + 8 > data.len() || &data[offset..offset + 4] != b"MTrk" {
            return Err("missing MIDI track chunk".to_string());
        }
        let track_size = read_u32_be(data, offset + 4)
            .ok_or_else(|| "truncated MIDI track".to_string())? as usize;
        let track_start = offset + 8;
        let track_end = track_start
            .checked_add(track_size)
            .filter(|end| *end <= data.len())
            .ok_or_else(|| "invalid MIDI track size".to_string())?;
        parse_midi_track(&data[track_start..track_end], &mut events, &mut order)?;
        offset = track_end;
    }

    Ok((division, events))
}

fn parse_midi_track(
    track: &[u8],
    events: &mut Vec<MidiEvent>,
    order: &mut usize,
) -> Result<(), String> {
    let mut offset = 0usize;
    let mut tick = 0u64;
    let mut running_status = None;

    while offset < track.len() {
        let delta = read_vlq(track, &mut offset)?;
        tick = tick.saturating_add(u64::from(delta));
        let first = *track
            .get(offset)
            .ok_or_else(|| "truncated MIDI event".to_string())?;
        let status = if first & 0x80 != 0 {
            offset += 1;
            if first < 0xf0 {
                running_status = Some(first);
            }
            first
        } else {
            running_status.ok_or_else(|| "MIDI running status is missing".to_string())?
        };

        if status == 0xff {
            running_status = None;
            let meta_type = take_byte(track, &mut offset)?;
            let length = read_vlq(track, &mut offset)? as usize;
            let end = offset
                .checked_add(length)
                .filter(|end| *end <= track.len())
                .ok_or_else(|| "truncated MIDI meta event".to_string())?;
            if meta_type == 0x51 && length == 3 {
                let tempo =
                    u32::from_be_bytes([0, track[offset], track[offset + 1], track[offset + 2]]);
                push_midi_event(events, order, tick, MidiMessage::Tempo(tempo));
            }
            offset = end;
            if meta_type == 0x2f {
                break;
            }
            continue;
        }

        if status == 0xf0 || status == 0xf7 {
            running_status = None;
            let length = read_vlq(track, &mut offset)? as usize;
            offset = offset
                .checked_add(length)
                .filter(|end| *end <= track.len())
                .ok_or_else(|| "truncated MIDI system event".to_string())?;
            continue;
        }

        let channel = status & 0x0f;
        match status & 0xf0 {
            0x80 => {
                let note = take_byte(track, &mut offset)?;
                let _velocity = take_byte(track, &mut offset)?;
                push_midi_event(events, order, tick, MidiMessage::NoteOff { channel, note });
            }
            0x90 => {
                let note = take_byte(track, &mut offset)?;
                let velocity = take_byte(track, &mut offset)?;
                let message = if velocity == 0 {
                    MidiMessage::NoteOff { channel, note }
                } else {
                    MidiMessage::NoteOn {
                        channel,
                        note,
                        velocity,
                    }
                };
                push_midi_event(events, order, tick, message);
            }
            0xa0 => {
                let _note = take_byte(track, &mut offset)?;
                let _pressure = take_byte(track, &mut offset)?;
            }
            0xb0 => {
                let control = take_byte(track, &mut offset)?;
                let value = take_byte(track, &mut offset)?;
                push_midi_event(
                    events,
                    order,
                    tick,
                    MidiMessage::Control {
                        channel,
                        control,
                        value,
                    },
                );
            }
            0xc0 => {
                let program = take_byte(track, &mut offset)?;
                push_midi_event(
                    events,
                    order,
                    tick,
                    MidiMessage::Program { channel, program },
                );
            }
            0xd0 => {
                let _pressure = take_byte(track, &mut offset)?;
            }
            0xe0 => {
                let _least = take_byte(track, &mut offset)?;
                let _most = take_byte(track, &mut offset)?;
            }
            _ => return Err(format!("unsupported MIDI status 0x{status:02x}")),
        }
    }

    Ok(())
}

fn push_midi_event(
    events: &mut Vec<MidiEvent>,
    order: &mut usize,
    tick: u64,
    message: MidiMessage,
) {
    events.push(MidiEvent {
        tick,
        order: *order,
        message,
    });
    *order += 1;
}

fn take_byte(data: &[u8], offset: &mut usize) -> Result<u8, String> {
    let byte = *data
        .get(*offset)
        .ok_or_else(|| "truncated MIDI event data".to_string())?;
    *offset += 1;
    Ok(byte)
}

fn read_vlq(data: &[u8], offset: &mut usize) -> Result<u32, String> {
    let mut value = 0u32;
    for _ in 0..4 {
        let byte = take_byte(data, offset)?;
        value = (value << 7) | u32::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("invalid MIDI variable-length value".to_string())
}

#[derive(Debug, Clone, Copy)]
struct MidiChannel {
    program: u8,
    volume: f32,
    expression: f32,
    pan: f32,
}

impl Default for MidiChannel {
    fn default() -> Self {
        Self {
            program: 0,
            volume: 100.0 / 127.0,
            expression: 1.0,
            pan: 0.5,
        }
    }
}

#[derive(Debug)]
struct MidiVoice {
    channel: u8,
    note: u8,
    program: u8,
    velocity: f32,
    phase: f32,
    age: usize,
    release_remaining: Option<usize>,
    release_length: usize,
    percussion_length: Option<usize>,
    noise: u32,
}

#[derive(Debug)]
struct MidiSynth {
    sample_rate: u32,
    channels: [MidiChannel; 16],
    voices: Vec<MidiVoice>,
}

impl MidiSynth {
    fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            channels: [MidiChannel::default(); 16],
            voices: Vec::new(),
        }
    }

    fn handle(&mut self, message: MidiMessage) {
        match message {
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } => self.note_on(channel, note, velocity),
            MidiMessage::NoteOff { channel, note } => self.note_off(channel, note),
            MidiMessage::Control {
                channel,
                control,
                value,
            } => self.control(channel, control, value),
            MidiMessage::Program { channel, program } => {
                self.channels[usize::from(channel)].program = program;
            }
            MidiMessage::Tempo(_) => {}
        }
    }

    fn note_on(&mut self, channel: u8, note: u8, velocity: u8) {
        self.note_off(channel, note);
        let percussion_length = (channel == 9).then_some((self.sample_rate / 4) as usize);
        self.voices.push(MidiVoice {
            channel,
            note,
            program: self.channels[usize::from(channel)].program,
            velocity: f32::from(velocity) / 127.0,
            phase: 0.0,
            age: 0,
            release_remaining: None,
            release_length: (self.sample_rate / 20).max(1) as usize,
            percussion_length,
            noise: 0x9e37_79b9 ^ (u32::from(note) << 16),
        });
    }

    fn note_off(&mut self, channel: u8, note: u8) {
        for voice in self
            .voices
            .iter_mut()
            .filter(|voice| voice.channel == channel && voice.note == note)
        {
            if voice.release_remaining.is_none() {
                voice.release_remaining = Some(voice.release_length);
            }
        }
    }

    fn control(&mut self, channel: u8, control: u8, value: u8) {
        let state = &mut self.channels[usize::from(channel)];
        match control {
            7 => state.volume = f32::from(value) / 127.0,
            10 => state.pan = f32::from(value) / 127.0,
            11 => state.expression = f32::from(value) / 127.0,
            120 | 123 => {
                for voice in self
                    .voices
                    .iter_mut()
                    .filter(|voice| voice.channel == channel)
                {
                    voice.release_remaining = Some(voice.release_length);
                }
            }
            _ => {}
        }
    }

    fn release_all(&mut self) {
        for voice in &mut self.voices {
            voice.release_remaining = Some(voice.release_length);
        }
    }

    fn render(&mut self, frames: usize, output: &mut Vec<i16>) {
        output.reserve(frames * 2);
        for _ in 0..frames {
            let mut left = 0.0f32;
            let mut right = 0.0f32;

            for voice in &mut self.voices {
                let channel = self.channels[usize::from(voice.channel)];
                let envelope = voice.envelope(self.sample_rate);
                let sample = voice.next_sample(self.sample_rate)
                    * voice.velocity
                    * channel.volume
                    * channel.expression
                    * envelope
                    * 0.18;
                left += sample * (1.0 - channel.pan).sqrt();
                right += sample * channel.pan.sqrt();
            }

            output.push(float_to_i16(left));
            output.push(float_to_i16(right));
            self.voices.retain(|voice| !voice.finished());
        }
    }
}

impl MidiVoice {
    fn envelope(&self, sample_rate: u32) -> f32 {
        let attack_length = (sample_rate / 100).max(1) as usize;
        let attack = (self.age as f32 / attack_length as f32).min(1.0);
        let release = self.release_remaining.map_or(1.0, |remaining| {
            remaining as f32 / self.release_length as f32
        });
        let percussion = self.percussion_length.map_or(1.0, |length| {
            (1.0 - self.age as f32 / length as f32).max(0.0)
        });
        attack * release * percussion
    }

    fn next_sample(&mut self, sample_rate: u32) -> f32 {
        let frequency = 440.0 * 2.0f32.powf((f32::from(self.note) - 69.0) / 12.0);
        let sample = if self.channel == 9 {
            self.noise ^= self.noise << 13;
            self.noise ^= self.noise >> 17;
            self.noise ^= self.noise << 5;
            (self.noise as i32 as f32) / i32::MAX as f32
        } else {
            match self.program / 8 {
                0 => {
                    (std::f32::consts::TAU * self.phase).sin()
                        + 0.35 * (std::f32::consts::TAU * self.phase * 2.0).sin()
                }
                1 | 2 => 1.0 - 4.0 * (self.phase - 0.5).abs(),
                3..=5 => 2.0 * self.phase - 1.0,
                6 | 7 => {
                    if self.phase < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                _ => (std::f32::consts::TAU * self.phase).sin(),
            }
        };

        self.phase = (self.phase + frequency / sample_rate as f32).fract();
        self.age += 1;
        if let Some(remaining) = &mut self.release_remaining {
            *remaining = remaining.saturating_sub(1);
        }
        sample
    }

    fn finished(&self) -> bool {
        self.release_remaining == Some(0)
            || self
                .percussion_length
                .is_some_and(|length| self.age >= length)
    }
}

fn float_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

fn read_u16_le(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u16_be(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32_le(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u32_be(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Permission;

    fn test_wave() -> Vec<u8> {
        let samples = [0u8, 128, 255, 128];
        let mut wave = Vec::new();
        wave.extend_from_slice(b"RIFF");
        wave.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
        wave.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
        wave.extend_from_slice(&8_000u32.to_le_bytes());
        wave.extend_from_slice(&8_000u32.to_le_bytes());
        wave.extend_from_slice(&1u16.to_le_bytes());
        wave.extend_from_slice(&8u16.to_le_bytes());
        wave.extend_from_slice(b"data");
        wave.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        wave.extend_from_slice(&samples);
        wave
    }

    fn test_midi() -> Vec<u8> {
        let track = [
            0x00, 0x90, 60, 100, 0x83, 0x60, 0x80, 60, 0, 0x00, 0xff, 0x2f, 0x00,
        ];
        let mut midi = Vec::new();
        midi.extend_from_slice(b"MThd\0\0\0\x06\0\0\0\x01\x01\xe0MTrk");
        midi.extend_from_slice(&(track.len() as u32).to_be_bytes());
        midi.extend_from_slice(&track);
        midi
    }

    #[test]
    fn inspects_wave_and_midi_resource_lengths() {
        let wave = test_wave();
        let midi = test_midi();
        let mut memory = Memory::new();
        memory
            .map_region(0x1000, 8192, Permission::ALL, "audio")
            .unwrap();
        memory.write_block(0x1000, &wave).unwrap();
        memory.write_block(0x2000, &midi).unwrap();

        assert_eq!(
            inspect_resource(&memory, 0x1000),
            Some((RESOURCE_TYPE_WAV, wave.len() as u32))
        );
        assert_eq!(
            inspect_resource(&memory, 0x2000),
            Some((RESOURCE_TYPE_MIDI, midi.len() as u32))
        );
    }

    #[test]
    fn decodes_unsigned_mono_wave_to_stereo() {
        let decoded = decode_wav(&test_wave(), 8_000).unwrap();

        assert_eq!(decoded.len(), 8);
        assert_eq!(decoded[0], i16::MIN);
        assert_eq!(decoded[1], i16::MIN);
        assert_eq!(decoded[2], 0);
        assert_eq!(decoded[3], 0);
        assert!(decoded[4] > 32_000);
        assert_eq!(decoded[4], decoded[5]);
    }

    #[test]
    fn renders_standard_midi_as_non_silent_stereo() {
        let rendered = render_midi(&test_midi(), 8_000).unwrap();

        assert!(rendered.len() >= 8_000);
        assert!(rendered.iter().any(|sample| *sample != 0));
        assert_eq!(rendered.len() % 2, 0);
    }
}
