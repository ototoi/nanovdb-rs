//! Float-valued NanoVDB tree walker (`NanoGrid<float>`).
//!
//! Mirrors `nanovdb::Tree<RootNode<InternalNode<InternalNode<LeafNode<float>>>>>`
//! at the byte level so a single `get_value((i, j, k))` call walks
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
const READ_ACCESSOR_LEAF_MASK: i32 = (1 << LEAF_TOTAL) - 1;
const READ_ACCESSOR_LOWER_MASK: i32 = (1 << LOWER_TOTAL) - 1;
const READ_ACCESSOR_UPPER_MASK: i32 = (1 << UPPER_TOTAL) - 1;
const LEAF_LEVEL: usize = 0;
const LOWER_LEVEL: usize = 1;
const UPPER_LEVEL: usize = 2;
const ROOT_LEVEL: usize = 3;

/// Parsed `nanovdb::TreeData` header fields used by the tree walker.
/// Cheap to compute and `Copy`, so it can be cached and passed by value
/// to repeated [`ReadAccessor::with_tree_data`]
/// calls.
#[derive(Debug, Clone, Copy)]
pub struct TreeData {
    /// NanoVDB `TreeData::mNodeOffset`.
    pub node_offset: [u64; 4],
    /// NanoVDB `TreeData::mNodeCount`.
    pub node_count: [u32; 3],
    /// NanoVDB `TreeData::mTileCount`.
    pub tile_count: [u32; 3],
    /// NanoVDB `TreeData::mVoxelCount`.
    pub voxel_count: u64,
}

impl TreeData {
    pub fn parse(bytes: &[u8]) -> Self {
        // TreeData layout (NanoVDB.h:2500):
        //   u64 mNodeOffset[4]  (0=leaf, 1=lower, 2=upper, 3=root)
        //   u32 mNodeCount[3]
        //   u32 mTileCount[3]
        //   u64 mVoxelCount
        debug_assert!(bytes.len() >= 64);
        TreeData {
            node_offset: [
                u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
                u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
                u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
                u64::from_le_bytes(bytes[24..32].try_into().unwrap()),
            ],
            node_count: [
                u32::from_le_bytes(bytes[32..36].try_into().unwrap()),
                u32::from_le_bytes(bytes[36..40].try_into().unwrap()),
                u32::from_le_bytes(bytes[40..44].try_into().unwrap()),
            ],
            tile_count: [
                u32::from_le_bytes(bytes[44..48].try_into().unwrap()),
                u32::from_le_bytes(bytes[48..52].try_into().unwrap()),
                u32::from_le_bytes(bytes[52..56].try_into().unwrap()),
            ],
            voxel_count: u64::from_le_bytes(bytes[56..64].try_into().unwrap()),
        }
    }

