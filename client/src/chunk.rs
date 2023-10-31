use crate::triangulation_table::{CORNERS, EDGES, TRIANGULATION_TABLE};
use bytemuck::cast_slice;
use nalgebra::{convert, Vector3};
use solarscape_shared::chunk::{index_of, CHUNK_VOLUME};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferUsages, Device, RenderPass};

pub struct Chunk {
	pub grid_position: Vector3<i32>,
	pub data: [bool; CHUNK_VOLUME],

	pub instance_buffer: Buffer,
	pub vertex_buffer: Option<Buffer>,
	pub vertex_count: u32,
}

impl Chunk {
	#[must_use]
	pub fn new(device: &Device, grid_position: Vector3<i32>, data: [bool; CHUNK_VOLUME]) -> Self {
		let mut chunk = Self {
			instance_buffer: device.create_buffer_init(&BufferInitDescriptor {
				label: None, // TODO
				contents: cast_slice(convert::<_, Vector3<f32>>(grid_position * 16).as_slice()),
				usage: BufferUsages::VERTEX,
			}),
			vertex_buffer: None,
			grid_position,
			data,
			vertex_count: 0,
		};
		chunk.build_mesh(device);
		chunk
	}

	// TODO: Meshes are all blocking built on the main thread. Start here if there is a lot of lag when loading chunks.
	pub fn build_mesh(&mut self, device: &Device) {
		// TODO: Not using indexes, this is also wasteful. Start here if there are rendering performance problems.
		let mut vertices = vec![];

		for x in 0..15 {
			for y in 0..15 {
				for z in 0..15 {
					#[rustfmt::skip]
					#[allow(clippy::identity_op)]
					let cube_index = {
						let mut result = 0u8;

						if self.data[index_of(x + 0, y + 0, z + 1)] { result |=   1 };
						if self.data[index_of(x + 1, y + 0, z + 1)] { result |=   2 };
						if self.data[index_of(x + 1, y + 0, z + 0)] { result |=   4 };
						if self.data[index_of(x + 0, y + 0, z + 0)] { result |=   8 };
						if self.data[index_of(x + 0, y + 1, z + 1)] { result |=  16 };
						if self.data[index_of(x + 1, y + 1, z + 1)] { result |=  32 };
						if self.data[index_of(x + 1, y + 1, z + 0)] { result |=  64 };
						if self.data[index_of(x + 0, y + 1, z + 0)] { result |= 128 };

						result
					};

					let edges = TRIANGULATION_TABLE[cube_index as usize];

					for edge_index in (0..16).step_by(3) {
						if edges[edge_index] == -1 {
							break;
						}

						for offset in 0..3 {
							let edge = EDGES[edges[edge_index + offset] as usize];

							let a = interpolate(CORNERS[edge[0]], CORNERS[edge[1]]);
							let b = interpolate(CORNERS[edge[1]], CORNERS[edge[0]]);

							vertices.push(x as f32 + (a[0] + b[0]) / 2.0);
							vertices.push(y as f32 + (a[1] + b[1]) / 2.0);
							vertices.push(z as f32 + (a[2] + b[2]) / 2.0);
						}
					}
				}
			}
		}

		self.vertex_count = vertices.len() as u32 / 3;
		if self.vertex_count == 0 {
			return;
		}

		self.vertex_buffer = Some(device.create_buffer_init(&BufferInitDescriptor {
			label: None, // TODO
			contents: cast_slice(&vertices),
			usage: BufferUsages::VERTEX,
		}));
	}

	pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
		if let Some(ref vertex_buffer) = self.vertex_buffer {
			render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
			render_pass.set_vertex_buffer(1, vertex_buffer.slice(..));
			render_pass.draw(0..self.vertex_count, 0..1)
		};
	}
}

// TODO: Figure out what the hell this function is doing. :foxple:
#[must_use]
fn interpolate(corner1: Vector3<f32>, corner2: Vector3<f32>) -> [f32; 3] {
	[
		corner1[0] + 1.0 * (corner2[0] - corner1[0]),
		corner1[1] + 1.0 * (corner2[1] - corner1[1]),
		corner1[2] + 1.0 * (corner2[2] - corner1[2]),
	]
}
