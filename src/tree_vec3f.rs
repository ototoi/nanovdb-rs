//! `nanovdb::Vec3f` support.
//!
//! Vec3f tree walking is intentionally not exposed until the
//! `RootData<Vec3f>` and `LeafData<Vec3f>` layouts are validated against
//! a real fixture.

/// Matches `nanovdb::Vec3f` at the byte level.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
pub struct Vec3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3f {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}
