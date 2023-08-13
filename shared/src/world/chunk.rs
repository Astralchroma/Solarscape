use nalgebra::Vector3;

pub fn index_of_vec(cell_position: Vector3<u8>) -> usize {
	let x = cell_position.x as usize;
	let y = cell_position.y as usize;
	let z = cell_position.z as usize;

	index_of(x, y, z)
}

pub fn index_of(x: usize, y: usize, z: usize) -> usize {
	assert!(x <= 0xf);
	assert!(y <= 0xf);
	assert!(z <= 0xf);

	(x << 8) + (y << 4) + z
}
