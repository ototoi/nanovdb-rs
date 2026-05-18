//! Enum mirrors of NanoVDB's `GridType` and `GridClass` plus the
//! double-precision `Vec3` r4 needs for AABB/voxel-size metadata.

/// Mirror of `nanovdb::GridType` (NanoVDB.h around line 425). Only the
/// values we currently surface are spelled out; everything else maps to
/// `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GridType {
    Unknown = 0,
    Float = 1,
    Double = 2,
    Int16 = 3,
    Int32 = 4,
    Int64 = 5,
    Vec3f = 6,
    Vec3d = 7,
    Mask = 8,
    Half = 9,
    Uint32 = 10,
    Boolean = 11,
    RgbA8 = 12,
    Fp4 = 13,
    Fp8 = 14,
    Fp16 = 15,
    FpN = 16,
    Vec4f = 17,
    Vec4d = 18,
    Index = 19,
    OnIndex = 20,
    IndexMask = 21,
    OnIndexMask = 22,
    PointIndex = 23,
    Vec3u8 = 24,
    Vec3u16 = 25,
    UInt8 = 26,
}

impl GridType {
    pub fn from_raw(v: u32) -> Self {
        match v {
            1 => Self::Float,
            2 => Self::Double,
            3 => Self::Int16,
            4 => Self::Int32,
            5 => Self::Int64,
            6 => Self::Vec3f,
            7 => Self::Vec3d,
            8 => Self::Mask,
            9 => Self::Half,
            10 => Self::Uint32,
            11 => Self::Boolean,
            12 => Self::RgbA8,
            13 => Self::Fp4,
            14 => Self::Fp8,
            15 => Self::Fp16,
            16 => Self::FpN,
            17 => Self::Vec4f,
            18 => Self::Vec4d,
            19 => Self::Index,
            20 => Self::OnIndex,
            21 => Self::IndexMask,
            22 => Self::OnIndexMask,
            23 => Self::PointIndex,
            24 => Self::Vec3u8,
            25 => Self::Vec3u16,
            26 => Self::UInt8,
            _ => Self::Unknown,
        }
    }
}

/// Mirror of `nanovdb::GridClass`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GridClass {
    Unknown = 0,
    LevelSet = 1,
    FogVolume = 2,
    Staggered = 3,
    PointIndex = 4,
    PointData = 5,
    Topology = 6,
    VoxelVolume = 7,
    IndexGrid = 8,
    TensorGrid = 9,
}

impl GridClass {
    pub fn from_raw(v: u32) -> Self {
        match v {
            1 => Self::LevelSet,
            2 => Self::FogVolume,
            3 => Self::Staggered,
            4 => Self::PointIndex,
            5 => Self::PointData,
            6 => Self::Topology,
            7 => Self::VoxelVolume,
            8 => Self::IndexGrid,
            9 => Self::TensorGrid,
            _ => Self::Unknown,
        }
    }
}

/// Double-precision 3-vector, matching `nanovdb::Vec3d` in IO metadata.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Vec3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3d {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}
