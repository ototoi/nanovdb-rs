//! `nanovdb::Vec3f` support.
//!
//! Vec3f tree walking for NanoVDB v32.

use crate::grid_data::{GridDataHeader, GRID_DATA_SIZE};
use crate::tree_f32::TreeData;
use crate::types::GridType;
use std::collections::HashSet;

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

/// Random-access reader for a `Vec3f` NanoVDB tree.
pub struct Vec3fReadAccessor<'a> {
    bytes: &'a [u8],
    background: Vec3f,
    root_abs: usize,
    root_table_size: u32,
}

impl<'a> Vec3fReadAccessor<'a> {
    pub fn from_grid_bytes(bytes: &'a [u8]) -> Option<Self> {
        let header = GridDataHeader::parse(bytes)?;
        if GridType::from_raw(header.grid_type) != GridType::Vec3f {
            return None;
        }
        let tree = TreeData::parse(bytes.get(GRID_DATA_SIZE..GRID_DATA_SIZE + 64)?);
        let root_abs = GRID_DATA_SIZE.checked_add(tree.root_offset() as usize)?;
        let root_table_size = read_u32(bytes, root_abs.checked_add(24)?)?;
        let background = read_vec3f(bytes, root_abs.checked_add(28)?)?;
        checked_range(
            bytes,
            root_abs,
            96usize.checked_add(root_table_size as usize * 32)?,
        )?;
        let mut validated_upper = HashSet::new();
        let mut validated_lower = HashSet::new();
        for index in 0..root_table_size as usize {
            let tile = root_abs + 96 + index * 32;
            let child = read_i64(bytes, tile + 8)?;
            if child != 0 {
                let upper = child_target(root_abs, child)?;
                if validated_upper.insert(upper) {
                    validate_upper(bytes, upper, &mut validated_lower)?;
                }
            }
        }
        Some(Self {
            bytes,
            background,
            root_abs,
            root_table_size,
        })
    }

    pub fn background(&self) -> Vec3f {
        self.background
    }

    pub fn get_value(&self, ijk: [i32; 3]) -> Vec3f {
        let key = coord_to_root_key(ijk);
        let tiles = self.root_abs + 96;
        for index in 0..self.root_table_size as usize {
            let tile = tiles + index * 32;
            if read_u64(self.bytes, tile) != Some(key) {
                continue;
            }
            let child = read_i64(self.bytes, tile + 8).expect("validated root tile");
            if child == 0 {
                return read_vec3f(self.bytes, tile + 20).expect("validated Vec3f tile value");
            }
            return self.node_value(
                child_target(self.root_abs, child).expect("validated child"),
                ijk,
            );
        }
        self.background
    }

    fn node_value(&self, upper: usize, ijk: [i32; 3]) -> Vec3f {
        let upper_off = upper_offset(ijk) as usize;
        let upper_mask = upper + 32 + 4096;
        let upper_entry = upper + 8256 + upper_off * 16;
        if mask_is_on(self.bytes, upper_mask, upper_off as u32) {
            let child = read_i64(self.bytes, upper_entry).unwrap_or(0);
            if child == 0 {
                return self.background;
            }
            return self.node_lower(child_target(upper, child).expect("validated child"), ijk);
        }
        read_vec3f(self.bytes, upper_entry).expect("validated Vec3f upper value")
    }

    fn node_lower(&self, lower: usize, ijk: [i32; 3]) -> Vec3f {
        let lower_off = lower_offset(ijk) as usize;
        let lower_mask = lower + 32 + 64;
        let lower_entry = lower + 1088 + lower_off * 16;
        if mask_is_on(self.bytes, lower_mask, lower_off as u32) {
            let child = read_i64(self.bytes, lower_entry).expect("validated child entry");
            if child == 0 {
                return self.background;
            }
            let leaf = child_target(lower, child).expect("validated child");
            return read_vec3f(self.bytes, leaf + 128 + leaf_offset(ijk) as usize * 12)
                .expect("validated Vec3f leaf value");
        }
        read_vec3f(self.bytes, lower_entry).expect("validated Vec3f lower value")
    }
}

