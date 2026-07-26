// xBRZ-style pixel-art scaler with reusable intermediate storage.

use super::bilinear::BilinearScaler;

const DISTANCE_THRESHOLD: u32 = 80;

pub struct XbrzScaler {
    intermediate: Vec<u32>,
    bilinear: BilinearScaler,
}

impl XbrzScaler {
    pub fn new() -> Self {
        Self {
            intermediate: Vec::new(),
            bilinear: BilinearScaler::new(),
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
        let factor = (dst_w / src_w).min(dst_h / src_h);
        if factor < 2 {
            self.bilinear.scale(src, src_w, src_h, dst_w, dst_h, dst);
            return;
        }

        let factor = factor.min(4);
        let intermediate_w = src_w * factor;
        let intermediate_h = src_h * factor;
        if intermediate_w == dst_w && intermediate_h == dst_h {
            scale_integer(src, src_w, src_h, factor, dst);
            return;
        }

        self.intermediate
            .resize((intermediate_w * intermediate_h) as usize, 0);
        scale_integer(src, src_w, src_h, factor, &mut self.intermediate);
        self.bilinear.scale(
            &self.intermediate,
            intermediate_w,
            intermediate_h,
            dst_w,
            dst_h,
            dst,
        );
    }
}

#[inline]
fn blend_pixel(base: u32, blend_to: u32, weight: u32) -> u32 {
    let inverse = 255 - weight;
    let channel = |shift: u32| {
        (((base >> shift) & 0xFF) * inverse + ((blend_to >> shift) & 0xFF) * weight) / 255
    };
    (channel(24) << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

#[inline]
fn color_distance(left: u32, right: u32) -> u32 {
    [16, 8, 0]
        .into_iter()
        .map(|shift| {
            let left = ((left >> shift) & 0xFF) as i32;
            let right = ((right >> shift) & 0xFF) as i32;
            (left - right).unsigned_abs()
        })
        .sum()
}

fn should_blend_corner(
    center: u32,
    diagonal: u32,
    other_diagonal: u32,
    opposite: u32,
    horizontal: u32,
    vertical: u32,
) -> bool {
    let similar_diagonal = color_distance(center, diagonal) <= DISTANCE_THRESHOLD;
    let similar_other = color_distance(center, other_diagonal) <= DISTANCE_THRESHOLD;
    let similar_opposite = color_distance(center, opposite) <= DISTANCE_THRESHOLD;
    let similar_horizontal = color_distance(center, horizontal) <= DISTANCE_THRESHOLD;
    let similar_vertical = color_distance(center, vertical) <= DISTANCE_THRESHOLD;

    similar_opposite
        && ((similar_diagonal && !similar_horizontal && !similar_vertical)
            || (similar_diagonal
                && similar_other
                && color_distance(diagonal, other_diagonal) > DISTANCE_THRESHOLD))
}

fn scale_integer(src: &[u32], src_w: u32, src_h: u32, factor: u32, dst: &mut [u32]) {
    let src_w = src_w as usize;
    let src_h = src_h as usize;
    let factor = factor as usize;
    let dst_w = src_w * factor;
    let get = |x: isize, y: isize| -> u32 {
        let x = x.clamp(0, src_w as isize - 1) as usize;
        let y = y.clamp(0, src_h as isize - 1) as usize;
        src[y * src_w + x]
    };

    for y in 0..src_h {
        for x in 0..src_w {
            let x = x as isize;
            let y = y as isize;
            let center = get(x, y);
            let north_west = get(x - 1, y - 1);
            let north = get(x, y - 1);
            let north_east = get(x + 1, y - 1);
            let west = get(x - 1, y);
            let east = get(x + 1, y);
            let south_west = get(x - 1, y + 1);
            let south = get(x, y + 1);
            let south_east = get(x + 1, y + 1);

            let blends = [
                should_blend_corner(center, north_west, north_east, south_east, north, west),
                should_blend_corner(center, north_east, north_west, south_west, north, east),
                should_blend_corner(center, south_west, south_east, north_east, south, west),
                should_blend_corner(center, south_east, south_west, north_west, south, east),
            ];
            let targets = [south_east, south_west, north_east, north_west];
            let base_x = x as usize * factor;
            let base_y = y as usize * factor;

            for offset_y in 0..factor {
                let start = (base_y + offset_y) * dst_w + base_x;
                dst[start..start + factor].fill(center);
            }

            let corners = [
                base_y * dst_w + base_x,
                base_y * dst_w + base_x + factor - 1,
                (base_y + factor - 1) * dst_w + base_x,
                (base_y + factor - 1) * dst_w + base_x + factor - 1,
            ];
            for index in 0..4 {
                if blends[index] {
                    dst[corners[index]] = blend_pixel(center, targets[index], 128);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xbrz_preserves_uniform_image_at_four_times_scale() {
        let src = [0x00AABBCC; 4];
        let mut dst = [0; 64];
        XbrzScaler::new().scale(&src, 2, 2, 8, 8, &mut dst);
        assert_eq!(dst, [0x00AABBCC; 64]);
    }

    #[test]
    fn xbrz_handles_non_integer_destination() {
        let src = [0x00112233; 4];
        let mut dst = [0; 35];
        XbrzScaler::new().scale(&src, 2, 2, 7, 5, &mut dst);
        assert_eq!(dst, [0x00112233; 35]);
    }
}
