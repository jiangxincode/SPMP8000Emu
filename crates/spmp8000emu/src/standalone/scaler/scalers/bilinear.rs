// Bilinear scaler using fixed-point interpolation weights.

use super::{build_bi_axis_map, BiAxisMap};

pub struct BilinearScaler {
    x_map: Vec<BiAxisMap>,
    y_map: Vec<BiAxisMap>,
    dimensions: (u32, u32, u32, u32),
}

impl BilinearScaler {
    pub fn new() -> Self {
        Self {
            x_map: Vec::new(),
            y_map: Vec::new(),
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
            self.x_map = build_bi_axis_map(src_w, dst_w);
            self.y_map = build_bi_axis_map(src_h, dst_h);
            self.dimensions = (src_w, src_h, dst_w, dst_h);
        }

        let src_w = src_w as usize;
        let src_h = src_h as usize;
        let dst_w = dst_w as usize;

        for (dst_y, y_map) in self.y_map.iter().enumerate() {
            let src_y0 = y_map.src as usize;
            let src_y1 = (src_y0 + 1).min(src_h - 1);
            let fraction_y = u32::from(y_map.frac);
            let inverse_y = 256 - fraction_y;
            let row0 = &src[src_y0 * src_w..(src_y0 + 1) * src_w];
            let row1 = &src[src_y1 * src_w..(src_y1 + 1) * src_w];
            let dst_row = &mut dst[dst_y * dst_w..(dst_y + 1) * dst_w];

            for (dst_x, pixel) in dst_row.iter_mut().enumerate() {
                let x_map = &self.x_map[dst_x];
                let src_x0 = x_map.src as usize;
                let src_x1 = (src_x0 + 1).min(src_w - 1);
                let fraction_x = u32::from(x_map.frac);
                let inverse_x = 256 - fraction_x;
                let weights = [
                    inverse_x * inverse_y,
                    fraction_x * inverse_y,
                    inverse_x * fraction_y,
                    fraction_x * fraction_y,
                ];
                let pixels = [row0[src_x0], row0[src_x1], row1[src_x0], row1[src_x1]];
                *pixel = blend_channels(&pixels, &weights);
            }
        }
    }
}

fn blend_channels(pixels: &[u32; 4], weights: &[u32; 4]) -> u32 {
    let channel = |shift: u32| {
        pixels
            .iter()
            .zip(weights)
            .map(|(&pixel, &weight)| ((pixel >> shift) & 0xFF) * weight)
            .sum::<u32>()
            >> 16
    };
    (channel(24) << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilinear_preserves_uniform_image_at_non_integer_scale() {
        let src = [0x00ABCDEF; 4];
        let mut dst = [0; 15];
        BilinearScaler::new().scale(&src, 2, 2, 5, 3, &mut dst);
        assert_eq!(dst, [0x00ABCDEF; 15]);
    }
}
