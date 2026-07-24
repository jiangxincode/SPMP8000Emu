// Nearest-neighbour scaler.

pub struct NearestScaler {
    x_map: Vec<usize>,
    y_map: Vec<usize>,
    dimensions: (u32, u32, u32, u32),
}

impl NearestScaler {
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
            self.x_map = (0..dst_w)
                .map(|x| (u64::from(x) * u64::from(src_w) / u64::from(dst_w)) as usize)
                .collect();
            self.y_map = (0..dst_h)
                .map(|y| (u64::from(y) * u64::from(src_h) / u64::from(dst_h)) as usize)
                .collect();
            self.dimensions = (src_w, src_h, dst_w, dst_h);
        }

        let src_w = src_w as usize;
        let dst_w = dst_w as usize;
        for (dst_y, &src_y) in self.y_map.iter().enumerate() {
            let src_row = &src[src_y * src_w..(src_y + 1) * src_w];
            let dst_row = &mut dst[dst_y * dst_w..(dst_y + 1) * dst_w];
            for (pixel, &src_x) in dst_row.iter_mut().zip(&self.x_map) {
                *pixel = src_row[src_x];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_two_times_replicates_pixels() {
        let src = [1, 2, 3, 4];
        let mut dst = [0; 16];
        NearestScaler::new().scale(&src, 2, 2, 4, 4, &mut dst);
        assert_eq!(dst, [1, 1, 2, 2, 1, 1, 2, 2, 3, 3, 4, 4, 3, 3, 4, 4]);
    }
}
