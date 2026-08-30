use nanovdb_rs::ValidatedFloatTree;

const GRID_DATA_SIZE: usize = 672;
const ROOT_OFFSET: usize = 64;
const ROOT_ABS: usize = GRID_DATA_SIZE + ROOT_OFFSET;
const ROOT_HEADER_SIZE: usize = 64;
const ROOT_TILE_SIZE: usize = 32;

fn root_tile_grid(background: f32, value: f32) -> Vec<u8> {
    let mut bytes = vec![0_u8; ROOT_ABS + ROOT_HEADER_SIZE + ROOT_TILE_SIZE];
    bytes[636..640].copy_from_slice(&1_u32.to_le_bytes());
    bytes[672 + 24..672 + 32].copy_from_slice(&(ROOT_OFFSET as u64).to_le_bytes());

    bytes[ROOT_ABS + 24..ROOT_ABS + 28].copy_from_slice(&1_u32.to_le_bytes());
    bytes[ROOT_ABS + 28..ROOT_ABS + 32].copy_from_slice(&background.to_le_bytes());
    let tile = ROOT_ABS + ROOT_HEADER_SIZE;
    bytes[tile..tile + 8].copy_from_slice(&0_u64.to_le_bytes());
    bytes[tile + 16..tile + 20].copy_from_slice(&1_u32.to_le_bytes());
    bytes[tile + 20..tile + 24].copy_from_slice(&value.to_le_bytes());
    bytes
}

#[test]
fn samples_a_valid_root_tile_without_external_assets() {
    let bytes = root_tile_grid(0.25, 2.5);
    let tree = ValidatedFloatTree::new(&bytes).expect("valid synthetic float tree");

    assert_eq!(tree.sample([0.0, 0.0, 0.0]), Some(2.5));
    assert_eq!(tree.sample([0.75, 0.25, 0.5]), Some(2.5));
}

#[test]
fn rejects_non_float_and_non_finite_inputs() {
    let mut bytes = root_tile_grid(0.0, 1.0);
    bytes[636..640].copy_from_slice(&2_u32.to_le_bytes());
    assert!(ValidatedFloatTree::new(&bytes).is_none());

    let bytes = root_tile_grid(0.0, 1.0);
    let tree = ValidatedFloatTree::new(&bytes).expect("valid synthetic float tree");
    assert!(tree.sample([f32::INFINITY, 0.0, 0.0]).is_none());
    assert!(tree.sample([i32::MAX as f32, 0.0, 0.0]).is_none());
}