fn coord_to_root_key(ijk: [i32; 3]) -> u64 {
    let x = (ijk[0] as u32 >> 12) as u64;
    let y = (ijk[1] as u32 >> 12) as u64;
    let z = (ijk[2] as u32 >> 12) as u64;
    z | (y << 21) | (x << 42)
}

fn upper_offset(ijk: [i32; 3]) -> u32 {
    let x = ((ijk[0] >> 7) & 31) as u32;
    let y = ((ijk[1] >> 7) & 31) as u32;
    let z = ((ijk[2] >> 7) & 31) as u32;
    (x << 10) | (y << 5) | z
}

fn lower_offset(ijk: [i32; 3]) -> u32 {
    let x = ((ijk[0] >> 3) & 15) as u32;
    let y = ((ijk[1] >> 3) & 15) as u32;
    let z = ((ijk[2] >> 3) & 15) as u32;
    (x << 8) | (y << 4) | z
}

fn leaf_offset(ijk: [i32; 3]) -> u32 {
    let x = (ijk[0] & 7) as u32;
    let y = (ijk[1] & 7) as u32;
    let z = (ijk[2] & 7) as u32;
    (x << 6) | (y << 3) | z
}

fn mask_is_on(bytes: &[u8], mask: usize, offset: u32) -> bool {
    let word = mask + (offset as usize / 64) * 8;
    read_u64(bytes, word)
        .map(|value| value & (1u64 << (offset % 64)) != 0)
        .unwrap_or(false)
}

fn checked_range(bytes: &[u8], offset: usize, len: usize) -> Option<()> {
    offset
        .checked_add(len)
        .filter(|end| *end <= bytes.len())
        .map(|_| ())
}

fn child_target(base: usize, child: i64) -> Option<usize> {
    let target = (base as i64).checked_add(child)?;
    if target < 0 {
        return None;
    }
    let target = target as usize;
    (target % 32 == 0).then_some(target)
}

fn validate_upper(bytes: &[u8], upper: usize, validated_lower: &mut HashSet<usize>) -> Option<()> {
    const UPPER_SIZE: usize = 8256 + 32768 * 16;
    checked_range(bytes, upper, UPPER_SIZE)?;
    let mask = upper + 32 + 4096;
    let table = upper + 8256;
    for offset in 0..32768u32 {
        if mask_is_on(bytes, mask, offset) {
            let child = read_i64(bytes, table + offset as usize * 16)?;
            if child == 0 {
                return None;
            }
            let lower = child_target(upper, child)?;
            if validated_lower.insert(lower) {
                validate_lower(bytes, lower)?;
            }
        }
    }
    Some(())
}

fn validate_lower(bytes: &[u8], lower: usize) -> Option<()> {
    const LOWER_SIZE: usize = 1088 + 4096 * 16;
    const LEAF_SIZE: usize = 128 + 512 * 12;
    checked_range(bytes, lower, LOWER_SIZE)?;
    let mask = lower + 32 + 64;
    let table = lower + 1088;
    for offset in 0..4096u32 {
        if mask_is_on(bytes, mask, offset) {
            let child = read_i64(bytes, table + offset as usize * 16)?;
            if child == 0 {
                return None;
            }
            let leaf = child_target(lower, child)?;
            checked_range(bytes, leaf, LEAF_SIZE)?;
        }
    }
    Some(())
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
}

fn read_i64(bytes: &[u8], offset: usize) -> Option<i64> {
    Some(i64::from_le_bytes(
        bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
}

fn read_vec3f(bytes: &[u8], offset: usize) -> Option<Vec3f> {
    Some(Vec3f::new(
        f32::from_le_bytes(bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?),
        f32::from_le_bytes(
            bytes
                .get(offset.checked_add(4)?..offset.checked_add(8)?)?
                .try_into()
                .ok()?,
        ),
        f32::from_le_bytes(
            bytes
                .get(offset.checked_add(8)?..offset.checked_add(12)?)?
                .try_into()
                .ok()?,
        ),
    ))
}
