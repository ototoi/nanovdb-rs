//! Top-level `.nvdb` file reader: mmap the file once, walk through each
//! segment header, parse the per-grid metadata for that segment, and
//! present a `Grid` view onto the grid bytes that follow.
//!
//! Uncompressed segments expose their bytes zero-copy from the input mmap.
//! ZIP segments are streamed into anonymous temporary files and memory
//! mapped, avoiding a retained heap-sized decompression buffer.

use std::path::Path;
use std::sync::Arc;

use memmap2::Mmap;

use crate::error::Error;
use crate::header::{Codec, SegmentHeader};
use crate::metadata::GridMetadata;

/// A read-only memory-mapped view of an entire `.nvdb` file plus the
/// parsed per-grid metadata.
pub struct NvdbFile {
    file_size: usize,
    grids: Vec<Grid>,
}

/// A single grid inside a `.nvdb` file. Either references the mmap
/// directly (uncompressed segments) or a mmap of a decompressed ZIP
/// segment.
pub struct Grid {
    /// Parsed `GridMetadata` for this grid.
    pub metadata: GridMetadata,
    pub(crate) bytes: GridBytes,
}

pub(crate) enum GridBytes {
    /// The grid lives in the mmap at [offset, offset + len).
    Mmap {
        mmap: Arc<Mmap>,
        offset: u64,
        len: u64,
    },
}

impl NvdbFile {
    /// Memory-map the given `.nvdb` file, parse all segment headers,
    /// grid metadata, and (for ZIP segments) decompress each grid into
    /// an owned buffer.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let file = std::fs::File::open(path)?;
        // SAFETY: the mapping is read-only and outlives all `&[u8]`
        // returned by `Grid::raw_bytes`. NanoVDB files are assumed not
        // to be mutated by other processes for the lifetime of this
        // `NvdbFile`.
        let mmap = unsafe { Mmap::map(&file) }?;
        Self::from_mmap(Arc::new(mmap))
    }

    fn from_mmap(mmap: Arc<Mmap>) -> Result<Self, Error> {
        let bytes: &[u8] = &mmap;
        let mut grids: Vec<Grid> = Vec::new();
        let mut cursor: u64 = 0;
        let total: u64 = bytes.len() as u64;
        while cursor < total {
            let head_start = cursor as usize;
            let head_end = head_start + SegmentHeader::BYTE_SIZE;
            if head_end > bytes.len() {
                return Err(Error::Truncated {
                    offset: cursor,
                    wanted: SegmentHeader::BYTE_SIZE,
                    actual: bytes.len() - head_start,
                });
            }
            let header = SegmentHeader::parse(&bytes[head_start..head_end], cursor)?;
            cursor += SegmentHeader::BYTE_SIZE as u64;

            // Parse per-grid metadata for the segment.
            let mut metadata_list: Vec<GridMetadata> =
                Vec::with_capacity(header.grid_count as usize);
            for _ in 0..header.grid_count {
                let meta_start = cursor as usize;
                let (meta, consumed) = GridMetadata::parse(&bytes[meta_start..])?;
                metadata_list.push(meta);
                cursor += consumed as u64;
            }

            for meta in metadata_list.into_iter() {
                let grid_bytes = match header.codec {
                    Codec::None => {
                        let grid_offset = cursor;
                        cursor += meta.grid_size;
                        GridBytes::Mmap {
                            mmap: Arc::clone(&mmap),
                            offset: grid_offset,
                            len: meta.grid_size,
                        }
                    }
                    Codec::Zip => {
                        // NanoVDB ZIP layout (util/IO.h:317-328): [u64 size]
                        // followed by `size` bytes of zlib-compressed
                        // data that decompresses to `meta.grid_size`.
                        #[cfg(feature = "zip")]
                        {
                            let size_off = cursor as usize;
                            if size_off + 8 > bytes.len() {
                                return Err(Error::Truncated {
                                    offset: cursor,
                                    wanted: 8,
                                    actual: bytes.len() - size_off,
                                });
                            }
                            let compressed_size = u64::from_le_bytes(
                                bytes[size_off..size_off + 8].try_into().unwrap(),
                            );
                            cursor += 8;
                            let comp_start = cursor as usize;
                            let comp_end = comp_start + compressed_size as usize;
                            if comp_end > bytes.len() {
                                return Err(Error::Truncated {
                                    offset: cursor,
                                    wanted: compressed_size as usize,
                                    actual: bytes.len() - comp_start,
                                });
                            }
                            let grid_mmap = decompress_zip_grid_to_mmap(
                                &bytes[comp_start..comp_end],
                                meta.grid_size,
                            )?;
                            cursor += compressed_size;
                            GridBytes::Mmap {
                                mmap: Arc::new(grid_mmap),
                                offset: 0,
                                len: meta.grid_size,
                            }
                        }
                        #[cfg(not(feature = "zip"))]
                        {
                            return Err(Error::CompressionUnsupported(Codec::Zip));
                        }
                    }
                    Codec::Blosc | Codec::Other => {
                        return Err(Error::CompressionUnsupported(header.codec));
                    }
                };
                grids.push(Grid {
                    metadata: meta,
                    bytes: grid_bytes,
                });
            }
        }
        Ok(NvdbFile {
            file_size: total as usize,
            grids,
        })
    }

    /// All grids found in the file, in document order.
    pub fn grids(&self) -> &[Grid] {
        &self.grids
    }

    /// Total file size in bytes.
    pub fn file_size(&self) -> usize {
        self.file_size
    }
}

