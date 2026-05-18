# nanovdb-rs

[![License](https://img.shields.io/github/license/ototoi/nanovdb-rs)](LICENSE)

A small, pure-Rust reader for **NanoVDB** (`.nvdb`) sparse volumetric grid
files — the static runtime form of OpenVDB used by pbrt-v4 and other
modern renderers for fog, fire, cloud, and similar volumetric assets.

This crate exists primarily to feed
[pbrt-r4](https://github.com/ototoi/pbrt-r4)'s `GridMedium`. It's the
read half of [OpenVDB's NanoVDB I/O
format](https://www.openvdb.org/documentation/doxygen/group__NanoVDB.html);
voxel-level tree traversal is a follow-up release.

## Status

- [x] Memory-mapped, zero-copy file reader
- [x] Multi-segment / multi-grid files
- [x] Per-grid metadata: name, value type, voxel size, world / index
      bounding box, voxel count, version
- [x] Raw grid blob handed back so downstream code can do its own tree
      walk
- [x] ZIP (zlib) compressed segments (default `zip` feature, via
      `flate2`)
- [ ] In-crate voxel point lookup (Float / Vec3f random access)
- [ ] BLOSC compressed segments

## Usage

```rust
use nanovdb_rs::NvdbFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = NvdbFile::open("bunny_cloud.nvdb")?;
    for grid in file.grids() {
        println!(
            "{} ({:?}, {} voxels, bbox {:?}..{:?})",
            grid.name(),
            grid.value_type(),
            grid.voxel_count(),
            grid.metadata.index_bbox_min,
            grid.metadata.index_bbox_max,
        );
    }
    Ok(())
}
```

## License

Apache 2.0, matching upstream OpenVDB/NanoVDB. See [`LICENSE`](LICENSE).
