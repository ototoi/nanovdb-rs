//! Pure-Rust reader for NanoVDB (`.nvdb`) sparse volumetric grid files.
//!
//! This crate ports just enough of OpenVDB's [`nanovdb` runtime
//! format](https://www.openvdb.org/documentation/doxygen/group__NanoVDB.html)
//! to enumerate the grids in a `.nvdb` file, read their per-grid metadata
//! (name, value type, world / index bounding box, voxel size), and run
//! point lookups and trilinear sampling against `FloatGrid` voxel data.
//!
//! `Vec3f` accessor is currently a stub that recognises the grid type
//! but does not yet walk the tree. `Double` accessor is planned for a
//! follow-up release.
//!
//! ## Quick start
//!
//! ```no_run
//! use nanovdb_rs::{NvdbFile, Vec3d};
//!
//! let file = NvdbFile::open("bunny_cloud.nvdb").unwrap();
//! for grid in file.grids() {
//!     println!(
//!         "{} ({:?}, {:?} voxels)",
//!         grid.name(),
//!         grid.value_type(),
//!         grid.voxel_count()
//!     );
//!     if let Some(mut acc) = grid.float_read_accessor() {
//!         let idx = grid.world_to_index(Vec3d::new(0.0, 0.0, 0.0)).unwrap();
//!         let v = nanovdb_rs::create_sampler1(&mut acc).sample([idx.x, idx.y, idx.z]);
//!         println!("  sample at world (0,0,0): {}", v);
//!     }
//! }
//! ```
//!
//! ## Format support
//!
//! - [x] Single-segment uncompressed files
//! - [x] Multi-segment / multi-grid files (one segment header at the start
//!       of each contiguous block)
//! - [ ] ZIP and BLOSC compression (returns `Error::CompressionUnsupported`)
//!
//! Compression is rare in practice for production .nvdb assets; we'll add
//! ZIP support when a real scene needs it.

#![forbid(unsafe_op_in_unsafe_fn)]

mod error;
mod file;
mod grid_data;
mod header;
mod metadata;
mod sample_from_voxels;
mod tree_f32;
mod tree_vec3f;
mod types;

pub use error::Error;
pub use file::{Grid, NvdbFile};
pub use grid_data::{GridDataHeader, Map, GRID_DATA_SIZE, MAP_SIZE};
pub use header::{Codec, SegmentHeader, Version};
pub use metadata::GridMetadata;
pub use sample_from_voxels::{create_sampler1, SampleFromVoxels};
pub use tree_f32::{ReadAccessor, TreeData};
pub use tree_vec3f::Vec3f;
pub use types::{GridClass, GridType, Vec3d};
