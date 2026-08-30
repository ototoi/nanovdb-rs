# nanovdb-rs

[![License](https://img.shields.io/github/license/ototoi/nanovdb-rs)](LICENSE)

A small, pure-Rust reader for **NanoVDB** (`.nvdb`) sparse volumetric grid
files — the static runtime form of OpenVDB used by rendering applications and other
modern renderers for fog, fire, cloud, and similar volumetric assets.

The crate provides memory-mapped file access, per-grid metadata, raw grid
bytes, world/index coordinate transforms, and FloatGrid point sampling. It
is designed as a standalone NanoVDB reader for applications that need to
inspect or sample sparse volumetric data.

## Status

- [x] Memory-mapped, zero-copy file reader
- [x] Multi-segment / multi-grid files
- [x] Per-grid metadata: name, value type, voxel size, world / index
      bounding box, voxel count, version
- [x] Raw grid blob handed back so downstream code can do its own tree
      walk
- [x] ZIP (zlib) compressed segments (default `zip` feature, via
      `flate2`)
- [x] In-crate NanoVDB tree traversal and voxel point lookup for `FloatGrid`
      (`ReadAccessor`) with trilinear interpolation + index<->world transform
- [ ] `Vec3f` / `Double` grid accessors
- [ ] BLOSC compressed segments

## Usage

```rust
use nanovdb_rs::{NvdbFile, Vec3d};

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
        // Random-access a float grid via a NanoVDB-style sampler.
        if let Some(mut acc) = grid.float_read_accessor() {
            let idx = grid.world_to_index(Vec3d::new(0.0, 0.0, 0.0)).unwrap();
            println!(
                "  background={} at world (0,0,0) -> idx ({:.3}, {:.3}, {:.3}), value={}",
                acc.background(),
                idx.x, idx.y, idx.z,
                nanovdb_rs::create_sampler1(&mut acc).sample([idx.x, idx.y, idx.z]),
            );
        }
    }
    Ok(())
}
```

## License

Mozilla Public License 2.0, matching upstream OpenVDB/NanoVDB. See
[`LICENSE`](LICENSE).
