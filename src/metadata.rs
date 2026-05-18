//! `GridMetadata` parser, mirroring `nanovdb::io::MetaData` (util/IO.h:144).
//!
//! Memory layout (176 bytes + nameSize):
//! ```text
//! offset   bytes   field
//!   0       8      gridSize
//!   8       8      fileSize
//!  16       8      nameKey
//!  24       8      voxelCount
//!  32       4      gridType
//!  36       4      gridClass
//!  40      48      worldBBox (3 × f64 min, 3 × f64 max)
//!  88      24      indexBBox (3 × i32 min, 3 × i32 max)
//! 112      24      voxelSize (3 × f64)
//! 136       4      nameSize
//! 140      16      nodeCount[4] (u32 × 4)
//! 156      12      tileCount[3] (u32 × 3)
//! 168       2      codec
//! 170       2      padding
//! 172       4      version
//! 176     name     gridName (nameSize bytes, zero-terminated)
//! ```

use crate::error::Error;
use crate::header::{Codec, Version};
use crate::types::{GridClass, GridType, Vec3d};

pub const META_FIXED_BYTES: usize = 176;

#[derive(Debug, Clone)]
pub struct GridMetadata {
    pub grid_size: u64,
    pub file_size: u64,
    pub name_key: u64,
    pub voxel_count: u64,
    pub grid_type: GridType,
    pub grid_class: GridClass,
    pub world_bbox_min: Vec3d,
    pub world_bbox_max: Vec3d,
    pub index_bbox_min: [i32; 3],
    pub index_bbox_max: [i32; 3],
    pub voxel_size: Vec3d,
    pub node_count: [u32; 4],
    pub tile_count: [u32; 3],
    pub codec: Codec,
    pub version: Version,
    pub name: String,
}

impl GridMetadata {
    /// Parse `[bytes]` starting at offset 0; returns the parsed metadata
    /// and the number of bytes consumed (`META_FIXED_BYTES + nameSize`).
    pub fn parse(bytes: &[u8]) -> Result<(Self, usize), Error> {
        if bytes.len() < META_FIXED_BYTES {
            return Err(Error::Truncated {
                offset: 0,
                wanted: META_FIXED_BYTES,
                actual: bytes.len(),
            });
        }
        let grid_size = read_u64(bytes, 0);
        let file_size = read_u64(bytes, 8);
        let name_key = read_u64(bytes, 16);
        let voxel_count = read_u64(bytes, 24);
        let grid_type = GridType::from_raw(read_u32(bytes, 32));
        let grid_class = GridClass::from_raw(read_u32(bytes, 36));
        let world_bbox_min = Vec3d::new(
            read_f64(bytes, 40),
            read_f64(bytes, 48),
            read_f64(bytes, 56),
        );
        let world_bbox_max = Vec3d::new(
            read_f64(bytes, 64),
            read_f64(bytes, 72),
            read_f64(bytes, 80),
        );
        let index_bbox_min = [
            read_i32(bytes, 88),
            read_i32(bytes, 92),
            read_i32(bytes, 96),
        ];
        let index_bbox_max = [
            read_i32(bytes, 100),
            read_i32(bytes, 104),
            read_i32(bytes, 108),
        ];
        let voxel_size = Vec3d::new(
            read_f64(bytes, 112),
            read_f64(bytes, 120),
            read_f64(bytes, 128),
        );
        let name_size = read_u32(bytes, 136) as usize;
        let node_count = [
            read_u32(bytes, 140),
            read_u32(bytes, 144),
            read_u32(bytes, 148),
            read_u32(bytes, 152),
        ];
        let tile_count = [
            read_u32(bytes, 156),
            read_u32(bytes, 160),
            read_u32(bytes, 164),
        ];
        let codec = Codec::from_raw(read_u16(bytes, 168));
        // 170..172 = padding
        let version = Version(read_u32(bytes, 172));

        let name_end = META_FIXED_BYTES + name_size;
        if bytes.len() < name_end {
            return Err(Error::BadGridName {
                wanted: name_size,
                available: bytes.len() - META_FIXED_BYTES,
            });
        }
        // NanoVDB stores nameSize INCLUDING the null terminator. Strip it.
        let name_bytes = &bytes[META_FIXED_BYTES..name_end];
        let name = String::from_utf8_lossy(
            &name_bytes[..name_bytes.iter().position(|&b| b == 0).unwrap_or(name_bytes.len())],
        )
        .into_owned();

        Ok((
            GridMetadata {
                grid_size,
                file_size,
                name_key,
                voxel_count,
                grid_type,
                grid_class,
                world_bbox_min,
                world_bbox_max,
                index_bbox_min,
                index_bbox_max,
                voxel_size,
                node_count,
                tile_count,
                codec,
                version,
                name,
            },
            name_end,
        ))
    }
}

#[inline]
fn read_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
#[inline]
fn read_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
#[inline]
fn read_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
#[inline]
fn read_u64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
#[inline]
fn read_f64(b: &[u8], o: usize) -> f64 {
    f64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
