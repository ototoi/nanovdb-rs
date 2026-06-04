use crate::tree_f32::ReadAccessor;

/// Trilinear voxel sampler for Float grids.
///
/// This mirrors NanoVDB's `SampleFromVoxels<TreeOrAccT, 1, false>`
/// concept for the Float `ReadAccessor` currently implemented by this
/// crate. Higher-order interpolation and cached stencils are intentionally
/// left for later, matching upstream concepts without copying the C++
/// template matrix into the public Rust API.
pub struct SampleFromVoxels<'a, 'b> {
    acc: &'a mut ReadAccessor<'b>,
}

impl<'a, 'b> SampleFromVoxels<'a, 'b> {
    pub fn new(acc: &'a mut ReadAccessor<'b>) -> Self {
        Self { acc }
    }

    pub fn sample(&mut self, mut xyz: [f64; 3]) -> f32 {
        let coord = floor_coord(&mut xyz);

        let lerp = |a: f32, b: f32, w: f64| a + (w as f32) * (b - a);
        self.sample_with(coord, xyz, lerp)
    }

    pub fn sample_f32(&mut self, mut xyz: [f32; 3]) -> f32 {
        let coord = floor_coord_f32(&mut xyz);

        let lerp = |a: f32, b: f32, w: f32| a + w * (b - a);
        self.sample_with(coord, xyz, lerp)
    }

    fn sample_with<T, F>(&mut self, coord: [i32; 3], xyz: [T; 3], lerp: F) -> f32
    where
        T: Copy,
        F: Fn(f32, f32, T) -> f32,
    {
        let mut coord = coord;

        let vz = self.acc.get_value(coord);
        coord[2] += 1;
        let vz1 = self.acc.get_value(coord);
        let vy = lerp(vz, vz1, xyz[2]);

        coord[1] += 1;
        let vz1 = self.acc.get_value(coord);
        coord[2] -= 1;
        let vz = self.acc.get_value(coord);
        let vy1 = lerp(vz, vz1, xyz[2]);
        let vx = lerp(vy, vy1, xyz[1]);

        coord[0] += 1;
        let vz = self.acc.get_value(coord);
        coord[2] += 1;
        let vz1 = self.acc.get_value(coord);
        let vy1 = lerp(vz, vz1, xyz[2]);

        coord[1] -= 1;
        let vz1 = self.acc.get_value(coord);
        coord[2] -= 1;
        let vz = self.acc.get_value(coord);
        let vy = lerp(vz, vz1, xyz[2]);
        let vx1 = lerp(vy, vy1, xyz[1]);

        lerp(vx, vx1, xyz[0])
    }
}

/// Equivalent to NanoVDB's `createSampler<1>(accessor)` for the supported
/// Float `ReadAccessor` case.
pub fn create_sampler1<'a, 'b>(acc: &'a mut ReadAccessor<'b>) -> SampleFromVoxels<'a, 'b> {
    SampleFromVoxels::new(acc)
}

fn floor_coord(xyz: &mut [f64; 3]) -> [i32; 3] {
    let ix = xyz[0].floor();
    let iy = xyz[1].floor();
    let iz = xyz[2].floor();
    xyz[0] -= ix;
    xyz[1] -= iy;
    xyz[2] -= iz;
    [ix as i32, iy as i32, iz as i32]
}

fn floor_coord_f32(xyz: &mut [f32; 3]) -> [i32; 3] {
    let ix = xyz[0].floor();
    let iy = xyz[1].floor();
    let iz = xyz[2].floor();
    xyz[0] -= ix;
    xyz[1] -= iy;
    xyz[2] -= iz;
    [ix as i32, iy as i32, iz as i32]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_coord_returns_integer_coord_and_fractional_xyz() {
        let mut xyz = [1.25, -2.75, 3.0];
        let coord = floor_coord(&mut xyz);
        assert_eq!(coord, [1, -3, 3]);
        assert_eq!(xyz, [0.25, 0.25, 0.0]);
    }

    #[test]
    fn floor_coord_f32_returns_integer_coord_and_fractional_xyz() {
        let mut xyz = [1.25, -2.75, 3.0];
        let coord = floor_coord_f32(&mut xyz);
        assert_eq!(coord, [1, -3, 3]);
        assert_eq!(xyz, [0.25, 0.25, 0.0]);
    }
}
