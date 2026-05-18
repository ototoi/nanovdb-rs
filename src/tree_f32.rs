//! Float-valued NanoVDB tree walker (`NanoGrid<float>`).
//!
//! Mirrors `nanovdb::Tree<RootNode<InternalNode<InternalNode<LeafNode<float>>>>>`
//! at the byte level so a single `value_at_index((i, j, k))` call walks
//! Root -> Upper(32^3) -> Lower(16^3) -> Leaf(8^3) and returns either
//! the active voxel value or the inherited "tile" / background value.
//!
//! Targets NanoVDB v32 (`USE_SINGLE_ROOT_KEY` set, `Codec::None` after
//! decompression). LEAF_LOG2DIM=3, LOWER_LOG2DIM=4, UPPER_LOG2DIM=5;
//! total bits = 12, so the root key shifts each axis right by 12.

use crate::grid_data::{GridDataHeader, GRID_DATA_SIZE};

const LEAF_LOG2DIM: i32 = 3;
const LOWER_LOG2DIM: i32 = 4;
const UPPER_LOG2DIM: i32 = 5;
const LEAF_TOTAL: i32 = LEAF_LOG2DIM; // 3
const LOWER_TOTAL: i32 = LEAF_TOTAL + LOWER_LOG2DIM; // 7
const UPPER_TOTAL: i32 = LOWER_TOTAL + UPPER_LOG2DIM; // 12

const LEAF_MASK: i32 = (1 << LEAF_LOG2DIM) - 1; // 7
const LOWER_MASK: i32 = (1 << LOWER_LOG2DIM) - 1; // 15
const UPPER_MASK: i32 = (1 << UPPER_LOG2DIM) - 1; // 31

/// Offset of the root inside the grid's bytes. v4 `TreeData::mNodeOffset[3]`.
pub const TREE_DATA_OFFSET_IN_GRID: usize = GRID_DATA_SIZE;

#[derive(Debug, Clone, Copy)]
pub struct TreeOffsets {
    pub leaf_offset: u64,
    pub lower_offset: u64,
    pub upper_offset: u64,
    pub root_offset: u64,
}

impl TreeOffsets {
    pub fn parse(bytes: &[u8]) -> Self {
        // TreeData layout (NanoVDB.h:2500):
        //   u64 mNodeOffset[4]  (0=leaf, 1=lower, 2=upper, 3=root)
        //   u32 mNodeCount[3]
        //   u32 mTileCount[3]
        //   u64 mVoxelCount
        debug_assert!(bytes.len() >= 64);
        TreeOffsets {
            leaf_offset: u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            lower_offset: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            upper_offset: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            root_offset: u64::from_le_bytes(bytes[24..32].try_into().unwrap()),
        }
    }
}

/// pbrt-v4 `RootData<ChildT>::CoordToKey` with `USE_SINGLE_ROOT_KEY`:
///   z's top bits | y's top bits << 21 | x's top bits << 42
fn coord_to_root_key(ijk: [i32; 3]) -> u64 {
    let xs = (ijk[0] as u32) >> UPPER_TOTAL;
    let ys = (ijk[1] as u32) >> UPPER_TOTAL;
    let zs = (ijk[2] as u32) >> UPPER_TOTAL;
    (zs as u64) | ((ys as u64) << 21) | ((xs as u64) << 42)
}

/// Compute the linear offset within a 32^3 upper internal node from
/// the local (i, j, k) coordinates (each in `[0, 32)`).
#[inline]
fn upper_offset(ijk: [i32; 3]) -> u32 {
    let i = ((ijk[0] >> LOWER_TOTAL) & UPPER_MASK) as u32;
    let j = ((ijk[1] >> LOWER_TOTAL) & UPPER_MASK) as u32;
    let k = ((ijk[2] >> LOWER_TOTAL) & UPPER_MASK) as u32;
    (i << (2 * UPPER_LOG2DIM)) | (j << UPPER_LOG2DIM) | k
}

