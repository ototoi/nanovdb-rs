//! In-memory grid header parsing.
//!
//! Mirrors `nanovdb::GridData` (NanoVDB.h:2184) and the embedded
//! `nanovdb::Map` (NanoVDB.h:1997) used for index <-> world transforms.
//! Targets NanoVDB v32 (used by pbrt-v4-scenes).

use crate::types::Vec3d;

/// `nanovdb::GridData` size on disk / in-memory.
pub const GRID_DATA_SIZE: usize = 672;
/// `nanovdb::Map` size: 264B (single + double precision matrices and
/// translation vectors, plus two taper placeholders).
pub const MAP_SIZE: usize = 264;
/// Maximum grid name length (including the null terminator).
pub const MAX_NAME_SIZE: usize = 256;

/// World <-> index affine transform stored inside `GridData`. The
/// double-precision matrices are what NanoVDB writes by default; the
/// `f32` slots are kept for shader-side use and are redundant here.
#[derive(Debug, Clone)]
pub struct Map {
    /// 3x3 mapping matrix index -> world (row-major)
    pub mat_d: [[f64; 3]; 3],
    /// 3x3 inverse mapping (world -> index)
    pub inv_mat_d: [[f64; 3]; 3],
    /// Translation (index origin in world space).
    pub vec_d: [f64; 3],
}

impl Map {
    pub fn parse(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() >= MAP_SIZE);
        // Skip the single-precision slot (88 bytes); start of the
        // double-precision block.
        let dbl_off = 88;
        let mut mat_d = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                let off = dbl_off + (i * 3 + j) * 8;
                mat_d[i][j] = f64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
            }
        }
        let mut inv_mat_d = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                let off = dbl_off + 72 + (i * 3 + j) * 8;
                inv_mat_d[i][j] = f64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
            }
        }
        let vec_d_off = dbl_off + 144;
        let vec_d = [
            f64::from_le_bytes(bytes[vec_d_off..vec_d_off + 8].try_into().unwrap()),
            f64::from_le_bytes(bytes[vec_d_off + 8..vec_d_off + 16].try_into().unwrap()),
            f64::from_le_bytes(bytes[vec_d_off + 16..vec_d_off + 24].try_into().unwrap()),
        ];
        Map {
            mat_d,
            inv_mat_d,
            vec_d,
        }
    }

    /// pbrt-v4 `Map::applyMap(idx) = mat_d * idx + vec_d`. The
    /// "voxel" -> "world" direction.
    pub fn apply_map(&self, idx: Vec3d) -> Vec3d {
        let x = self.mat_d[0][0] * idx.x
            + self.mat_d[0][1] * idx.y
            + self.mat_d[0][2] * idx.z
            + self.vec_d[0];
        let y = self.mat_d[1][0] * idx.x
            + self.mat_d[1][1] * idx.y
            + self.mat_d[1][2] * idx.z
            + self.vec_d[1];
        let z = self.mat_d[2][0] * idx.x
            + self.mat_d[2][1] * idx.y
            + self.mat_d[2][2] * idx.z
            + self.vec_d[2];
        Vec3d::new(x, y, z)
    }

    /// pbrt-v4 `Map::applyInverseMap(world) = inv_mat * (world - vec)`.
    /// The "world" -> "voxel" direction.
    pub fn apply_inverse_map(&self, world: Vec3d) -> Vec3d {
        let d = [
            world.x - self.vec_d[0],
            world.y - self.vec_d[1],
            world.z - self.vec_d[2],
        ];
        let x =
            self.inv_mat_d[0][0] * d[0] + self.inv_mat_d[0][1] * d[1] + self.inv_mat_d[0][2] * d[2];
        let y =
            self.inv_mat_d[1][0] * d[0] + self.inv_mat_d[1][1] * d[1] + self.inv_mat_d[1][2] * d[2];
        let z =
            self.inv_mat_d[2][0] * d[0] + self.inv_mat_d[2][1] * d[1] + self.inv_mat_d[2][2] * d[2];
        Vec3d::new(x, y, z)
    }
}

/// Parsed header that lives at the start of every grid's bytes
/// (`Grid::raw_bytes()` offset 0).
#[derive(Debug, Clone)]
pub struct GridDataHeader {
    pub magic: u64,
    pub checksum: u64,
    pub version: crate::header::Version,
    pub flags: u32,
    pub grid_index: u32,
    pub grid_count: u32,
    pub grid_size: u64,
    pub grid_name: String,
    pub map: Map,
    pub world_bbox_min: Vec3d,
    pub world_bbox_max: Vec3d,
    pub voxel_size: Vec3d,
    pub grid_class: u32,
    pub grid_type: u32,
    pub blind_metadata_offset: i64,
    pub blind_metadata_count: u32,
}

impl GridDataHeader {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < GRID_DATA_SIZE {
            return None;
        }
        let magic = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let checksum = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let version = crate::header::Version(u32::from_le_bytes(bytes[16..20].try_into().unwrap()));
        let flags = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
        let grid_index = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let grid_count = u32::from_le_bytes(bytes[28..32].try_into().unwrap());
        let grid_size = u64::from_le_bytes(bytes[32..40].try_into().unwrap());

        let name_bytes = &bytes[40..40 + MAX_NAME_SIZE];
        let grid_name = String::from_utf8_lossy(
            &name_bytes[..name_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(name_bytes.len())],
        )
        .into_owned();

        let map_off = 40 + MAX_NAME_SIZE; // 296
        let map = Map::parse(&bytes[map_off..map_off + MAP_SIZE]);

        let bbox_off = map_off + MAP_SIZE; // 560
        let world_bbox_min = Vec3d::new(
            f64::from_le_bytes(bytes[bbox_off..bbox_off + 8].try_into().unwrap()),
            f64::from_le_bytes(bytes[bbox_off + 8..bbox_off + 16].try_into().unwrap()),
            f64::from_le_bytes(bytes[bbox_off + 16..bbox_off + 24].try_into().unwrap()),
        );
        let world_bbox_max = Vec3d::new(
            f64::from_le_bytes(bytes[bbox_off + 24..bbox_off + 32].try_into().unwrap()),
            f64::from_le_bytes(bytes[bbox_off + 32..bbox_off + 40].try_into().unwrap()),
            f64::from_le_bytes(bytes[bbox_off + 40..bbox_off + 48].try_into().unwrap()),
        );

        let vox_off = bbox_off + 48; // 608
        let voxel_size = Vec3d::new(
            f64::from_le_bytes(bytes[vox_off..vox_off + 8].try_into().unwrap()),
            f64::from_le_bytes(bytes[vox_off + 8..vox_off + 16].try_into().unwrap()),
            f64::from_le_bytes(bytes[vox_off + 16..vox_off + 24].try_into().unwrap()),
        );

        let grid_class = u32::from_le_bytes(bytes[632..636].try_into().unwrap());
        let grid_type = u32::from_le_bytes(bytes[636..640].try_into().unwrap());
        let blind_metadata_offset = i64::from_le_bytes(bytes[640..648].try_into().unwrap());
        let blind_metadata_count = u32::from_le_bytes(bytes[648..652].try_into().unwrap());

        Some(GridDataHeader {
            magic,
            checksum,
            version,
            flags,
            grid_index,
            grid_count,
            grid_size,
            grid_name,
            map,
            world_bbox_min,
            world_bbox_max,
            voxel_size,
            grid_class,
            grid_type,
            blind_metadata_offset,
            blind_metadata_count,
        })
    }
}
