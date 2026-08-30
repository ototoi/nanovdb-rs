use nanovdb_rs::NvdbFile;

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