    pub fn root_offset(self) -> u64 {
        self.node_offset[3]
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
const UPPER_MASK_SIZE: usize = (1usize << (3 * UPPER_LOG2DIM as usize)) / 8;
const LOWER_MASK_SIZE: usize = (1usize << (3 * LOWER_LOG2DIM as usize)) / 8;

// ---- InternalData<ChildT, LOG2DIM> layout for Float (Upper LOG2DIM=5)
// 0..24    mBBox
// 24..32   mFlags (u64)
// 32..(32+M)  mValueMask  (M = (1<<3*LOG2DIM)/8 bytes)
// (32+M)..(32+2M) mChildMask
// (32+2M)..(32+2M+16) mMin,Max,Avg,StdDevi (4 × f32)
// pad to 32-byte alignment
// mTable[(1<<3*LOG2DIM)]  -- each entry is 8 bytes (union of f32 and i64)

const UPPER_HEADER_SIZE: usize = internal_header_size_const(UPPER_LOG2DIM);
const LOWER_HEADER_SIZE: usize = internal_header_size_const(LOWER_LOG2DIM);

const fn internal_header_size_const(log2dim: i32) -> usize {
    let n = 1usize << (3 * log2dim as usize);
    let mask_bytes = n / 8;
    let pre = 24 + 8 + mask_bytes * 2 + 16;
    (pre + 31) & !31
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

/// Random-access accessor over the Float tree.
///
/// The public concept follows `nanovdb::ReadAccessor`; the internal cache
/// layout follows `cnanovdb_readaccessor`: last key plus cached
/// Leaf/Lower/Upper/Root node offsets.
pub struct ReadAccessor<'a> {
    grid_bytes: &'a [u8],
    background: f32,
    root_abs: usize,
    root_table_size: u32,
    key: [i32; 3],
    node: [usize; 4],
}

impl<'a> ReadAccessor<'a> {
    fn initial_nodes(root_abs: usize) -> [usize; 4] {
        let mut node = [0; 4];
        node[ROOT_LEVEL] = root_abs;
        node
    }

    pub fn from_grid_bytes(bytes: &'a [u8]) -> Option<Self> {
        let header = GridDataHeader::parse(bytes)?;
        // Only Float and FogVolume-ish FloatGrids are handled here.
        // Use `GridType::from_raw` to confirm; caller can also check.
        if crate::types::GridType::from_raw(header.grid_type) != crate::types::GridType::Float {
            return None;
        }
        let tree_data_offset = GRID_DATA_SIZE;
        let tree = TreeData::parse(&bytes[tree_data_offset..tree_data_offset + 64]);
        // background is RootData[28..32].
        let root_abs = tree_data_offset + tree.root_offset() as usize;
        let background =
            f32::from_le_bytes(bytes[root_abs + 28..root_abs + 32].try_into().unwrap());
        let root_table_size =
            u32::from_le_bytes(bytes[root_abs + 24..root_abs + 28].try_into().unwrap());
        Some(ReadAccessor {
            grid_bytes: bytes,
            background,
            root_abs,
            root_table_size,
            key: [0; 3],
            node: Self::initial_nodes(root_abs),
        })
    }

    /// Parse just the bits the tree walker needs (`TreeData` +
    /// `background`) without going through the full `GridDataHeader`
    /// parse. Use this once at scene-build time and pair the result
    /// with [`ReadAccessor::with_tree_data`] on the hot path to avoid the
    /// `String` allocation that `GridDataHeader::parse` does on every
    /// call.
    ///
    /// Returns `None` if the grid is not a `Float` grid or the bytes
    /// are too short to hold a valid header.
    pub fn parse_tree_data(bytes: &[u8]) -> Option<(TreeData, f32)> {
        if bytes.len() < GRID_DATA_SIZE + 64 {
            return None;
        }
        // GridType lives at offset 636 in GridData.
        let grid_type = u32::from_le_bytes(bytes[636..640].try_into().unwrap());
        if crate::types::GridType::from_raw(grid_type) != crate::types::GridType::Float {
            return None;
        }
        let tree_data_offset = GRID_DATA_SIZE;
        let tree = TreeData::parse(&bytes[tree_data_offset..tree_data_offset + 64]);
        let root_abs = tree_data_offset + tree.root_offset() as usize;
        if root_abs + 32 > bytes.len() {
            return None;
        }
        let background =
            f32::from_le_bytes(bytes[root_abs + 28..root_abs + 32].try_into().unwrap());
        Some((tree, background))
    }

    /// Construct a `ReadAccessor` from precomputed `TreeData` and
    /// background value (see [`ReadAccessor::parse_tree_data`]). This is
    /// the cheap path: no header parse, no allocation. Suitable for use
    /// in inner loops that need to read voxels billions of times.
    ///
    /// Safety / correctness: `bytes` must be the same grid bytes that
    /// were used to compute `tree`/`background`. The accessor blindly
    /// trusts the offsets.
    pub fn with_tree_data(bytes: &'a [u8], tree: TreeData, background: f32) -> Self {
        let root_abs = GRID_DATA_SIZE + tree.root_offset() as usize;
        let root_table_size =
            u32::from_le_bytes(bytes[root_abs + 24..root_abs + 28].try_into().unwrap());
        ReadAccessor {
            grid_bytes: bytes,
            background,
            root_abs,
            root_table_size,
            key: [0; 3],
            node: Self::initial_nodes(root_abs),
        }
    }

    /// `nanovdb::Tree::background()` -- value of unset voxels.
    pub fn background(&self) -> f32 {
        self.background
    }

    fn insert(&mut self, child_level: usize, node_abs: usize, ijk: [i32; 3]) {
        self.node[child_level] = node_abs;
        self.key = ijk;
    }

    fn compute_dirty(&self, ijk: [i32; 3]) -> i32 {
        (ijk[0] ^ self.key[0]) | (ijk[1] ^ self.key[1]) | (ijk[2] ^ self.key[2])
    }

    fn is_cached(&mut self, level: usize, dirty: i32, mask: i32) -> bool {
        if self.node[level] == 0 {
            return false;
        }
        if dirty & !mask != 0 {
            self.node[level] = 0;
            return false;
        }
        true
    }

    /// Random-access voxel lookup. Returns `background` for inactive /
    /// missing voxels, matching v4's `Tree::getValue(ijk)` behaviour.
    ///
    /// This updates the `ReadAccessor` cache, mirroring
    /// `cnanovdb_readaccessor_getValueF`.
    pub fn get_value(&mut self, ijk: [i32; 3]) -> f32 {
        let dirty = self.compute_dirty(ijk);
        if self.is_cached(LEAF_LEVEL, dirty, READ_ACCESSOR_LEAF_MASK) {
            return self.node0_get_value(self.node[LEAF_LEVEL], ijk);
        }
        if self.is_cached(LOWER_LEVEL, dirty, READ_ACCESSOR_LOWER_MASK) {
            return self.node1_get_value_and_cache(self.node[LOWER_LEVEL], ijk);
        }
        if self.is_cached(UPPER_LEVEL, dirty, READ_ACCESSOR_UPPER_MASK) {
            return self.node2_get_value_and_cache(self.node[UPPER_LEVEL], ijk);
        }
        self.rootdata_get_value_and_cache(ijk)
    }

    fn rootdata_find_tile(&self, ijk: [i32; 3]) -> Option<usize> {
        let key = coord_to_root_key(ijk);
        let tiles_off = self.root_abs + ROOT_HEADER_SIZE;
        for n in 0..self.root_table_size as usize {
            let tile_off = tiles_off + n * ROOT_TILE_SIZE;
            let tkey =
                u64::from_le_bytes(self.grid_bytes[tile_off..tile_off + 8].try_into().unwrap());
            if tkey == key {
                return Some(tile_off);
            }
        }
        None
    }

    fn rootdata_tile_child(&self, tile_off: usize) -> i64 {
        i64::from_le_bytes(
            self.grid_bytes[tile_off + 8..tile_off + 16]
                .try_into()
                .unwrap(),
        )
    }

    fn rootdata_get_child(&self, tile_off: usize) -> usize {
        let child = self.rootdata_tile_child(tile_off);
        (self.root_abs as i64 + child) as usize
    }

    fn rootdata_get_value_and_cache(&mut self, ijk: [i32; 3]) -> f32 {
        let tile_off = match self.rootdata_find_tile(ijk) {
            Some(off) => off,
            None => return self.background,
        };
        let child = self.rootdata_tile_child(tile_off);
        if child == 0 {
            // No child -> use the tile's value.
            return f32::from_le_bytes(
                self.grid_bytes[tile_off + 20..tile_off + 24]
                    .try_into()
                    .unwrap(),
            );
        }
        let upper_abs = self.rootdata_get_child(tile_off);
        self.insert(UPPER_LEVEL, upper_abs, ijk);
        self.node2_get_value_and_cache(upper_abs, ijk)
    }

    fn node2_get_value_and_cache(&mut self, upper_abs: usize, ijk: [i32; 3]) -> f32 {
        let off = upper_offset(ijk);
        let value_mask_off = upper_abs + 24 + 8;
        let child_mask_off = value_mask_off + UPPER_MASK_SIZE;
        let table_off = upper_abs + UPPER_HEADER_SIZE;
        let entry_off = table_off + (off as usize) * 8;

        if mask_is_on(
            &self.grid_bytes[child_mask_off..child_mask_off + UPPER_MASK_SIZE],
            off,
        ) {
            // child slot -> lower internal node
            let child_byte_off = i64::from_le_bytes(
                self.grid_bytes[entry_off..entry_off + 8]
                    .try_into()
                    .unwrap(),
            );
            let lower_abs = (upper_abs as i64 + child_byte_off) as usize;
            self.insert(LOWER_LEVEL, lower_abs, ijk);
            self.node1_get_value_and_cache(lower_abs, ijk)
        } else {
            // tile value (active or inactive both stored as f32)
            f32::from_le_bytes(
                self.grid_bytes[entry_off..entry_off + 4]
                    .try_into()
                    .unwrap(),
            )
        }
    }

    fn node1_get_value_and_cache(&mut self, lower_abs: usize, ijk: [i32; 3]) -> f32 {
        let off = lower_offset(ijk);
        let value_mask_off = lower_abs + 24 + 8;
        let child_mask_off = value_mask_off + LOWER_MASK_SIZE;
        let table_off = lower_abs + LOWER_HEADER_SIZE;
        let entry_off = table_off + (off as usize) * 8;

        if mask_is_on(
            &self.grid_bytes[child_mask_off..child_mask_off + LOWER_MASK_SIZE],
            off,
        ) {
            let child_byte_off = i64::from_le_bytes(
                self.grid_bytes[entry_off..entry_off + 8]
                    .try_into()
                    .unwrap(),
            );
            let leaf_abs = (lower_abs as i64 + child_byte_off) as usize;
            self.insert(LEAF_LEVEL, leaf_abs, ijk);
            self.node0_get_value(leaf_abs, ijk)
        } else {
            f32::from_le_bytes(
                self.grid_bytes[entry_off..entry_off + 4]
                    .try_into()
                    .unwrap(),
            )
        }
    }

    fn node0_get_value(&self, leaf_abs: usize, ijk: [i32; 3]) -> f32 {
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
            // `is_active` if you need it.
        }
        let val_off = leaf_abs + LEAF_VALUES_OFF + (off as usize) * 4;
        f32::from_le_bytes(self.grid_bytes[val_off..val_off + 4].try_into().unwrap())
    }

    /// True if `(i, j, k)` is in the value mask of its leaf, matching
    /// v4 `Tree::isActive`.
    pub fn is_active(&mut self, ijk: [i32; 3]) -> bool {
        let dirty = self.compute_dirty(ijk);
        if self.is_cached(LEAF_LEVEL, dirty, READ_ACCESSOR_LEAF_MASK) {
            return self.node0_is_active(self.node[LEAF_LEVEL], ijk);
        }
        if self.is_cached(LOWER_LEVEL, dirty, READ_ACCESSOR_LOWER_MASK) {
            return self.node1_is_active_and_cache(self.node[LOWER_LEVEL], ijk);
        }
        if self.is_cached(UPPER_LEVEL, dirty, READ_ACCESSOR_UPPER_MASK) {
            return self.node2_is_active_and_cache(self.node[UPPER_LEVEL], ijk);
        }
        self.rootdata_is_active_and_cache(ijk)
    }

    fn rootdata_is_active_and_cache(&mut self, ijk: [i32; 3]) -> bool {
        let Some(tile_off) = self.rootdata_find_tile(ijk) else {
            return false;
        };
        let child = self.rootdata_tile_child(tile_off);
        if child == 0 {
            let state = u32::from_le_bytes(
                self.grid_bytes[tile_off + 16..tile_off + 20]
                    .try_into()
                    .unwrap(),
            );
            return state != 0;
        }
        let upper_abs = self.rootdata_get_child(tile_off);
        self.insert(UPPER_LEVEL, upper_abs, ijk);
        self.node2_is_active_and_cache(upper_abs, ijk)
    }

    fn node2_is_active_and_cache(&mut self, upper_abs: usize, ijk: [i32; 3]) -> bool {
        let off = upper_offset(ijk);
        let value_mask_off = upper_abs + 24 + 8;
        let child_mask_off = value_mask_off + UPPER_MASK_SIZE;
        let table_off = upper_abs + UPPER_HEADER_SIZE;
        let entry_off = table_off + (off as usize) * 8;
        if mask_is_on(
            &self.grid_bytes[child_mask_off..child_mask_off + UPPER_MASK_SIZE],
            off,
        ) {
            let child_byte_off = i64::from_le_bytes(
                self.grid_bytes[entry_off..entry_off + 8]
                    .try_into()
                    .unwrap(),
            );
            let lower_abs = (upper_abs as i64 + child_byte_off) as usize;
            self.insert(LOWER_LEVEL, lower_abs, ijk);
            self.node1_is_active_and_cache(lower_abs, ijk)
        } else {
            mask_is_on(
                &self.grid_bytes[value_mask_off..value_mask_off + UPPER_MASK_SIZE],
                off,
            )
        }
    }

    fn node1_is_active_and_cache(&mut self, lower_abs: usize, ijk: [i32; 3]) -> bool {
        let off = lower_offset(ijk);
        let value_mask_off = lower_abs + 24 + 8;
        let child_mask_off = value_mask_off + LOWER_MASK_SIZE;
        let table_off = lower_abs + LOWER_HEADER_SIZE;
        let entry_off = table_off + (off as usize) * 8;
        if mask_is_on(
            &self.grid_bytes[child_mask_off..child_mask_off + LOWER_MASK_SIZE],
            off,
        ) {
            let child_byte_off = i64::from_le_bytes(
                self.grid_bytes[entry_off..entry_off + 8]
                    .try_into()
                    .unwrap(),
            );
            let leaf_abs = (lower_abs as i64 + child_byte_off) as usize;
            self.insert(LEAF_LEVEL, leaf_abs, ijk);
            self.node0_is_active(leaf_abs, ijk)
        } else {
            mask_is_on(
                &self.grid_bytes[value_mask_off..value_mask_off + LOWER_MASK_SIZE],
                off,
            )
        }
    }

    fn node0_is_active(&self, leaf_abs: usize, ijk: [i32; 3]) -> bool {
        let value_mask_off = leaf_abs + LEAF_VALUE_MASK_OFF;
        mask_is_on(
            &self.grid_bytes[value_mask_off..value_mask_off + 64],
            leaf_offset(ijk),
        )
    }
}
