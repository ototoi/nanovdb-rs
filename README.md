# nanovdb-rs

[![License](https://img.shields.io/github/license/ototoi/nanovdb-rs)](LICENSE)

`nanovdb-rs` is a standalone, pure-Rust reader for NanoVDB (`.nvdb`)
sparse volumetric grid files. It provides memory-mapped file access,
per-grid metadata, coordinate transforms, and FloatGrid sampling without
depending on a renderer or scene repository.

## Features

- Memory-mapped access to uncompressed grid data
- ZIP (zlib) compressed segments through the default `zip` feature
- Multi-segment and multi-grid files
- Grid metadata: name, type, voxel count, voxel size, bounding boxes, and version
- Raw grid bytes for application-specific tree access
- FloatGrid voxel lookup through `ReadAccessor`
- Trilinear FloatGrid sampling through `create_sampler1`
- World/index coordinate transforms
- Validated zero-copy FloatGrid sampling through `ValidatedFloatTree`

Currently unsupported:

- BLOSC compressed segments
- Full `Double` and `Vec3f` tree accessors

## Installation

Add the crate to your application:

```toml
[dependencies]
nanovdb-rs = "0.0.5"
```

ZIP support is enabled by default. To disable it:

```toml
[dependencies]
nanovdb-rs = { version = "0.0.5", default-features = false }
```

## Library usage

```rust
use nanovdb_rs::{create_sampler1, NvdbFile, Vec3d};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = NvdbFile::open("volume.nvdb")?;

    for grid in file.grids() {
        println!(
            "{}: {:?}, {} voxels",
            grid.name(),
            grid.value_type(),
            grid.voxel_count(),
        );

        if let Some(mut accessor) = grid.float_read_accessor() {
            let index = grid
                .world_to_index(Vec3d::new(0.0, 0.0, 0.0))
                .ok_or("missing grid map")?;
            let value = create_sampler1(&mut accessor).sample([index.x, index.y, index.z]);
            println!("  sample at world origin: {value}");
        }
    }

    Ok(())
}
```

## Inspect example

The repository includes a small example that prints grid metadata and samples
the center of each Float grid:

```bash
cargo run --example inspect -- path/to/volume.nvdb
```

The example accepts any `.nvdb` file and does not require test fixtures or
another renderer.

## Workspace tests

The repository is a Cargo workspace with two packages:

- `nanovdb-rs`: the publishable library crate
- `nanovdb-rs-tests`: non-publishable integration tests for real `.nvdb` files

The default commands target only the library crate:

```bash
cargo test
cargo check --examples
```

To run the real-file tests as well:

```bash
cargo test --workspace
```

The integration fixtures are stored under
`nanovdb-rs-tests/fixtures/` and tracked with Git LFS. A checkout without the
LFS objects skips those tests. An alternate fixture root can be supplied with
`NANOVDB_TEST_FIXTURE_ROOT`.

Only the library crate is published:

```bash
cargo publish -p nanovdb-rs
```

The `nanovdb-rs-tests` package has `publish = false`, so its tests and large
fixtures are not included in the `nanovdb-rs` crates.io package.

## License

Mozilla Public License 2.0. See [`LICENSE`](LICENSE).
