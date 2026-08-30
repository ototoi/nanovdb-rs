//! `nanovdb::Vec3f` support.
//!
//! Vec3f tree walking for NanoVDB v32.

use crate::grid_data::{GridDataHeader, GRID_DATA_SIZE};
use crate::tree_data::TreeData;
use crate::types::{GridType, Vec3f};
use std::collections::HashSet;

const ROOT_HEADER_SIZE: usize = 96;
const TREE_DATA_SIZE: usize = 64;
const ROOT_TILE_SIZE: usize = 32;
const ROOT_TABLE_SIZE_OFFSET: usize = 24;
const ROOT_BACKGROUND_OFFSET: usize = 28;
const ROOT_TILE_CHILD_OFFSET: usize = 8;
const ROOT_TILE_STATE_OFFSET: usize = 16;
const ROOT_TILE_VALUE_OFFSET: usize = 20;

const NODE_BBOX_FLAGS_SIZE: usize = 32;
const UPPER_VALUE_MASK_SIZE: usize = 4096;
const LOWER_VALUE_MASK_SIZE: usize = 512;
const UPPER_VALUE_MASK_OFFSET: usize = NODE_BBOX_FLAGS_SIZE;
const UPPER_CHILD_MASK_OFFSET: usize = UPPER_VALUE_MASK_OFFSET + UPPER_VALUE_MASK_SIZE;
const LOWER_VALUE_MASK_OFFSET: usize = NODE_BBOX_FLAGS_SIZE;
const LOWER_CHILD_MASK_OFFSET: usize = LOWER_VALUE_MASK_OFFSET + LOWER_VALUE_MASK_SIZE;
const UPPER_TABLE_OFFSET: usize = 8256;
const LOWER_TABLE_OFFSET: usize = 1088;
const NODE_TABLE_STRIDE: usize = 16;
const LEAF_VALUE_MASK_OFFSET: usize = 16;
const LEAF_VALUES_OFFSET: usize = 128;
const VALUE_STRIDE: usize = 12;
const VALUE_COMPONENT_SIZE: usize = 4;
const NODE_ALIGNMENT: usize = 32;
const MASK_WORD_BITS: u32 = 64;
const MASK_WORD_SIZE: usize = 8;

const ROOT_LEVEL_BITS: u32 = 12;
const ROOT_KEY_AXIS_BITS: u32 = 21;
const UPPER_AXIS_BITS: i32 = 5;
const LOWER_AXIS_BITS: i32 = 4;
const LEAF_AXIS_BITS: i32 = 3;
const UPPER_AXIS_MASK: i32 = 31;
const LOWER_AXIS_MASK: i32 = 15;
const LEAF_AXIS_MASK: i32 = 7;
const UPPER_TABLE_SIZE: u32 = 32768;
const LOWER_TABLE_SIZE: u32 = 4096;
const LEAF_TABLE_SIZE: usize = 512;
const UPPER_SIZE: usize = UPPER_TABLE_OFFSET + UPPER_TABLE_SIZE as usize * NODE_TABLE_STRIDE;
const LOWER_SIZE: usize = LOWER_TABLE_OFFSET + LOWER_TABLE_SIZE as usize * NODE_TABLE_STRIDE;
const LEAF_SIZE: usize = LEAF_VALUES_OFFSET + LEAF_TABLE_SIZE * VALUE_STRIDE;

/// Random-access reader for a `Vec3f` NanoVDB tree.
pub struct Vec3fValidatedTree<'a> {
    bytes: &'a [u8],
    background: Vec3f,
    root_abs: usize,
    root_table_size: u32,
}

