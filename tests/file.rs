use nanovdb_rs::{create_sampler1, NvdbFile, ValidatedFloatTree, Vec3d};

fn fixture(name: &str) -> Option<std::path::PathBuf> {
    let candidates = [
        format!("../pbrt-v4-scenes/bunny-cloud/{name}"),
        format!("../pbrt-v4-scenes/explosion/{name}"),
        format!("../pbrt-v4-scenes/disney-cloud/{name}"),
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
            grid.name(), grid.value_type(), grid.voxel_count(),
            grid.metadata.index_bbox_min, grid.metadata.index_bbox_max,
            grid.metadata.voxel_size.x, grid.metadata.voxel_size.y, grid.metadata.voxel_size.z,
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

#[test]
fn float_read_accessor_bunny_cloud() {
    let Some(path) = fixture("bunny_cloud.nvdb") else {
        eprintln!("bunny_cloud.nvdb not present; skipping");
        return;
    };
    let file = NvdbFile::open(&path).expect("open bunny_cloud");
    let grid = &file.grids()[0];
    let mut accessor = grid.float_read_accessor().expect("float accessor");
    let (bbox_min, bbox_max) = grid.index_bbox();
    let bg = accessor.background();
    let outside = accessor.get_value([bbox_min[0] - 10, bbox_min[1] - 10, bbox_min[2] - 10]);
    assert_eq!(outside, bg);

    let mid = [
        (bbox_min[0] + bbox_max[0]) / 2,
        (bbox_min[1] + bbox_max[1]) / 2,
        (bbox_min[2] + bbox_max[2]) / 2,
    ];
    let mut non_bg = 0;
    for di in -8..=8 {
        for dj in -8..=8 {
            for dk in -8..=8 {
                let v = accessor.get_value([mid[0] + di, mid[1] + dj, mid[2] + dk]);
                if (v - bg).abs() > 1e-6 {
                    non_bg += 1;
                }
            }
        }
    }
    assert!(non_bg > 0, "expected non-background hits near bbox centre");

    let mid_world = grid
        .index_to_world(Vec3d::new(mid[0] as f64, mid[1] as f64, mid[2] as f64))
        .unwrap();
    let mid_idx = grid.world_to_index(mid_world).unwrap();
    for (a, b) in [
        (mid_idx.x, mid[0] as f64),
        (mid_idx.y, mid[1] as f64),
        (mid_idx.z, mid[2] as f64),
    ] {
        assert!((a - b).abs() < 1e-6, "round-trip drift: {a} vs {b}");
    }

    let v_int = accessor.get_value(mid);
    let v_sample =
        create_sampler1(&mut accessor).sample([mid[0] as f64, mid[1] as f64, mid[2] as f64]);
    assert!((v_int - v_sample).abs() <= 1e-5);

    let validated = ValidatedFloatTree::new(grid.raw_bytes()).expect("validated tree");
    for point in [
        [
            mid[0] as f32 + 0.125,
            mid[1] as f32 + 0.25,
            mid[2] as f32 + 0.5,
        ],
        [
            bbox_min[0] as f32 - 0.25,
            bbox_min[1] as f32 + 0.5,
            bbox_max[2] as f32 + 0.75,
        ],
        [
            bbox_max[0] as f32 + 1.25,
            bbox_max[1] as f32 + 0.5,
            bbox_max[2] as f32 + 0.25,
        ],
    ] {
        let mut reference_accessor = grid.float_read_accessor().expect("float accessor");
        let reference = create_sampler1(&mut reference_accessor).sample([
            point[0] as f64,
            point[1] as f64,
            point[2] as f64,
        ]) as f32;
        let direct = validated.sample(point).expect("direct sample");
        assert!(
            (direct - reference).abs() <= 1e-30,
            "sample mismatch at {point:?}"
        );
    }
    assert!(validated.sample([f32::INFINITY, 0.0, 0.0]).is_none());
    assert!(validated.sample([i32::MAX as f32, 0.0, 0.0]).is_none());
}
