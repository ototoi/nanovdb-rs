use nanovdb_rs::{ReadAccessor, ValidatedFloatTree, ValidatedTree, Vec3f};

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

#[test]
fn reads_a_vec3f_root_tile() {
    let root_abs = 672 + 64;
    let tile = root_abs + 96;
    let mut bytes = vec![0_u8; tile + 32];
    bytes[636..640].copy_from_slice(&6_u32.to_le_bytes());
    bytes[672 + 24..672 + 32].copy_from_slice(&(64_u64).to_le_bytes());
    bytes[root_abs + 24..root_abs + 28].copy_from_slice(&1_u32.to_le_bytes());
    for (offset, value) in [
        (root_abs + 28, 1.0_f32),
        (root_abs + 32, 1.0_f32),
        (root_abs + 36, 1.0_f32),
        (tile + 20, 2.0_f32),
        (tile + 24, 3.0_f32),
        (tile + 28, 4.0_f32),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[tile + 16..tile + 20].copy_from_slice(&1_u32.to_le_bytes());

    let mut accessor = ReadAccessor::<Vec3f>::from_grid_bytes(&bytes).expect("Vec3f accessor");
    assert_eq!(accessor.background(), Vec3f::new(1.0, 1.0, 1.0));
    assert_eq!(accessor.get_value([0, 0, 0]), Vec3f::new(2.0, 3.0, 4.0));
    assert!(accessor.is_active([0, 0, 0]));

    let tree = ValidatedTree::<Vec3f>::new(&bytes).expect("validated Vec3f tree");
    assert_eq!(tree.get_value([0, 0, 0]), Vec3f::new(2.0, 3.0, 4.0));
}
