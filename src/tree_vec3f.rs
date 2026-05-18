//! Vec3f-valued NanoVDB tree walker (`NanoGrid<Vec3f>`).
//!
//! Stub: the type-discrimination, byte-offset constants, and accessor
//! API are sketched out so callers can branch on `grid.vec3f_accessor()`
//! today, but the actual tree walk is **not implemented** yet. Calling
//! `value_at_index` / `sample_trilinear` returns `Vec3f::ZERO`.
//!
//! The exact `RootData<Vec3f>` and `LeafData<Vec3f>` byte layouts in
//! NanoVDB depend on `alignas(NANOVDB_DATA_ALIGNMENT)` placement and the
//! `union { Vec3f; int64_t; }` table-entry sizing rules; getting those
//! right needs a real `.nvdb` Vec3f fixture to validate against. None of
//! the scenes we ship with (`bunny_cloud`, `fire`, `wdas_cloud_quarter`)
//! contain a Vec3f grid, so this is left as a follow-up.

use crate::grid_data::GridDataHeader;
use crate::types::GridType;

/// A 3-component f32 vector returned by `Vec3fAccessor` lookups. Matches
/// `nanovdb::Vec3f` at the byte level.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
pub struct Vec3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3f {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// Random-access accessor over a `Vec3f` tree.
///
/// Stub: the constructor verifies the grid type but the lookup methods
/// are placeholders. Filed for follow-up once a Vec3f fixture is
/// available. Callers can still match on `Grid::value_type()` to gate
/// the call site cleanly.
pub struct Vec3fAccessor<'a> {
    #[allow(dead_code)]
    grid_bytes: &'a [u8],
}

impl<'a> Vec3fAccessor<'a> {
    pub fn from_grid_bytes(bytes: &'a [u8]) -> Option<Self> {
        let header = GridDataHeader::parse(bytes)?;
        if GridType::from_raw(header.grid_type) != GridType::Vec3f {
            return None;
        }
        Some(Self { grid_bytes: bytes })
    }

    /// Placeholder: returns `Vec3f::ZERO` until the tree walk is
    /// implemented. The signature mirrors `FloatAccessor::value_at_index`
    /// so consumers can prepare call sites now.
    pub fn value_at_index(&self, _ijk: [i32; 3]) -> Vec3f {
        Vec3f::ZERO
    }

    /// Placeholder: returns `Vec3f::ZERO`. See `value_at_index`.
    pub fn sample_trilinear(&self, _idx: [f64; 3]) -> Vec3f {
        Vec3f::ZERO
    }
}
