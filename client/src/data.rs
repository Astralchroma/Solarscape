use nalgebra::Point3;

pub struct EdgeData {
	pub count: u8,
	pub edge_indices: [u8; 15],
}

#[rustfmt::skip]
pub const CELL_EDGE_MAP: [EdgeData; 256] = [
	EdgeData { count:  0, edge_indices: [ 0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 3,  8,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 9,  1,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 3,  8,  1,  1,  8,  9,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [10,  2,  1,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 3,  8,  0, 10,  2,  1,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [10,  2,  9,  9,  2,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 3,  8,  2,  8, 10,  2,  8,  9, 10,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 2, 11,  3,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 2, 11,  0,  0, 11,  8,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 0,  9,  1, 11,  3,  2,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 2, 11,  1, 11,  9,  1, 11,  8,  9,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 1, 10,  3,  3, 10, 11,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 1, 10,  0, 10,  8,  0, 10, 11,  8,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 0,  9,  3,  9, 11,  3,  9, 10, 11,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [10,  8,  9, 11,  8, 10,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 8,  7,  4,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 0,  3,  4,  4,  3,  7,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 9,  1,  0,  7,  4,  8,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 9,  1,  4,  1,  7,  4,  1,  3,  7,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [10,  2,  1,  7,  4,  8,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 7,  4,  3,  4,  0,  3, 10,  2,  1,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [10,  2,  9,  2,  0,  9,  7,  4,  8,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 9, 10,  2,  7,  9,  2,  3,  7,  2,  4,  9,  7,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 7,  4,  8,  2, 11,  3,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 7,  4, 11,  4,  2, 11,  4,  0,  2,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 1,  0,  9,  7,  4,  8, 11,  3,  2,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [11,  7,  4, 11,  4,  9,  2, 11,  9,  1,  2,  9,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 1, 10,  3, 10, 11,  3,  4,  8,  7,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [10, 11,  1, 11,  4,  1,  4,  0,  1,  4, 11,  7,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 8,  7,  4, 11,  0,  9, 10, 11,  9,  3,  0, 11,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [11,  7,  4,  9, 11,  4, 10, 11,  9,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 4,  5,  9,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 4,  5,  9,  3,  8,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 4,  5,  0,  0,  5,  1,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 4,  5,  8,  5,  3,  8,  5,  1,  3,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [10,  2,  1,  4,  5,  9,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 8,  0,  3, 10,  2,  1,  5,  9,  4,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [10,  2,  5,  2,  4,  5,  2,  0,  4,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 5, 10,  2,  5,  2,  3,  4,  5,  3,  8,  4,  3,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 4,  5,  9, 11,  3,  2,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 2, 11,  0, 11,  8,  0,  5,  9,  4,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 4,  5,  0,  5,  1,  0, 11,  3,  2,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 5,  1,  2,  8,  5,  2, 11,  8,  2,  5,  8,  4,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [11,  3, 10,  3,  1, 10,  4,  5,  9,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 5,  9,  4,  1,  8,  0,  1, 10,  8, 10, 11,  8,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 0,  4,  5, 11,  0,  5, 10, 11,  5,  3,  0, 11,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 8,  4,  5, 10,  8,  5, 11,  8, 10,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 8,  7,  9,  9,  7,  5,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 0,  3,  9,  3,  5,  9,  3,  7,  5,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 8,  7,  0,  7,  1,  0,  7,  5,  1,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 3,  5,  1,  7,  5,  3,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 8,  7,  9,  7,  5,  9,  2,  1, 10,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 2,  1, 10,  0,  5,  9,  0,  3,  5,  3,  7,  5,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 2,  0,  8,  5,  2,  8,  7,  5,  8,  2,  5, 10,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 5, 10,  2,  3,  5,  2,  7,  5,  3,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 5,  9,  7,  9,  8,  7,  2, 11,  3,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 7,  5,  9,  2,  7,  9,  0,  2,  9, 11,  7,  2,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [11,  3,  2,  8,  1,  0,  8,  7,  1,  7,  5,  1,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 1,  2, 11,  7,  1, 11,  5,  1,  7,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 8,  5,  9,  7,  5,  8,  3,  1, 10, 11,  3, 10,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 0,  7,  5,  9,  0,  5,  0, 11,  7, 10,  0,  1,  0, 10, 11] },
	EdgeData { count: 15, edge_indices: [ 0, 10, 11,  3,  0, 11,  0,  5, 10,  7,  0,  8,  0,  7,  5] },
	EdgeData { count:  6, edge_indices: [ 5, 10, 11,  5, 11,  7,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 5,  6, 10,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 3,  8,  0,  6, 10,  5,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 1,  0,  9,  6, 10,  5,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 3,  8,  1,  8,  9,  1,  6, 10,  5,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 5,  6,  1,  1,  6,  2,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 5,  6,  1,  6,  2,  1,  8,  0,  3,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 5,  6,  9,  6,  0,  9,  6,  2,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 8,  9,  5,  2,  8,  5,  6,  2,  5,  8,  2,  3,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [11,  3,  2,  5,  6, 10,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 8,  0, 11,  0,  2, 11,  5,  6, 10,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 9,  1,  0, 11,  3,  2,  6, 10,  5,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 6, 10,  5,  2,  9,  1,  2, 11,  9, 11,  8,  9,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [11,  3,  6,  3,  5,  6,  3,  1,  5,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [11,  8,  0,  5, 11,  0,  1,  5,  0,  6, 11,  5,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 6, 11,  3,  6,  3,  0,  5,  6,  0,  9,  5,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 9,  5,  6, 11,  9,  6,  8,  9, 11,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 6, 10,  5,  8,  7,  4,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 0,  3,  4,  3,  7,  4, 10,  5,  6,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 0,  9,  1,  6, 10,  5,  7,  4,  8,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 5,  6, 10,  7,  9,  1,  3,  7,  1,  4,  9,  7,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 2,  1,  6,  1,  5,  6,  8,  7,  4,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 5,  2,  1,  6,  2,  5,  4,  0,  3,  7,  4,  3,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 7,  4,  8,  5,  0,  9,  5,  6,  0,  6,  2,  0,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 9,  3,  7,  4,  9,  7,  9,  2,  3,  6,  9,  5,  9,  6,  2] },
	EdgeData { count:  9, edge_indices: [ 2, 11,  3,  4,  8,  7,  5,  6, 10,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 6, 10,  5,  2,  7,  4,  0,  2,  4, 11,  7,  2,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 9,  1,  0,  8,  7,  4, 11,  3,  2,  6, 10,  5,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 1,  2,  9,  2, 11,  9, 11,  4,  9,  4, 11,  7,  6, 10,  5] },
	EdgeData { count: 12, edge_indices: [ 7,  4,  8,  5, 11,  3,  1,  5,  3,  6, 11,  5,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [11,  1,  5,  6, 11,  5, 11,  0,  1,  4, 11,  7, 11,  4,  0] },
	EdgeData { count: 15, edge_indices: [ 9,  5,  0,  5,  6,  0,  6,  3,  0,  3,  6, 11,  7,  4,  8] },
	EdgeData { count: 12, edge_indices: [ 9,  5,  6, 11,  9,  6,  9,  7,  4,  9, 11,  7,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 9,  4, 10, 10,  4,  6,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 6, 10,  4, 10,  9,  4,  3,  8,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 1,  0, 10,  0,  6, 10,  0,  4,  6,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 1,  3,  8,  6,  1,  8,  4,  6,  8, 10,  1,  6,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 9,  4,  1,  4,  2,  1,  4,  6,  2,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 8,  0,  3,  9,  2,  1,  9,  4,  2,  4,  6,  2,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 4,  2,  0,  6,  2,  4,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 2,  3,  8,  4,  2,  8,  6,  2,  4,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 9,  4, 10,  4,  6, 10,  3,  2, 11,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 2,  8,  0, 11,  8,  2, 10,  9,  4,  6, 10,  4,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 2, 11,  3,  6,  1,  0,  4,  6,  0, 10,  1,  6,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 1,  4,  6, 10,  1,  6,  1,  8,  4, 11,  1,  2,  1, 11,  8] },
	EdgeData { count: 12, edge_indices: [ 4,  6,  9,  6,  3,  9,  3,  1,  9,  3,  6, 11,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 1, 11,  8,  0,  1,  8,  1,  6, 11,  4,  1,  9,  1,  4,  6] },
	EdgeData { count:  9, edge_indices: [ 6, 11,  3,  0,  6,  3,  4,  6,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 8,  4,  6,  8,  6, 11,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 6, 10,  7, 10,  8,  7, 10,  9,  8,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 3,  7,  0,  7, 10,  0, 10,  9,  0, 10,  7,  6,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 7,  6, 10,  7, 10,  1,  8,  7,  1,  0,  8,  1,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 7,  6, 10,  1,  7, 10,  3,  7,  1,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 6,  2,  1,  8,  6,  1,  9,  8,  1,  7,  6,  8,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 9,  6,  2,  1,  9,  2,  9,  7,  6,  3,  9,  0,  9,  3,  7] },
	EdgeData { count:  9, edge_indices: [ 0,  8,  7,  6,  0,  7,  2,  0,  6,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 2,  3,  7,  2,  7,  6,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [11,  3,  2,  8,  6, 10,  9,  8, 10,  7,  6,  8,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 7,  0,  2, 11,  7,  2,  7,  9,  0, 10,  7,  6,  7, 10,  9] },
	EdgeData { count: 15, edge_indices: [ 0,  8,  1,  8,  7,  1,  7, 10,  1, 10,  7,  6, 11,  3,  2] },
	EdgeData { count: 12, edge_indices: [ 1,  2, 11,  7,  1, 11,  1,  6, 10,  1,  7,  6,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 6,  9,  8,  7,  6,  8,  6,  1,  9,  3,  6, 11,  6,  3,  1] },
	EdgeData { count:  6, edge_indices: [ 1,  9,  0,  7,  6, 11,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 0,  8,  7,  6,  0,  7,  0, 11,  3,  0,  6, 11,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 6, 11,  7,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [11,  6,  7,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 8,  0,  3,  6,  7, 11,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 9,  1,  0,  6,  7, 11,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 9,  1,  8,  1,  3,  8,  6,  7, 11,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 2,  1, 10,  7, 11,  6,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [10,  2,  1,  8,  0,  3,  7, 11,  6,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 0,  9,  2,  9, 10,  2,  7, 11,  6,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 7, 11,  6,  3, 10,  2,  3,  8, 10,  8,  9, 10,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 3,  2,  7,  7,  2,  6,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 8,  0,  7,  0,  6,  7,  0,  2,  6,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 6,  7,  2,  7,  3,  2,  9,  1,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 2,  6,  1,  6,  8,  1,  8,  9,  1,  6,  7,  8,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 6,  7, 10,  7,  1, 10,  7,  3,  1,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 6,  7, 10, 10,  7,  1,  7,  8,  1,  8,  0,  1,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 7,  3,  0, 10,  7,  0,  9, 10,  0,  7, 10,  6,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [10,  6,  7,  8, 10,  7,  9, 10,  8,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 4,  8,  6,  6,  8, 11,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [11,  6,  3,  6,  0,  3,  6,  4,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [11,  6,  8,  6,  4,  8,  1,  0,  9,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 6,  4,  9,  3,  6,  9,  1,  3,  9,  6,  3, 11,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 4,  8,  6,  8, 11,  6,  1, 10,  2,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [10,  2,  1, 11,  0,  3, 11,  6,  0,  6,  4,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 8, 11,  4, 11,  6,  4,  9,  2,  0,  9, 10,  2,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 3,  9, 10,  2,  3, 10,  3,  4,  9,  6,  3, 11,  3,  6,  4] },
	EdgeData { count:  9, edge_indices: [ 3,  2,  8,  2,  4,  8,  2,  6,  4,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 2,  4,  0,  2,  6,  4,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 0,  9,  1,  4,  3,  2,  6,  4,  2,  8,  3,  4,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 4,  9,  1,  2,  4,  1,  6,  4,  2,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 3,  1,  8,  1,  6,  8,  6,  4,  8,  1, 10,  6,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 0,  1, 10,  6,  0, 10,  4,  0,  6,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 3,  6,  4,  8,  3,  4,  3, 10,  6,  9,  3,  0,  3,  9, 10] },
	EdgeData { count:  6, edge_indices: [ 4,  9, 10,  4, 10,  6,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 5,  9,  4, 11,  6,  7,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 3,  8,  0,  5,  9,  4,  6,  7, 11,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 1,  0,  5,  0,  4,  5, 11,  6,  7,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 6,  7, 11,  4,  3,  8,  4,  5,  3,  5,  1,  3,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 4,  5,  9,  2,  1, 10, 11,  6,  7,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 7, 11,  6, 10,  2,  1,  3,  8,  0,  5,  9,  4,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [11,  6,  7, 10,  4,  5, 10,  2,  4,  2,  0,  4,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 8,  4,  3,  4,  5,  3,  5,  2,  3,  2,  5, 10,  6,  7, 11] },
	EdgeData { count:  9, edge_indices: [ 3,  2,  7,  2,  6,  7,  9,  4,  5,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 4,  5,  9,  6,  8,  0,  2,  6,  0,  7,  8,  6,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 2,  6,  3,  6,  7,  3,  0,  5,  1,  0,  4,  5,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 8,  2,  6,  7,  8,  6,  8,  1,  2,  5,  8,  4,  8,  5,  1] },
	EdgeData { count: 12, edge_indices: [ 4,  5,  9,  6,  1, 10,  6,  7,  1,  7,  3,  1,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [10,  6,  1,  6,  7,  1,  7,  0,  1,  0,  7,  8,  4,  5,  9] },
	EdgeData { count: 15, edge_indices: [10,  0,  4,  5, 10,  4, 10,  3,  0,  7, 10,  6, 10,  7,  3] },
	EdgeData { count: 12, edge_indices: [10,  6,  7,  8, 10,  7, 10,  4,  5, 10,  8,  4,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 5,  9,  6,  9, 11,  6,  9,  8, 11,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [11,  6,  3,  3,  6,  0,  6,  5,  0,  5,  9,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 8, 11,  0, 11,  5,  0,  5,  1,  0, 11,  6,  5,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 3, 11,  6,  5,  3,  6,  1,  3,  5,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [10,  2,  1, 11,  5,  9,  8, 11,  9,  6,  5, 11,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 3, 11,  0, 11,  6,  0,  6,  9,  0,  9,  6,  5, 10,  2,  1] },
	EdgeData { count: 15, edge_indices: [ 5,  8, 11,  6,  5, 11,  5,  0,  8,  2,  5, 10,  5,  2,  0] },
	EdgeData { count: 12, edge_indices: [ 3, 11,  6,  5,  3,  6,  3, 10,  2,  3,  5, 10,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 9,  8,  5,  8,  2,  5,  2,  6,  5,  2,  8,  3,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 6,  5,  9,  0,  6,  9,  2,  6,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 8,  5,  1,  0,  8,  1,  8,  6,  5,  2,  8,  3,  8,  2,  6] },
	EdgeData { count:  6, edge_indices: [ 6,  5,  1,  6,  1,  2,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 6,  3,  1, 10,  6,  1,  6,  8,  3,  9,  6,  5,  6,  9,  8] },
	EdgeData { count: 12, edge_indices: [ 0,  1, 10,  6,  0, 10,  0,  5,  9,  0,  6,  5,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 8,  3,  0, 10,  6,  5,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 6,  5, 10,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [10,  5, 11, 11,  5,  7,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [10,  5, 11,  5,  7, 11,  0,  3,  8,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 7, 11,  5, 11, 10,  5,  0,  9,  1,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 5,  7, 10,  7, 11, 10,  1,  8,  9,  1,  3,  8,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 2,  1, 11,  1,  7, 11,  1,  5,  7,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 3,  8,  0,  7,  2,  1,  5,  7,  1, 11,  2,  7,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 5,  7,  9,  7,  2,  9,  2,  0,  9,  7, 11,  2,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 2,  5,  7, 11,  2,  7,  2,  9,  5,  8,  2,  3,  2,  8,  9] },
	EdgeData { count:  9, edge_indices: [10,  5,  2,  5,  3,  2,  5,  7,  3,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 0,  2,  8,  2,  5,  8,  5,  7,  8,  5,  2, 10,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 1,  0,  9,  3, 10,  5,  7,  3,  5,  2, 10,  3,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 2,  8,  9,  1,  2,  9,  2,  7,  8,  5,  2, 10,  2,  5,  7] },
	EdgeData { count:  6, edge_indices: [ 5,  3,  1,  5,  7,  3,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 7,  8,  0,  1,  7,  0,  5,  7,  1,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 3,  0,  9,  5,  3,  9,  7,  3,  5,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 7,  8,  9,  7,  9,  5,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 4,  8,  5,  8, 10,  5,  8, 11, 10,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 4,  0,  5,  0, 11,  5, 11, 10,  5,  0,  3, 11,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 9,  1,  0, 10,  4,  8, 11, 10,  8,  5,  4, 10,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 4, 11, 10,  5,  4, 10,  4,  3, 11,  1,  4,  9,  4,  1,  3] },
	EdgeData { count: 12, edge_indices: [ 1,  5,  2,  5,  8,  2,  8, 11,  2,  8,  5,  4,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [11,  4,  0,  3, 11,  0, 11,  5,  4,  1, 11,  2, 11,  1,  5] },
	EdgeData { count: 15, edge_indices: [ 5,  2,  0,  9,  5,  0,  5, 11,  2,  8,  5,  4,  5,  8, 11] },
	EdgeData { count:  6, edge_indices: [ 5,  4,  9,  3, 11,  2,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [10,  5,  2,  2,  5,  3,  5,  4,  3,  4,  8,  3,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 2, 10,  5,  4,  2,  5,  0,  2,  4,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 2, 10,  3, 10,  5,  3,  5,  8,  3,  8,  5,  4,  9,  1,  0] },
	EdgeData { count: 12, edge_indices: [ 2, 10,  5,  4,  2,  5,  2,  9,  1,  2,  4,  9,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 5,  4,  8,  3,  5,  8,  1,  5,  3,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 5,  4,  0,  5,  0,  1,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 5,  4,  8,  3,  5,  8,  5,  0,  9,  5,  3,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 5,  4,  9,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 7, 11,  4, 11,  9,  4, 11, 10,  9,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 3,  8,  0,  7,  9,  4,  7, 11,  9, 11, 10,  9,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [11, 10,  1,  4, 11,  1,  0,  4,  1, 11,  4,  7,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 4,  1,  3,  8,  4,  3,  4, 10,  1, 11,  4,  7,  4, 11, 10] },
	EdgeData { count: 12, edge_indices: [ 7, 11,  4,  4, 11,  9, 11,  2,  9,  2,  1,  9,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 4,  7,  9,  7, 11,  9, 11,  1,  9,  1, 11,  2,  3,  8,  0] },
	EdgeData { count:  9, edge_indices: [ 4,  7, 11,  2,  4, 11,  0,  4,  2,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 4,  7, 11,  2,  4, 11,  4,  3,  8,  4,  2,  3,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [10,  9,  2,  9,  7,  2,  7,  3,  2,  9,  4,  7,  0,  0,  0] },
	EdgeData { count: 15, edge_indices: [ 7, 10,  9,  4,  7,  9,  7,  2, 10,  0,  7,  8,  7,  0,  2] },
	EdgeData { count: 15, edge_indices: [10,  7,  3,  2, 10,  3, 10,  4,  7,  0, 10,  1, 10,  0,  4] },
	EdgeData { count:  6, edge_indices: [ 2, 10,  1,  4,  7,  8,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 1,  9,  4,  7,  1,  4,  3,  1,  7,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 1,  9,  4,  7,  1,  4,  1,  8,  0,  1,  7,  8,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 3,  0,  4,  3,  4,  7,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 7,  8,  4,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 8, 10,  9,  8, 11, 10,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 9,  0,  3, 11,  9,  3, 10,  9, 11,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [10,  1,  0,  8, 10,  0, 11, 10,  8,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [10,  1,  3, 10,  3, 11,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [11,  2,  1,  9, 11,  1,  8, 11,  9,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 9,  0,  3, 11,  9,  3,  9,  2,  1,  9, 11,  2,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [11,  2,  0, 11,  0,  8,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [11,  2,  3,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  9, edge_indices: [ 8,  3,  2, 10,  8,  2,  9,  8, 10,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 2, 10,  9,  2,  9,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count: 12, edge_indices: [ 8,  3,  2, 10,  8,  2,  8,  1,  0,  8, 10,  1,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 2, 10,  1,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  6, edge_indices: [ 8,  3,  1,  8,  1,  9,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 1,  9,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  3, edge_indices: [ 8,  3,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
	EdgeData { count:  0, edge_indices: [ 0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0] },
];

pub const EDGE_CORNER_MAP: [(usize, usize); 12] = [
	(0, 1),
	(1, 2),
	(2, 3),
	(3, 0),
	(4, 5),
	(5, 6),
	(6, 7),
	(7, 4),
	(0, 4),
	(1, 5),
	(2, 6),
	(3, 7),
];

pub const CORNERS: [Point3<f32>; 8] = [
	Point3::new(0.0, 0.0, 1.0),
	Point3::new(1.0, 0.0, 1.0),
	Point3::new(1.0, 0.0, 0.0),
	Point3::new(0.0, 0.0, 0.0),
	Point3::new(0.0, 1.0, 1.0),
	Point3::new(1.0, 1.0, 1.0),
	Point3::new(1.0, 1.0, 0.0),
	Point3::new(0.0, 1.0, 0.0),
];