impl Grid {
    /// Grid name (zero-terminated string stored after the metadata).
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    pub fn value_type(&self) -> crate::types::GridType {
        self.metadata.grid_type
    }

    /// Parsed per-grid metadata. Mirrors NanoVDB `GridHandle::gridMetaData()`.
    pub fn grid_metadata(&self) -> &GridMetadata {
        &self.metadata
    }

    pub fn voxel_count(&self) -> u64 {
        self.metadata.voxel_count
    }

    pub fn index_bbox(&self) -> ([i32; 3], [i32; 3]) {
        (self.metadata.index_bbox_min, self.metadata.index_bbox_max)
    }

    pub fn world_bbox(&self) -> (crate::types::Vec3d, crate::types::Vec3d) {
        (self.metadata.world_bbox_min, self.metadata.world_bbox_max)
    }

    pub fn voxel_size(&self) -> crate::types::Vec3d {
        self.metadata.voxel_size
    }

    /// Raw grid bytes (`metadata.grid_size` long; decompressed for ZIP).
    /// Callers can reinterpret these as a `nanovdb::NanoGrid<T>` struct
    /// or use the higher-level `float_read_accessor` / `world_to_index` /
    /// `get_value` helpers on this `Grid`.
    pub fn raw_bytes(&self) -> &[u8] {
        match &self.bytes {
            GridBytes::Mmap { mmap, offset, len } => {
                let start = *offset as usize;
                let end = start + *len as usize;
                &mmap[start..end]
            }
        }
    }

    /// Parse the in-memory `GridData` header that begins at offset 0 of
    /// `raw_bytes()`. Cheap (just byte unpacking, no allocation beyond
    /// the grid name).
    pub fn header(&self) -> Option<crate::grid_data::GridDataHeader> {
        crate::grid_data::GridDataHeader::parse(self.raw_bytes())
    }

    /// Random-access accessor for `Float` grids. Returns `None` for
    /// non-float grid types; callers can match on `value_type()` first.
    pub fn float_read_accessor(&self) -> Option<crate::tree_f32::ReadAccessor<'_>> {
        crate::tree_f32::ReadAccessor::from_grid_bytes(self.raw_bytes())
    }

    /// World-space point -> index-space (voxel) coordinate via the
    /// grid's stored affine transform. Mirrors v4's
    /// `Grid::worldToIndex(p)`.
    pub fn world_to_index(&self, world: crate::types::Vec3d) -> Option<crate::types::Vec3d> {
        Some(self.header()?.map.apply_inverse_map(world))
    }

    /// Index-space (voxel) coordinate -> world-space point.
    pub fn index_to_world(&self, idx: crate::types::Vec3d) -> Option<crate::types::Vec3d> {
        Some(self.header()?.map.apply_map(idx))
    }
}

#[cfg(feature = "zip")]
fn decompress_zip_grid_to_mmap(compressed: &[u8], expected_size: u64) -> Result<Mmap, Error> {
    let mut decoder = flate2::read::ZlibDecoder::new(compressed);
    let mut file = tempfile::tempfile()?;
    let actual = std::io::copy(&mut decoder, &mut file)?;
    if actual != expected_size {
        return Err(Error::BadDecompressedSize {
            expected: expected_size,
            actual,
        });
    }
    // SAFETY: the mapping is read-only. The underlying anonymous temp
    // file is no longer needed after the mapping is established.
    Ok(unsafe { Mmap::map(&file) }?)
}
