use nanovdb_rs::NvdbFile;

fn fixture_root() -> Option<std::path::PathBuf> {
    std::env::var_os("NANOVDB_TEST_FIXTURE_ROOT").map(std::path::PathBuf::from)
}

fn fixture(name: &str) -> Option<std::path::PathBuf> {
    let root = fixture_root()?;
    [
        root.join("bunny-cloud").join(name),
        root.join("explosion").join(name),
        root.join("disney-cloud").join(name),
    ]
    .into_iter()
    .find(|path| path.exists())
}

#[test]
fn open_bunny_cloud() {
    let Some(path) = fixture("bunny_cloud.nvdb") else {
        eprintln!("set NANOVDB_TEST_FIXTURE_ROOT to run scene-file tests; skipping");
        return;
    };
    let file = NvdbFile::open(&path).expect("open bunny_cloud");
    assert!(!file.grids().is_empty(), "expected at least one grid");
    for grid in file.grids() {
        assert_eq!(grid.raw_bytes().len() as u64, grid.metadata.grid_size);
    }
}

#[test]
fn open_fire() {
    let Some(path) = fixture("fire.nvdb") else {
        eprintln!("set NANOVDB_TEST_FIXTURE_ROOT to run scene-file tests; skipping");
        return;
    };
    let file = NvdbFile::open(&path).expect("open fire");
    assert!(!file.grids().is_empty());
}
