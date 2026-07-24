// Bicubic scaler using separable Catmull-Rom interpolation.

use super::{build_bc_axis_map, clamp_i32_u8, BcAxisMap, FRAC_BITS};

pub struct BicubicScaler {
    x_map: Vec<BcAxisMap>,
    y_map: Vec<BcAxisMap>,
    intermediate: Vec<u32>,
    dimensions: (u32, u32, u32, u32),
}

impl BicubicScaler {
    pub fn new() -> Self {
        Self {
            x_map: Vec::new(),
            y_map: Vec::new(),
            intermediate: Vec::new(),
            dimensions: (0, 0, 0, 0),
        }
    }

    pub fn scale(
        &mut self,
        src: &[u32],
        src_w: u32,
        src_h: u32,
        dst_w: u32,
        dst_h: u32,
        dst: &mut [u32],
    ) {
        if self.dimensions != (src_w, src_h, dst_w, dst_h) {
            self.x_map = build_bc_axis_map(src_w, dst_w);
            self.y_map = build_bc_axis_map(src_h, dst_h);
            self.intermediate.resize((dst_w * src_h) as usize, 0);
            self.dimensions = (src_w, src_h, dst_w, dst_h);
        }

        let src_w = src_w as usize;
        let dst_w = dst_w as usize;

        for src_y in 0..src_h as usize {
            let src_row = &src[src_y * src_w..(src_y + 1) * src_w];
            let intermediate_row = &mut self.intermediate[src_y * dst_w..(src_y + 1) * dst_w];
            for (dst_x, pixel) in intermediate_row.iter_mut().enumerate() {
                let map = &self.x_map[dst_x];
                let samples = [
                    src_row[map.idx[0]],
                    src_row[map.idx[1]],
                    src_row[map.idx[2]],
                    src_row[map.idx[3]],
                ];
                *pixel = blend_channels(&samples, &map.weight);
            }
        }

        for (dst_y, map) in self.y_map.iter().enumerate() {
            let rows = [
                &self.intermediate[map.idx[0] * dst_w..(map.idx[0] + 1) * dst_w],
                &self.intermediate[map.idx[1] * dst_w..(map.idx[1] + 1) * dst_w],
                &self.intermediate[map.idx[2] * dst_w..(map.idx[2] + 1) * dst_w],
                &self.intermediate[map.idx[3] * dst_w..(map.idx[3] + 1) * dst_w],
            ];
            let dst_row = &mut dst[dst_y * dst_w..(dst_y + 1) * dst_w];
            for (dst_x, pixel) in dst_row.iter_mut().enumerate() {
                let samples = [
                    rows[0][dst_x],
                    rows[1][dst_x],
                    rows[2][dst_x],
                    rows[3][dst_x],
                ];
                *pixel = blend_channels(&samples, &map.weight);
            }
        }
    }
}

fn blend_channels(pixels: &[u32; 4], weights: &[i32; 4]) -> u32 {
    let channel = |shift: u32| {
        let value = pixels
            .iter()
            .zip(weights)
            .map(|(&pixel, &weight)| ((pixel >> shift) & 0xFF) as i32 * weight)
            .sum::<i32>();
        clamp_i32_u8(value >> FRAC_BITS)
    };
    (channel(24) << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bicubic_preserves_uniform_image_at_non_integer_scale() {
        let src = [0x00ABCDEF; 4];
        let mut dst = [0; 15];
        BicubicScaler::new().scale(&src, 2, 2, 5, 3, &mut dst);
        for pixel in dst {
            for shift in [0, 8, 16] {
                let actual = (pixel >> shift) & 0xFF;
                let expected = (0x00ABCDEF >> shift) & 0xFF;
                assert!(actual.abs_diff(expected) <= 1);
            }
        }
    }
}
