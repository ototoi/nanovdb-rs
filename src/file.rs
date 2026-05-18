//! Top-level `.nvdb` file reader: mmap the file once, walk through each
//! segment header, parse the per-grid metadata for that segment, and
//! present a `Grid` view onto the grid bytes that follow.
//!
//! Uncompressed segments expose their bytes zero-copy from the mmap;
//! ZIP segments are decompressed into per-grid `Arc<Vec<u8>>` buffers
//! (only on first access; the mmap is still used for the metadata).

use std::path::Path;
use std::sync::Arc;

use memmap2::Mmap;

use crate::error::Error;
use crate::header::{Codec, SegmentHeader};
use crate::metadata::GridMetadata;

/// A read-only memory-mapped view of an entire `.nvdb` file plus the
/// parsed per-grid metadata.
pub struct NvdbFile {
    mmap: Arc<Mmap>,
    grids: Vec<Grid>,
}

/// A single grid inside a `.nvdb` file. Either references the mmap
/// directly (uncompressed segments) or owns a decompressed copy of the
/// grid bytes (ZIP segments).
pub struct Grid {
    /// Parsed `GridMetadata` for this grid.
    pub metadata: GridMetadata,
    pub(crate) bytes: GridBytes,
}

pub(crate) enum GridBytes {
    /// The grid lives in the mmap at [offset, offset + grid_size).
    Mmap {
        mmap: Arc<Mmap>,
        offset: u64,
        len: u64,
    },
    /// The grid was decompressed at load time into this buffer.
    Owned(Arc<Vec<u8>>),
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
            let mut metadata_list: Vec<GridMetadata> = Vec::with_capacity(header.grid_count as usize);
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
                            let compressed = &bytes[comp_start..comp_end];
                            let mut decoder = flate2::read::ZlibDecoder::new(compressed);
                            let mut out = Vec::with_capacity(meta.grid_size as usize);
                            std::io::copy(&mut decoder, &mut out)?;
                            cursor += compressed_size;
                            GridBytes::Owned(Arc::new(out))
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
        Ok(NvdbFile { mmap, grids })
    }

    /// All grids found in the file, in document order.
    pub fn grids(&self) -> &[Grid] {
        &self.grids
    }

    /// Total file size in bytes (length of the underlying mmap).
    pub fn file_size(&self) -> usize {
        self.mmap.len()
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
    /// on the receiving side once the in-memory tree walker lands.
    pub fn raw_bytes(&self) -> &[u8] {
        match &self.bytes {
            GridBytes::Mmap { mmap, offset, len } => {
                let start = *offset as usize;
                let end = start + *len as usize;
                &mmap[start..end]
            }
            GridBytes::Owned(buf) => buf.as_slice(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Option<std::path::PathBuf> {
        let candidates = [
            format!("../pbrt-v4-scenes/bunny-cloud/{}", name),
            format!("../pbrt-v4-scenes/explosion/{}", name),
            format!("../pbrt-v4-scenes/disney-cloud/{}", name),
        ];
        candidates
            .into_iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.exists())
    }

    #[test]
    fn open_bunny_cloud() {
        let Some(path) = fixture("bunny_cloud.nvdb") else {
            eprintln!("bunny_cloud.nvdb not present; skipping");
            return;
        };
        let file = NvdbFile::open(&path).expect("open bunny_cloud");
        assert!(!file.grids().is_empty(), "expected at least one grid");
        for grid in file.grids() {
            eprintln!(
                "grid: name={:?} type={:?} voxels={} bbox_index=({:?}..{:?}) voxel_size=({:?},{:?},{:?})",
                grid.name(),
                grid.value_type(),
                grid.voxel_count(),
                grid.metadata.index_bbox_min,
                grid.metadata.index_bbox_max,
                grid.metadata.voxel_size.x,
                grid.metadata.voxel_size.y,
                grid.metadata.voxel_size.z,
            );
            assert_eq!(grid.raw_bytes().len() as u64, grid.metadata.grid_size);
        }
    }

    #[test]
    fn open_fire_nvdb() {
        let Some(path) = fixture("fire.nvdb") else {
            return;
        };
        let file = NvdbFile::open(&path).expect("open fire");
        assert!(!file.grids().is_empty());
    }
}