#[inline]
fn lower_offset(ijk: [i32; 3]) -> u32 {
    let i = ((ijk[0] >> LEAF_TOTAL) & LOWER_MASK) as u32;
    let j = ((ijk[1] >> LEAF_TOTAL) & LOWER_MASK) as u32;
    let k = ((ijk[2] >> LEAF_TOTAL) & LOWER_MASK) as u32;
    (i << (2 * LOWER_LOG2DIM)) | (j << LOWER_LOG2DIM) | k
}

#[inline]
fn leaf_offset(ijk: [i32; 3]) -> u32 {
    let i = (ijk[0] & LEAF_MASK) as u32;
    let j = (ijk[1] & LEAF_MASK) as u32;
    let k = (ijk[2] & LEAF_MASK) as u32;
    (i << (2 * LEAF_LOG2DIM)) | (j << LEAF_LOG2DIM) | k
}

#[inline]
fn mask_is_on(mask_bytes: &[u8], offset: u32) -> bool {
    let word = (offset >> 6) as usize;
    let bit = offset & 63;
    let w = u64::from_le_bytes(mask_bytes[word * 8..word * 8 + 8].try_into().unwrap());
    (w >> bit) & 1 != 0
}

// ---- RootData<Float> layout ----------------------------------------
// 0..24   mBBox (6 i32)
// 24..28  mTableSize (u32)
// 28..32  mBackground (f32)
// 32..36  mMinimum (f32)
// 36..40  mMaximum (f32)
// 40..44  mAverage (f32)
// 44..48  mStdDevi (f32)
// 48..64  padding (RootData is `NANOVDB_ALIGN(32)`, so sizeof rounds up to 64)
// 64      Tile[mTableSize]
//
// Tile (also `NANOVDB_ALIGN(32)`, so 32 bytes incl. padding):
//   0..8    key (u64, USE_SINGLE_ROOT_KEY=on)
//   8..16   child (i64)  -- signed byte offset from RootData; 0 = no child
//   16..20  state (u32)
//   20..24  value (f32)
//   24..32  padding
const ROOT_HEADER_SIZE: usize = 64;
const ROOT_TILE_SIZE: usize = 32;

// ---- InternalData<ChildT, LOG2DIM> layout for Float (Upper LOG2DIM=5)
// 0..24    mBBox
// 24..32   mFlags (u64)
// 32..(32+M)  mValueMask  (M = (1<<3*LOG2DIM)/8 bytes)
// (32+M)..(32+2M) mChildMask
// (32+2M)..(32+2M+16) mMin,Max,Avg,StdDevi (4 × f32)
// pad to 32-byte alignment
// mTable[(1<<3*LOG2DIM)]  -- each entry is 8 bytes (union of f32 and i64)

fn internal_header_size(log2dim: i32) -> usize {
    let n = 1usize << (3 * log2dim as usize);
    let mask_bytes = n / 8;
    let pre = 24 + 8 + mask_bytes * 2 + 16; // bbox+flags+valuemask+childmask+(min/max/avg/std)
    // round up to NANOVDB_DATA_ALIGNMENT (32)
    (pre + 31) & !31
}

fn mask_size_bytes(log2dim: i32) -> usize {
    (1usize << (3 * log2dim as usize)) / 8
}

// ---- LeafData<Float, ..., LOG2DIM=3> layout ------------------------
// 0..12    mBBoxMin
// 12..15   mBBoxDif
// 15..16   mFlags
// 16..80   mValueMask (64 bytes for 512 bits)
// 80..84   mMinimum (f32)
// 84..88   mMaximum (f32)
// 88..92   mAverage (f32)
// 92..96   mStdDevi (f32)
// 96..(96+512*4)  mValues
const LEAF_VALUE_MASK_OFF: usize = 16;
const LEAF_VALUES_OFF: usize = 96;