impl<'a> Vec3fValidatedTree<'a> {
    pub fn new(bytes: &'a [u8]) -> Option<Self> {
        let header = GridDataHeader::parse(bytes)?;
        if GridType::from_raw(header.grid_type) != GridType::Vec3f {
            return None;
        }
        let tree = TreeData::parse(bytes.get(GRID_DATA_SIZE..GRID_DATA_SIZE + TREE_DATA_SIZE)?);
        let root_abs = GRID_DATA_SIZE.checked_add(tree.root_offset() as usize)?;
        let root_table_size = read_u32(bytes, root_abs.checked_add(ROOT_TABLE_SIZE_OFFSET)?)?;
        let background = read_vec3f(bytes, root_abs.checked_add(ROOT_BACKGROUND_OFFSET)?)?;
        checked_range(
            bytes,
            root_abs,
            ROOT_HEADER_SIZE.checked_add(root_table_size as usize * ROOT_TILE_SIZE)?,
        )?;
        let mut validated_upper = HashSet::new();
        let mut validated_lower = HashSet::new();
        for index in 0..root_table_size as usize {
            let tile = root_abs + ROOT_HEADER_SIZE + index * ROOT_TILE_SIZE;
            let child = read_i64(bytes, tile + ROOT_TILE_CHILD_OFFSET)?;
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
        self.value_from_root(ijk, self.root_entry(ijk))
    }

    pub fn is_active(&self, ijk: [i32; 3]) -> bool {
        let Some(tile) = self.root_entry(ijk) else {
            return false;
        };
        let child =
            read_i64(self.bytes, tile + ROOT_TILE_CHILD_OFFSET).expect("validated root tile");
        if child == 0 {
            return read_u32(self.bytes, tile + ROOT_TILE_STATE_OFFSET)
                .expect("validated root state")
                != 0;
        }
        self.node_is_active(
            child_target(self.root_abs, child).expect("validated child"),
            ijk,
        )
    }

    fn root_entry(&self, ijk: [i32; 3]) -> Option<usize> {
        let key = coord_to_root_key(ijk);
        let tiles = self.root_abs + ROOT_HEADER_SIZE;
        for index in 0..self.root_table_size as usize {
            let tile = tiles + index * ROOT_TILE_SIZE;
            if read_u64(self.bytes, tile) != Some(key) {
                continue;
            }
            return Some(tile);
        }
        None
    }

    fn value_from_root(&self, ijk: [i32; 3], tile: Option<usize>) -> Vec3f {
        if let Some(tile) = tile {
            let child =
                read_i64(self.bytes, tile + ROOT_TILE_CHILD_OFFSET).expect("validated root tile");
            if child == 0 {
                return read_vec3f(self.bytes, tile + ROOT_TILE_VALUE_OFFSET)
                    .expect("validated Vec3f tile value");
            }
            return self.node_value(
                child_target(self.root_abs, child).expect("validated child"),
                ijk,
            );
        }
        self.background
    }

    fn node_is_active(&self, upper: usize, ijk: [i32; 3]) -> bool {
        let upper_off = upper_offset(ijk) as usize;
        let upper_value_mask = upper + UPPER_VALUE_MASK_OFFSET;
        let upper_child_mask = upper + UPPER_CHILD_MASK_OFFSET;
        if mask_is_on(self.bytes, upper_child_mask, upper_off as u32) {
            let entry = upper + UPPER_TABLE_OFFSET + upper_off * NODE_TABLE_STRIDE;
            let child = read_i64(self.bytes, entry).expect("validated child entry");
            return self
                .node_lower_is_active(child_target(upper, child).expect("validated child"), ijk);
        }
        mask_is_on(self.bytes, upper_value_mask, upper_off as u32)
    }

    fn node_lower_is_active(&self, lower: usize, ijk: [i32; 3]) -> bool {
        let lower_off = lower_offset(ijk) as usize;
        let lower_value_mask = lower + LOWER_VALUE_MASK_OFFSET;
        let lower_child_mask = lower + LOWER_CHILD_MASK_OFFSET;
        if mask_is_on(self.bytes, lower_child_mask, lower_off as u32) {
            let entry = lower + LOWER_TABLE_OFFSET + lower_off * NODE_TABLE_STRIDE;
            let child = read_i64(self.bytes, entry).expect("validated child entry");
            let leaf = child_target(lower, child).expect("validated child");
            return mask_is_on(self.bytes, leaf + LEAF_VALUE_MASK_OFFSET, leaf_offset(ijk));
        }
        mask_is_on(self.bytes, lower_value_mask, lower_off as u32)
    }

    fn node_value(&self, upper: usize, ijk: [i32; 3]) -> Vec3f {
        let upper_off = upper_offset(ijk) as usize;
        let upper_mask = upper + UPPER_CHILD_MASK_OFFSET;
        let upper_entry = upper + UPPER_TABLE_OFFSET + upper_off * NODE_TABLE_STRIDE;
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
        let lower_mask = lower + LOWER_CHILD_MASK_OFFSET;
        let lower_entry = lower + LOWER_TABLE_OFFSET + lower_off * NODE_TABLE_STRIDE;
        if mask_is_on(self.bytes, lower_mask, lower_off as u32) {
            let child = read_i64(self.bytes, lower_entry).expect("validated child entry");
            if child == 0 {
                return self.background;
            }
            let leaf = child_target(lower, child).expect("validated child");
            return read_vec3f(
                self.bytes,
                leaf + LEAF_VALUES_OFFSET + leaf_offset(ijk) as usize * VALUE_STRIDE,
            )
            .expect("validated Vec3f leaf value");
        }
        read_vec3f(self.bytes, lower_entry).expect("validated Vec3f lower value")
    }
}

/// Random-access accessor over a validated `Vec3f` NanoVDB tree.
pub struct Vec3fReadAccessor<'a> {
    tree: Vec3fValidatedTree<'a>,
    key: u64,
    root_tile: Option<usize>,
}

impl<'a> Vec3fReadAccessor<'a> {
    pub fn from_grid_bytes(bytes: &'a [u8]) -> Option<Self> {
        Some(Self {
            tree: Vec3fValidatedTree::new(bytes)?,
            key: u64::MAX,
            root_tile: None,
        })
    }

    fn root_entry(&mut self, ijk: [i32; 3]) -> Option<usize> {
        let key = coord_to_root_key(ijk);
        if self.key != key {
            self.key = key;
            self.root_tile = self.tree.root_entry(ijk);
        }
        self.root_tile
    }

    pub fn background(&self) -> Vec3f {
        self.tree.background()
    }

