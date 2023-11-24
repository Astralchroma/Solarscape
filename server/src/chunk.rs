use crate::{connection::ServerConnection, sync::Syncable};
use hecs::Entity;
use nalgebra::Vector3;
use solarscape_shared::chunk::{index_of_vec, CHUNK_VOLUME};
use solarscape_shared::protocol::{encode, Message, OctreeNode, SyncEntity};
use std::num::NonZeroU8;

pub struct Chunk {
	pub object: Entity,

	pub grid_position: Vector3<i32>,
	pub octree_node: OctreeNode,

	pub density: [f32; CHUNK_VOLUME],
}

impl Chunk {
	pub fn empty(object: Entity, scale: u8, grid_position: Vector3<i32>) -> Self {
		Self {
			object,
			grid_position,
			octree_node: match scale {
				0 => OctreeNode::Real,
				scale => OctreeNode::Node {
					scale: NonZeroU8::new(scale).expect("not 0, we just checked"),
					children: None,
				},
			},
			density: [0.0; CHUNK_VOLUME],
		}
	}

	pub fn get(&self, cell_position: &Vector3<u8>) -> f32 {
		self.density[index_of_vec(cell_position)]
	}

	pub fn set(&mut self, cell_position: &Vector3<u8>, value: f32) {
		self.density[index_of_vec(cell_position)] = value;
	}
}

impl Syncable for Chunk {
	fn sync(&self, entity: Entity, connection: &mut ServerConnection) {
		connection.send(encode(Message::SyncEntity {
			entity,
			sync: SyncEntity::Chunk {
				grid_position: self.grid_position,
				chunk_type: self.octree_node,

				data: self.density,
			},
		}))
	}
}
