//! Pure-Rust reader for NanoVDB (`.nvdb`) sparse volumetric grid files.
//!
//! This crate ports just enough of OpenVDB's [`nanovdb` runtime
//! format](https://www.openvdb.org/documentation/doxygen/group__NanoVDB.html)
//! to enumerate the grids in a `.nvdb` file, read their per-grid metadata
//! (name, value type, world / index bounding box, voxel size), and hand
//! back the raw grid bytes for downstream code that wants to do its own
//! tree traversal.
//!
//! Voxel-level point lookup (`Float` / `Vec3f` random access through the
//! NanoVDB tree) is planned for a follow-up release.
//!
//! ## Quick start
//!
//! ```no_run
//! use nanovdb_rs::NvdbFile;
//!
//! let file = NvdbFile::open("bunny_cloud.nvdb").unwrap();
//! for grid in file.grids() {
//!     println!(
//!         "{} ({:?}, {:?} voxels)",
//!         grid.name(),
//!         grid.value_type(),
//!         grid.voxel_count()
//!     );
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
mod header;
mod metadata;
mod types;

pub use error::Error;
pub use file::{Grid, NvdbFile};
pub use header::{Codec, SegmentHeader, Version};
pub use metadata::GridMetadata;
pub use types::{GridClass, GridType, Vec3d};
