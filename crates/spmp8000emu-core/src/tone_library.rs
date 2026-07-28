const PATCH_COUNT: usize = 256;
const PATCH_TABLE_SIZE: usize = PATCH_COUNT * 4;
const MELODIC_PATCH_COUNT: usize = 128;
const PERCUSSION_PATCH_BASE: usize = 128;
const MAX_ALIAS_DEPTH: usize = 16;
const MAX_ZONE_COUNT: usize = 32;

pub(crate) const EMBEDDED_TONE_LIBRARY: &[u8] = include_bytes!("../assets/tonelib.bin");

#[derive(Debug, Clone, Copy)]
pub(crate) struct ToneSample<'a> {
    pub(crate) data: &'a [u8],
    pub(crate) sample_rate: u32,
    pub(crate) root_key: u8,
    pub(crate) loop_start: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ToneLibrary<'a> {
    data: &'a [u8],
}

impl<'a> ToneLibrary<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        (data.len() >= PATCH_TABLE_SIZE).then_some(Self { data })
    }

    pub(crate) fn melodic_sample(&self, program: u8, note: u8) -> Option<ToneSample<'a>> {
        let record_offset = self.resolve_melodic_patch(usize::from(program))?;
        let zone_count = usize::try_from(self.read_u32(record_offset)?).ok()?;
        if !(1..=MAX_ZONE_COUNT).contains(&zone_count) {
            return None;
        }

        let key_table = self.relative_offset(record_offset, record_offset + 4)?;
        let wave_table = self.relative_offset(record_offset, record_offset + 8)?;
        let mut selected_zone = zone_count - 1;
        for zone in 0..zone_count {
            let key_offset = key_table.checked_add(zone.checked_mul(2)?)?;
            if note <= u8::try_from(self.read_u16(key_offset)?).ok()? {
                selected_zone = zone;
                break;
            }
        }

        let wave_entry = wave_table.checked_add(selected_zone.checked_mul(4)?)?;
        let wave_offset = self.relative_offset(record_offset, wave_entry)?;
        let sample_rate = u32::from(self.read_u16(wave_offset)?);
        let end_index = usize::from(self.read_u16(wave_offset + 2)?);
        let loop_start = usize::from(self.read_u16(wave_offset + 4)?);
        let root_key = u8::try_from(self.read_u16(wave_offset + 6)?).ok()?;
        let sample_start = wave_offset.checked_add(8)?;
        let sample_end = sample_start.checked_add(end_index.checked_add(1)?)?;
        let data = self.data.get(sample_start..sample_end)?;
        if sample_rate == 0 || loop_start >= data.len() {
            return None;
        }

        Some(ToneSample {
            data,
            sample_rate,
            root_key,
            loop_start: Some(loop_start),
        })
    }

    pub(crate) fn percussion_sample(&self, note: u8) -> Option<ToneSample<'a>> {
        let record_offset = self.resolve_percussion_patch(usize::from(note))?;
        let wave_offset = self.relative_offset(record_offset, record_offset + 4)?;
        let sample_rate = u32::from(self.read_u16(wave_offset)?);
        let end_index = usize::from(self.read_u16(wave_offset + 2)?);
        let sample_start = wave_offset.checked_add(4)?;
        let sample_end = sample_start.checked_add(end_index.checked_add(1)?)?;
        let data = self.data.get(sample_start..sample_end)?;
        if sample_rate == 0 {
            return None;
        }

        Some(ToneSample {
            data,
            sample_rate,
            root_key: note,
            loop_start: None,
        })
    }

    fn resolve_melodic_patch(&self, program: usize) -> Option<usize> {
        let mut patch = program;
        for _ in 0..MAX_ALIAS_DEPTH {
            if patch >= MELODIC_PATCH_COUNT {
                return None;
            }
            let value = usize::try_from(self.read_u32(patch.checked_mul(4)?)?).ok()?;
            if value >= PATCH_TABLE_SIZE {
                return (value < self.data.len()).then_some(value);
            }
            patch = value;
        }
        None
    }

    fn resolve_percussion_patch(&self, note: usize) -> Option<usize> {
        let mut patch = PERCUSSION_PATCH_BASE.checked_add(note)?;
        for _ in 0..MAX_ALIAS_DEPTH {
            if !(PERCUSSION_PATCH_BASE..PATCH_COUNT).contains(&patch) {
                return None;
            }
            let value = usize::try_from(self.read_u32(patch.checked_mul(4)?)?).ok()?;
            if value >= PATCH_TABLE_SIZE {
                return (value < self.data.len()).then_some(value);
            }
            patch = PERCUSSION_PATCH_BASE.checked_add(value)?;
        }
        None
    }

    fn relative_offset(&self, base: usize, field_offset: usize) -> Option<usize> {
        let relative = usize::try_from(self.read_u32(field_offset)?).ok()?;
        let offset = base.checked_add(relative)?;
        (offset < self.data.len()).then_some(offset)
    }

    fn read_u16(&self, offset: usize) -> Option<u16> {
        Some(u16::from_le_bytes(
            self.data
                .get(offset..offset.checked_add(2)?)?
                .try_into()
                .ok()?,
        ))
    }

    fn read_u32(&self, offset: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            self.data
                .get(offset..offset.checked_add(4)?)?
                .try_into()
                .ok()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_embedded_string_zones() {
        let library = ToneLibrary::new(EMBEDDED_TONE_LIBRARY).unwrap();
        let low = library.melodic_sample(48, 60).unwrap();
        let high = library.melodic_sample(48, 79).unwrap();

        assert_eq!(low.sample_rate, 13_500);
        assert_eq!(low.root_key, 60);
        assert_eq!(low.loop_start, Some(6_278));
        assert_eq!(low.data.len(), 9_370);
        assert_eq!(high.sample_rate, 13_969);
        assert_eq!(high.root_key, 77);
        assert_eq!(high.loop_start, Some(8_026));
        assert_eq!(high.data.len(), 9_879);
    }

    #[test]
    fn resolves_melodic_aliases() {
        let library = ToneLibrary::new(EMBEDDED_TONE_LIBRARY).unwrap();
        let direct = library.melodic_sample(48, 60).unwrap();
        let alias = library.melodic_sample(40, 60).unwrap();

        assert_eq!(direct.sample_rate, alias.sample_rate);
        assert_eq!(direct.root_key, alias.root_key);
        assert_eq!(direct.data, alias.data);
    }

    #[test]
    fn loads_percussion_samples() {
        let library = ToneLibrary::new(EMBEDDED_TONE_LIBRARY).unwrap();
        let sample = library.percussion_sample(36).unwrap();

        assert_eq!(sample.sample_rate, 11_025);
        assert_eq!(sample.root_key, 36);
        assert_eq!(sample.loop_start, None);
        assert_eq!(sample.data.len(), 630);
    }

    #[test]
    fn rejects_truncated_library() {
        assert!(ToneLibrary::new(&[0; PATCH_TABLE_SIZE - 1]).is_none());
    }

    #[test]
    fn resolves_every_midi_patch() {
        let library = ToneLibrary::new(EMBEDDED_TONE_LIBRARY).unwrap();

        for program in 0..=127 {
            assert!(library.melodic_sample(program, 60).is_some());
        }
        for note in 0..=127 {
            assert!(library.percussion_sample(note).is_some());
        }
    }
}