    pub fn get_value(&mut self, ijk: [i32; 3]) -> Vec3f {
        let tile = self.root_entry(ijk);
        self.tree.value_from_root(ijk, tile)
    }

    pub fn is_active(&mut self, ijk: [i32; 3]) -> bool {
        let Some(tile) = self.root_entry(ijk) else {
            return false;
        };
        let child =
            read_i64(self.tree.bytes, tile + ROOT_TILE_CHILD_OFFSET).expect("validated root tile");
        if child == 0 {
            return read_u32(self.tree.bytes, tile + ROOT_TILE_STATE_OFFSET)
                .expect("validated root state")
                != 0;
        }
        self.tree.node_is_active(
            child_target(self.tree.root_abs, child).expect("validated child"),
            ijk,
        )
    }
}

fn coord_to_root_key(ijk: [i32; 3]) -> u64 {
    let x = (ijk[0] as u32 >> ROOT_LEVEL_BITS) as u64;
    let y = (ijk[1] as u32 >> ROOT_LEVEL_BITS) as u64;
    let z = (ijk[2] as u32 >> ROOT_LEVEL_BITS) as u64;
    z | (y << ROOT_KEY_AXIS_BITS) | (x << (ROOT_KEY_AXIS_BITS * 2))
}

fn upper_offset(ijk: [i32; 3]) -> u32 {
    let x = ((ijk[0] >> (LOWER_AXIS_BITS + LEAF_AXIS_BITS)) & UPPER_AXIS_MASK) as u32;
    let y = ((ijk[1] >> (LOWER_AXIS_BITS + LEAF_AXIS_BITS)) & UPPER_AXIS_MASK) as u32;
    let z = ((ijk[2] >> (LOWER_AXIS_BITS + LEAF_AXIS_BITS)) & UPPER_AXIS_MASK) as u32;
    (x << (UPPER_AXIS_BITS * 2)) | (y << UPPER_AXIS_BITS) | z
}

fn lower_offset(ijk: [i32; 3]) -> u32 {
    let x = ((ijk[0] >> LEAF_AXIS_BITS) & LOWER_AXIS_MASK) as u32;
    let y = ((ijk[1] >> LEAF_AXIS_BITS) & LOWER_AXIS_MASK) as u32;
    let z = ((ijk[2] >> LEAF_AXIS_BITS) & LOWER_AXIS_MASK) as u32;
    (x << (LOWER_AXIS_BITS * 2)) | (y << LOWER_AXIS_BITS) | z
}

fn leaf_offset(ijk: [i32; 3]) -> u32 {
    let x = (ijk[0] & LEAF_AXIS_MASK) as u32;
    let y = (ijk[1] & LEAF_AXIS_MASK) as u32;
    let z = (ijk[2] & LEAF_AXIS_MASK) as u32;
    (x << (LEAF_AXIS_BITS * 2)) | (y << LEAF_AXIS_BITS) | z
}

fn mask_is_on(bytes: &[u8], mask: usize, offset: u32) -> bool {
    let word = mask + (offset / MASK_WORD_BITS) as usize * MASK_WORD_SIZE;
    read_u64(bytes, word)
        .map(|value| value & (1u64 << (offset % MASK_WORD_BITS)) != 0)
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
    (target % NODE_ALIGNMENT == 0).then_some(target)
}

fn validate_upper(bytes: &[u8], upper: usize, validated_lower: &mut HashSet<usize>) -> Option<()> {
    checked_range(bytes, upper, UPPER_SIZE)?;
    let mask = upper + UPPER_CHILD_MASK_OFFSET;
    let table = upper + UPPER_TABLE_OFFSET;
    for offset in 0..UPPER_TABLE_SIZE {
        if mask_is_on(bytes, mask, offset) {
            let child = read_i64(bytes, table + offset as usize * NODE_TABLE_STRIDE)?;
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
    checked_range(bytes, lower, LOWER_SIZE)?;
    let mask = lower + LOWER_CHILD_MASK_OFFSET;
    let table = lower + LOWER_TABLE_OFFSET;
    for offset in 0..LOWER_TABLE_SIZE {
        if mask_is_on(bytes, mask, offset) {
            let child = read_i64(bytes, table + offset as usize * NODE_TABLE_STRIDE)?;
            if child == 0 {
                return None;
            }
            let leaf = child_target(lower, child)?;
            if checked_range(bytes, leaf, LEAF_SIZE).is_none() {
                return None;
            }
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
        f32::from_le_bytes(
            bytes
                .get(offset..offset.checked_add(VALUE_COMPONENT_SIZE)?)?
                .try_into()
                .ok()?,
        ),
        f32::from_le_bytes(
            bytes
                .get(
                    offset.checked_add(VALUE_COMPONENT_SIZE)?
                        ..offset.checked_add(VALUE_COMPONENT_SIZE * 2)?,
                )?
                .try_into()
                .ok()?,
        ),
        f32::from_le_bytes(
            bytes
                .get(
                    offset.checked_add(VALUE_COMPONENT_SIZE * 2)?
                        ..offset.checked_add(VALUE_STRIDE)?,
                )?
                .try_into()
                .ok()?,
        ),
    ))
}