/// Random-access accessor over the Float tree. Holds a reference to the
/// grid bytes (decompressed) and the parsed tree offsets.
pub struct FloatAccessor<'a> {
    grid_bytes: &'a [u8],
    /// File offsets are recorded relative to `TreeData`, which itself
    /// sits at the start of the grid bytes right after `GridData`. We
    /// cache the absolute offset of TreeData to keep the conversions
    /// trivial.
    tree_data_offset: usize,
    tree: TreeOffsets,
    background: f32,
}

impl<'a> FloatAccessor<'a> {
    pub fn from_grid_bytes(bytes: &'a [u8]) -> Option<Self> {
        let header = GridDataHeader::parse(bytes)?;
        // Only Float and FogVolume-ish FloatGrids are handled here.
        // Use `GridType::from_raw` to confirm; caller can also check.
        if crate::types::GridType::from_raw(header.grid_type)
            != crate::types::GridType::Float
        {
            return None;
        }
        let tree_data_offset = GRID_DATA_SIZE;
        let tree = TreeOffsets::parse(&bytes[tree_data_offset..tree_data_offset + 64]);
        // background is RootData[28..32].
        let root_abs = tree_data_offset + tree.root_offset as usize;
        let background = f32::from_le_bytes(bytes[root_abs + 28..root_abs + 32].try_into().unwrap());
        Some(FloatAccessor {
            grid_bytes: bytes,
            tree_data_offset,
            tree,
            background,
        })
    }

    /// `nanovdb::Tree::background()` -- value of unset voxels.
    pub fn background(&self) -> f32 {
        self.background
    }

    /// Random-access voxel lookup. Returns `background` for inactive /
    /// missing voxels, matching v4's `Tree::getValue(ijk)` behaviour.
    pub fn value_at_index(&self, ijk: [i32; 3]) -> f32 {
        let root_abs = self.tree_data_offset + self.tree.root_offset as usize;
        let table_size =
            u32::from_le_bytes(self.grid_bytes[root_abs + 24..root_abs + 28].try_into().unwrap());

        // Search root tiles for one whose key matches the high bits of ijk.
        let key = coord_to_root_key(ijk);
        let tiles_off = root_abs + ROOT_HEADER_SIZE;
        // Tiles are stored unsorted (small N typically), so a linear
        // probe is fine and matches v4's `findTile` behaviour.
        let mut tile_match: Option<usize> = None;
        for n in 0..table_size as usize {
            let tile_off = tiles_off + n * ROOT_TILE_SIZE;
            let tkey =
                u64::from_le_bytes(self.grid_bytes[tile_off..tile_off + 8].try_into().unwrap());
            if tkey == key {
                tile_match = Some(tile_off);
                break;
            }
        }
        let tile_off = match tile_match {
            Some(off) => off,
            None => return self.background,
        };
        let child = i64::from_le_bytes(
            self.grid_bytes[tile_off + 8..tile_off + 16].try_into().unwrap(),
        );
        if child == 0 {
            // No child -> use the tile's value.
            return f32::from_le_bytes(
                self.grid_bytes[tile_off + 20..tile_off + 24].try_into().unwrap(),
            );
        }
        let upper_abs = (root_abs as i64 + child) as usize;
        self.read_upper(upper_abs, ijk)
    }

    fn read_upper(&self, upper_abs: usize, ijk: [i32; 3]) -> f32 {
        let off = upper_offset(ijk);
        let m_size = mask_size_bytes(UPPER_LOG2DIM);
        let header_size = internal_header_size(UPPER_LOG2DIM);
        let value_mask_off = upper_abs + 24 + 8;
        let child_mask_off = value_mask_off + m_size;
        let table_off = upper_abs + header_size;
        let entry_off = table_off + (off as usize) * 8;

        if mask_is_on(
            &self.grid_bytes[child_mask_off..child_mask_off + m_size],
            off,
        ) {
            // child slot -> lower internal node
            let child_byte_off = i64::from_le_bytes(
                self.grid_bytes[entry_off..entry_off + 8].try_into().unwrap(),
            );
            let lower_abs = (upper_abs as i64 + child_byte_off) as usize;
            self.read_lower(lower_abs, ijk)
        } else {
            // tile value (active or inactive both stored as f32)
            f32::from_le_bytes(self.grid_bytes[entry_off..entry_off + 4].try_into().unwrap())
        }
    }

