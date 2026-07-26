// Shared types and axis-map builders used by scaling algorithms.

pub mod bicubic;
pub mod bilinear;
pub mod nearest;
pub mod xbrz;

pub struct BiAxisMap {
    pub src: u32,
    pub frac: u16,
}

pub fn build_bi_axis_map(src_size: u32, dst_size: u32) -> Vec<BiAxisMap> {
    let mut map = Vec::with_capacity(dst_size as usize);
    for destination in 0..dst_size {
        let source = (destination as f64 + 0.5) * src_size as f64 / dst_size as f64 - 0.5;
        let source_index = source.floor().max(0.0) as u32;
        let fraction = ((source - source_index as f64) * 256.0)
            .round()
            .clamp(0.0, 255.0) as u16;
        map.push(BiAxisMap {
            src: source_index.min(src_size - 1),
            frac: fraction,
        });
    }
    map
}

pub const FRAC_BITS: i32 = 10;
pub const FRAC_UNIT: i32 = 1 << FRAC_BITS;

pub struct BcAxisMap {
    pub idx: [usize; 4],
    pub weight: [i32; 4],
}

pub fn build_bc_axis_map(src_size: u32, dst_size: u32) -> Vec<BcAxisMap> {
    let max_index = src_size as usize - 1;
    let mut map = Vec::with_capacity(dst_size as usize);
    for destination in 0..dst_size {
        let source = (destination as f64 + 0.5) * src_size as f64 / dst_size as f64 - 0.5;
        let center = source.floor() as i32;
        let fraction = (source - center as f64) as f32;
        let weight = catmull_rom_weights_fixed(fraction);
        let idx = [
            (center - 1).clamp(0, max_index as i32) as usize,
            center.clamp(0, max_index as i32) as usize,
            (center + 1).clamp(0, max_index as i32) as usize,
            (center + 2).clamp(0, max_index as i32) as usize,
        ];
        map.push(BcAxisMap { idx, weight });
    }
    map
}

pub fn catmull_rom_weights_fixed(t: f32) -> [i32; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        ((-0.5 * t3 + t2 - 0.5 * t) * FRAC_UNIT as f32) as i32,
        ((1.5 * t3 - 2.5 * t2 + 1.0) * FRAC_UNIT as f32) as i32,
        ((-1.5 * t3 + 2.0 * t2 + 0.5 * t) * FRAC_UNIT as f32) as i32,
        ((0.5 * t3 - 0.5 * t2) * FRAC_UNIT as f32) as i32,
    ]
}

#[inline]
pub fn clamp_i32_u8(value: i32) -> u32 {
    value.clamp(0, 255) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catmull_rom_weights_sum_to_one() {
        for step in 0..100 {
            let weights = catmull_rom_weights_fixed(step as f32 / 100.0);
            let sum: i32 = weights.iter().sum();
            assert!((sum - FRAC_UNIT).unsigned_abs() <= 2);
        }
    }

    #[test]
    fn bilinear_axis_map_handles_left_edge() {
        let map = build_bi_axis_map(2, 4);
        assert_eq!(map[0].src, 0);
        assert_eq!(map[0].frac, 0);
        assert_eq!(map[3].src, 1);
    }
}
