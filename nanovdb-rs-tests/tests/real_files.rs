use nanovdb_rs::{create_sampler1, NvdbFile, Vec3d};

fn fixture_root() -> std::path::PathBuf {
    std::env::var_os("NANOVDB_TEST_FIXTURE_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures"))
}

fn fixture(directory: &str, name: &str) -> std::path::PathBuf {
    let path = fixture_root().join(directory).join(name);
    let valid_fixture = path
        .metadata()
        .map(|metadata| metadata.len() > 1024)
        .unwrap_or(false);
    assert!(
        valid_fixture,
        "missing LFS fixture {path:?}; run `git lfs pull` or set NANOVDB_TEST_FIXTURE_ROOT"
    );
    path
}

#[test]
fn open_bunny_cloud() {
    let path = fixture("bunny-cloud", "bunny_cloud.nvdb");
    let file = NvdbFile::open(&path).expect("open bunny_cloud");
    assert!(!file.grids().is_empty(), "expected at least one grid");
    for grid in file.grids() {
        assert_eq!(grid.raw_bytes().len() as u64, grid.metadata.grid_size);
    }
}

#[test]
fn open_fire() {
    let path = fixture("explosion", "fire.nvdb");
    let file = NvdbFile::open(&path).expect("open fire");
    assert!(!file.grids().is_empty());
}

#[test]
fn open_disney_cloud() {
    let path = fixture("disney-cloud", "wdas_cloud_quarter.nvdb");
    let file = NvdbFile::open(&path).expect("open disney cloud");
    assert!(!file.grids().is_empty());
}

#[test]
fn sample_bunny_cloud() {
    let path = fixture("bunny-cloud", "bunny_cloud.nvdb");
    let file = NvdbFile::open(&path).expect("open bunny_cloud");
    let grid = &file.grids()[0];
    let mut accessor = grid.float_read_accessor().expect("float accessor");
    let (bbox_min, bbox_max) = grid.index_bbox();
    let background = accessor.background();

    let outside = accessor.get_value([bbox_min[0] - 10, bbox_min[1] - 10, bbox_min[2] - 10]);
    assert_eq!(outside, background);

    let mid = [
        (bbox_min[0] + bbox_max[0]) / 2,
        (bbox_min[1] + bbox_max[1]) / 2,
        (bbox_min[2] + bbox_max[2]) / 2,
    ];
    let mut non_background = 0;
    for di in -8..=8 {
        for dj in -8..=8 {
            for dk in -8..=8 {
                let value = accessor.get_value([mid[0] + di, mid[1] + dj, mid[2] + dk]);
                if (value - background).abs() > 1e-6 {
                    non_background += 1;
                }
            }
        }
    }
    assert!(
        non_background > 0,
        "expected at least one non-background voxel near bbox centre"
    );

    let mid_world = grid
        .index_to_world(Vec3d::new(mid[0] as f64, mid[1] as f64, mid[2] as f64))
        .expect("index-to-world transform");
    let mid_index = grid
        .world_to_index(mid_world)
        .expect("world-to-index transform");
    for (actual, expected) in [
        (mid_index.x, mid[0] as f64),
        (mid_index.y, mid[1] as f64),
        (mid_index.z, mid[2] as f64),
    ] {
        assert!(
            (actual - expected).abs() < 1e-6,
            "round-trip drift: {actual} vs {expected}"
        );
    }

    let integer_value = accessor.get_value(mid);
    let sampled_value =
        create_sampler1(&mut accessor).sample([mid[0] as f64, mid[1] as f64, mid[2] as f64]);
    assert!(
        (integer_value - sampled_value).abs() <= 1e-5,
        "sample at ({}, {}, {})={} vs get_value={}",
        mid[0],
        mid[1],
        mid[2],
        sampled_value,
        integer_value
    );
}
