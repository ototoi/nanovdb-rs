use std::env;
use std::path::Path;

use nanovdb_rs::{create_sampler1, GridType, NvdbFile, Vec3d};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args_os()
        .nth(1)
        .ok_or("usage: cargo run --example inspect -- <file.nvdb>")?;
    let file = NvdbFile::open(Path::new(&path))?;

    println!("file size: {} bytes", file.file_size());
    println!("grids: {}", file.grids().len());
    for (index, grid) in file.grids().iter().enumerate() {
        let (index_min, index_max) = grid.index_bbox();
        let (world_min, world_max) = grid.world_bbox();
        println!("grid {index}: {}", grid.name());
        println!("  type: {:?}", grid.value_type());
        println!("  voxels: {}", grid.voxel_count());
        println!("  index bbox: {:?}..{:?}", index_min, index_max);
        println!("  world bbox: {:?}..{:?}", world_min, world_max);
        println!("  voxel size: {:?}", grid.voxel_size());

        if grid.value_type() == GridType::Float {
            let world_center = Vec3d::new(
                (world_min.x + world_max.x) * 0.5,
                (world_min.y + world_max.y) * 0.5,
                (world_min.z + world_max.z) * 0.5,
            );
            let index_center = grid
                .world_to_index(world_center)
                .ok_or("missing grid map")?;
            let mut accessor = grid.float_read_accessor().ok_or("not a float grid")?;
            let value = create_sampler1(&mut accessor).sample([
                index_center.x,
                index_center.y,
                index_center.z,
            ]);
            println!("  center sample: {value}");
        }
    }

    Ok(())
}
