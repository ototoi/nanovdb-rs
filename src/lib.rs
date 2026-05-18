//! Pure-Rust reader for NanoVDB (`.nvdb`) sparse volumetric grid files.
//!
//! This crate ports just enough of OpenVDB's [`nanovdb` runtime
//! format](https://www.openvdb.org/documentation/doxygen/group__NanoVDB.html)
//! to enumerate the grids in a `.nvdb` file, read their per-grid metadata
//! (name, value type, world / index bounding box, voxel size), and run
//! point lookups against `FloatGrid` voxel data with either nearest or
//! trilinear interpolation.
//!
//! `Vec3f` / `Double` accessors are planned for a follow-up release.
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
//!     if let Some(acc) = grid.float_accessor() {
//!         let idx = grid.world_to_index(Vec3d::new(0.0, 0.0, 0.0)).unwrap();
//!         let v = acc.sample_trilinear([idx.x, idx.y, idx.z]);
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
mod tree_f32;
mod types;

pub use error::Error;
pub use file::{Grid, NvdbFile};
pub use grid_data::{GridDataHeader, Map, GRID_DATA_SIZE, MAP_SIZE};
pub use header::{Codec, SegmentHeader, Version};
pub use metadata::GridMetadata;
pub use tree_f32::FloatAccessor;
pub use types::{GridClass, GridType, Vec3d};