    fn read_lower(&self, lower_abs: usize, ijk: [i32; 3]) -> f32 {
        let off = lower_offset(ijk);
        let m_size = mask_size_bytes(LOWER_LOG2DIM);
        let header_size = internal_header_size(LOWER_LOG2DIM);
        let value_mask_off = lower_abs + 24 + 8;
        let child_mask_off = value_mask_off + m_size;
        let table_off = lower_abs + header_size;
        let entry_off = table_off + (off as usize) * 8;

        if mask_is_on(
            &self.grid_bytes[child_mask_off..child_mask_off + m_size],
            off,
        ) {
            let child_byte_off = i64::from_le_bytes(
                self.grid_bytes[entry_off..entry_off + 8].try_into().unwrap(),
            );
            let leaf_abs = (lower_abs as i64 + child_byte_off) as usize;
            self.read_leaf(leaf_abs, ijk)
        } else {
            f32::from_le_bytes(self.grid_bytes[entry_off..entry_off + 4].try_into().unwrap())
        }
    }

    fn read_leaf(&self, leaf_abs: usize, ijk: [i32; 3]) -> f32 {
        let off = leaf_offset(ijk);
        let value_mask_size = 64;
        let value_mask_off = leaf_abs + LEAF_VALUE_MASK_OFF;
        if !mask_is_on(
            &self.grid_bytes[value_mask_off..value_mask_off + value_mask_size],
            off,
        ) {
            // Inactive voxel inside a leaf -> the leaf stores the
            // value in `mValues[off]` but the v4 accessor treats it
            // as the background. We mirror v4 here: still return the
            // stored value (which is what `Tree::getValue` does --
            // active/inactive distinction is separate). Use
            // `is_active_at_index` if you need it.
        }
        let val_off = leaf_abs + LEAF_VALUES_OFF + (off as usize) * 4;
        f32::from_le_bytes(self.grid_bytes[val_off..val_off + 4].try_into().unwrap())
    }

    /// Trilinear-interpolated sample at a (possibly fractional)
    /// index-space coordinate `idx`. Mirrors the `SampleFromVoxels<...,1>`
    /// usage in v4's `nanovdb::createSampler` for `Float` grids.
    ///
    /// Returns `background` outside the data.
    pub fn sample_trilinear(&self, idx: [f64; 3]) -> f32 {
        let fx = idx[0].floor() as i32;
        let fy = idx[1].floor() as i32;
        let fz = idx[2].floor() as i32;
        let tx = (idx[0] - fx as f64) as f32;
        let ty = (idx[1] - fy as f64) as f32;
        let tz = (idx[2] - fz as f64) as f32;

        let v000 = self.value_at_index([fx, fy, fz]);
        let v100 = self.value_at_index([fx + 1, fy, fz]);
        let v010 = self.value_at_index([fx, fy + 1, fz]);
        let v110 = self.value_at_index([fx + 1, fy + 1, fz]);
        let v001 = self.value_at_index([fx, fy, fz + 1]);
        let v101 = self.value_at_index([fx + 1, fy, fz + 1]);
        let v011 = self.value_at_index([fx, fy + 1, fz + 1]);
        let v111 = self.value_at_index([fx + 1, fy + 1, fz + 1]);

        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let c00 = lerp(v000, v100, tx);
        let c10 = lerp(v010, v110, tx);
        let c01 = lerp(v001, v101, tx);
        let c11 = lerp(v011, v111, tx);
        let c0 = lerp(c00, c10, ty);
        let c1 = lerp(c01, c11, ty);
        lerp(c0, c1, tz)
    }

