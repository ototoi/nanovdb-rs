//! Common NanoVDB tree metadata.

/// Parsed `nanovdb::TreeData` header fields shared by all value types.
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
