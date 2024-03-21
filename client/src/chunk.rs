use crate::data::{EdgeData, CELL_EDGE_MAP, CORNERS, EDGE_CORNER_MAP};
use bytemuck::{cast_slice, Pod, Zeroable};
use nalgebra::{Point3, Vector3};
use solarscape_shared::types::ChunkData;
use wgpu::{util::BufferInitDescriptor, util::DeviceExt, Buffer, BufferUsages, Device, RenderPass};

pub struct Chunk {
	pub level: u8,
	pub coordinates: Vector3<i32>,

	pub data: ChunkData,

	pub mesh: Option<ChunkMesh>,
}

pub struct ChunkMesh {
	vertex_count: u32,
	vertex_buffer: Buffer,
	instance_buffer: Buffer,
}

impl Chunk {
	pub fn rebuild_mesh(&mut self, device: &Device) {
		let mut vertices = vec![];

		for x in 0..15 {
			for y in 0..15 {
				for z in 0..15 {
					#[allow(clippy::identity_op)]
					#[rustfmt::skip]
					let case_index = ((self.data[(x  ) << 8 | (y  ) << 4 | (z+1)] <= 4.0) as usize) << 0
					               | ((self.data[(x+1) << 8 | (y  ) << 4 | (z+1)] <= 4.0) as usize) << 1
					               | ((self.data[(x+1) << 8 | (y  ) << 4 | (z  )] <= 4.0) as usize) << 2
					               | ((self.data[(x  ) << 8 | (y  ) << 4 | (z  )] <= 4.0) as usize) << 3
					               | ((self.data[(x  ) << 8 | (y+1) << 4 | (z+1)] <= 4.0) as usize) << 4
					               | ((self.data[(x+1) << 8 | (y+1) << 4 | (z+1)] <= 4.0) as usize) << 5
					               | ((self.data[(x+1) << 8 | (y+1) << 4 | (z  )] <= 4.0) as usize) << 6
					               | ((self.data[(x  ) << 8 | (y+1) << 4 | (z  )] <= 4.0) as usize) << 7;

					let EdgeData { count, edge_indices } = CELL_EDGE_MAP[case_index];

					for edge_index in &edge_indices[0..count as usize] {
						let edge = EDGE_CORNER_MAP[*edge_index as usize];

						let a = interpolate(CORNERS[edge[0]], CORNERS[edge[1]]);
						let b = interpolate(CORNERS[edge[1]], CORNERS[edge[0]]);

						vertices.push(Point3::new(
							x as f32 + (a.x + b.x) / 2.0,
							y as f32 + (a.y + b.y) / 2.0,
							z as f32 + (a.z + b.z) / 2.0,
						));
					}
				}
			}
		}

		if vertices.is_empty() {
			self.mesh = None;
			return;
		}

		#[derive(Clone, Copy)]
		struct InstanceData {
			position: Vector3<f32>,
			scale: f32,
		}

		unsafe impl Zeroable for InstanceData {}
		unsafe impl Pod for InstanceData {}

		self.mesh = Some(ChunkMesh {
			vertex_count: vertices.len() as u32,
			vertex_buffer: device.create_buffer_init(&BufferInitDescriptor {
				label: Some("chunk.mesh.vertex_buffer"),
				contents: cast_slice(&vertices),
				usage: BufferUsages::VERTEX,
			}),
			instance_buffer: device.create_buffer_init(&BufferInitDescriptor {
				label: Some("chunk.mesh.instance_buffer"),
				contents: cast_slice(&[InstanceData {
					position: self.coordinates.cast() * ((16u64 << self.level) + 1) as f32,
					scale: (self.level + 1) as f32,
				}]),
				usage: BufferUsages::VERTEX,
			}),
		});
	}

	pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
		if self.level != 0 {
			return;
		}

		if let Some(ChunkMesh { vertex_count, vertex_buffer, instance_buffer }) = &self.mesh {
			render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
			render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
			render_pass.draw(0..*vertex_count, 0..1);
		}
	}
}

#[must_use]
fn interpolate(a: Point3<f32>, b: Point3<f32>) -> Point3<f32> {
	Point3::new(
		a.x + 1.0 * (b.x - a.x),
		a.y + 1.0 * (b.y - a.y),
		a.z + 1.0 * (b.z - a.z),
	)
}