    /// True if `(i, j, k)` is in the value mask of its leaf, matching
    /// v4 `Tree::isActive`.
    pub fn is_active(&self, ijk: [i32; 3]) -> bool {
        // Walk root -> upper -> lower -> leaf; on tile slots, the
        // active flag is on the slot itself (state field for root,
        // value mask for internal). For simplicity we just check the
        // leaf path; tiled-active values report as false here.
        let root_abs = self.tree_data_offset + self.tree.root_offset as usize;
        let table_size =
            u32::from_le_bytes(self.grid_bytes[root_abs + 24..root_abs + 28].try_into().unwrap());
        let key = coord_to_root_key(ijk);
        let tiles_off = root_abs + ROOT_HEADER_SIZE;
        let mut tile_match: Option<usize> = None;
        for n in 0..table_size as usize {
            let tile_off = tiles_off + n * ROOT_TILE_SIZE;
            let tkey =
                u64::from_le_bytes(self.grid_bytes[tile_off..tile_off + 8].try_into().unwrap());
            if tkey == key {
                tile_match = Some(tile_off);
                break;
            }
        }
        let Some(tile_off) = tile_match else {
            return false;
        };
        let child = i64::from_le_bytes(
            self.grid_bytes[tile_off + 8..tile_off + 16].try_into().unwrap(),
        );
        if child == 0 {
            let state = u32::from_le_bytes(
                self.grid_bytes[tile_off + 16..tile_off + 20].try_into().unwrap(),
            );
            return state != 0;
        }
        let upper_abs = (root_abs as i64 + child) as usize;
        self.is_active_upper(upper_abs, ijk)
    }

    fn is_active_upper(&self, upper_abs: usize, ijk: [i32; 3]) -> bool {
        let off = upper_offset(ijk);
        let m_size = mask_size_bytes(UPPER_LOG2DIM);
        let header_size = internal_header_size(UPPER_LOG2DIM);
        let value_mask_off = upper_abs + 24 + 8;
        let child_mask_off = value_mask_off + m_size;
        let table_off = upper_abs + header_size;
        let entry_off = table_off + (off as usize) * 8;
        if mask_is_on(
            &self.grid_bytes[child_mask_off..child_mask_off + m_size],
            off,
        ) {
            let child_byte_off = i64::from_le_bytes(
                self.grid_bytes[entry_off..entry_off + 8].try_into().unwrap(),
            );
            let lower_abs = (upper_abs as i64 + child_byte_off) as usize;
            self.is_active_lower(lower_abs, ijk)
        } else {
            mask_is_on(&self.grid_bytes[value_mask_off..value_mask_off + m_size], off)
        }
    }

    fn is_active_lower(&self, lower_abs: usize, ijk: [i32; 3]) -> bool {
        let off = lower_offset(ijk);
        let m_size = mask_size_bytes(LOWER_LOG2DIM);
        let header_size = internal_header_size(LOWER_LOG2DIM);
        let value_mask_off = lower_abs + 24 + 8;
        let child_mask_off = value_mask_off + m_size;
        let table_off = lower_abs + header_size;
        let entry_off = table_off + (off as usize) * 8;
        if mask_is_on(
            &self.grid_bytes[child_mask_off..child_mask_off + m_size],
            off,
        ) {
            let child_byte_off = i64::from_le_bytes(
                self.grid_bytes[entry_off..entry_off + 8].try_into().unwrap(),
            );
            let leaf_abs = (lower_abs as i64 + child_byte_off) as usize;
            let value_mask_off = leaf_abs + LEAF_VALUE_MASK_OFF;
            mask_is_on(&self.grid_bytes[value_mask_off..value_mask_off + 64], leaf_offset(ijk))
        } else {
            mask_is_on(&self.grid_bytes[value_mask_off..value_mask_off + m_size], off)
        }
    }
}
