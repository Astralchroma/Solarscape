//! I present to you: The Mega List of Potential Chunk Optimizations
//! - Build chunk meshes asynchronously
//! - Only trigger updates of dependent chunks if the edges of the chunk actually changed
//! - Delay chunk generation until dependency chunks have loaded
//! - Simd
//! - GPU Compute
//! - Cache meshes on the client and have the server tell the client to use or invalidate that cache on chunk load
//! - Chunk Position -> Entity ID Lookup Table
//! - Build and cache some chunks server side
//! - General algorithm optimizations
//! - Use Indices
//!
//! Kept coming up with ideas, but we dont need them right now, so just note them down here as potential things to look
//! into in the future if chunk performance becomes a problem.

use crate::triangulation_table::{CORNERS, EDGES, TRIANGULATION_TABLE};
use bytemuck::cast_slice;
use hecs::World;
use nalgebra::Vector3;
use solarscape_shared::chunk::Chunk;
use wgpu::{util::BufferInitDescriptor, util::DeviceExt, Buffer, BufferUsages, Device, RenderPass};

pub struct ChunkMesh {
	pub vertex_buffer: Buffer,
	pub vertex_count: u32,
}

impl ChunkMesh {
	pub fn new(world: &World, chunk: &Chunk, device: &Device) -> Option<Self> {
		let mut vertices = vec![];

		let mut chunks = [None; 8];
		chunks[0] = Some(chunk);

		let positions: [Vector3<i32>; 7] = [
			chunk.grid_position + Vector3::new(0, 0, 1),
			chunk.grid_position + Vector3::new(0, 1, 0),
			chunk.grid_position + Vector3::new(0, 1, 1),
			chunk.grid_position + Vector3::new(1, 0, 0),
			chunk.grid_position + Vector3::new(1, 0, 1),
			chunk.grid_position + Vector3::new(1, 1, 0),
			chunk.grid_position + Vector3::new(1, 1, 1),
		];

		// Great thing about ECS: Really fast iteration
		// Problem with ECS: If you wanna find something you need but don't know it's ID, its potentially O(n).
		// This will probably cause performance problems later.
		let mut query = world.query::<&Chunk>();
		for (_, other) in &mut query {
			if other.voxel_object != chunk.voxel_object {
				continue;
			}

			for (index, position) in positions.iter().enumerate() {
				if other.grid_position == *position {
					chunks[index + 1] = Some(other);
					break;
				}
			}
		}

		let get = |x: u8, y: u8, z: u8| -> f32 {
			let chunk_index = ((((x == 16) as u8) << 2) + (((y == 16) as u8) << 1) + ((z == 16) as u8)) as usize;
			match chunks[chunk_index] {
				Some(chunk) => chunk.get(
					match x {
						16 => 0,
						x => x,
					},
					match y {
						16 => 0,
						y => y,
					},
					match z {
						16 => 0,
						z => z,
					},
				),
				None => 0.0,
			}
		};

		for x in 0..16 {
			for y in 0..16 {
				for z in 0..16 {
					#[rustfmt::skip]
					#[allow(clippy::identity_op)]
					let cube_index = {
						let mut result = 0u8;

						if get(x + 0, y + 0, z + 1) > 0.0 { result |=   1 };
						if get(x + 1, y + 0, z + 1) > 0.0 { result |=   2 };
						if get(x + 1, y + 0, z + 0) > 0.0 { result |=   4 };
						if get(x + 0, y + 0, z + 0) > 0.0 { result |=   8 };
						if get(x + 0, y + 1, z + 1) > 0.0 { result |=  16 };
						if get(x + 1, y + 1, z + 1) > 0.0 { result |=  32 };
						if get(x + 1, y + 1, z + 0) > 0.0 { result |=  64 };
						if get(x + 0, y + 1, z + 0) > 0.0 { result |= 128 };

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

		if vertices.is_empty() {
			return None;
		}

		Some(ChunkMesh {
			vertex_count: vertices.len() as u32 / 3,
			vertex_buffer: device.create_buffer_init(&BufferInitDescriptor {
				label: None, // TODO
				contents: cast_slice(&vertices),
				usage: BufferUsages::VERTEX,
			}),
		})
	}

	pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
		render_pass.set_vertex_buffer(1, self.vertex_buffer.slice(..));
		render_pass.draw(0..self.vertex_count, 0..1)
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
