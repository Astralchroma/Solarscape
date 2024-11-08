use crate::data::{world::Block, world::Location, Id};
use crate::physics::{AutoCleanup, Physics};
use crate::{message::clientbound::SyncStructure, ShiftHasherBuilder};
use nalgebra::{vector, Vector3};
use rapier3d::dynamics::RigidBodyBuilder;
use rapier3d::prelude::RigidBodyHandle;
use std::collections::HashMap;

#[cfg(feature = "backend")]
use crate::message::serverbound::CreateStructure;

pub struct Structure {
	pub id: Id,
	pub location: Location,

	pub rigid_body: AutoCleanup<RigidBodyHandle>,

	blocks: HashMap<Vector3<i16>, Block, ShiftHasherBuilder<3>>,
}

impl Structure {
	#[cfg(feature = "backend")]
	pub fn new(physics: &mut Physics, CreateStructure { location, block }: CreateStructure) -> Self {
		let (x, y, z) = location.rotation.euler_angles();

		let rigid_body = physics.insert_rigid_body(
			RigidBodyBuilder::dynamic()
				.translation(location.position.coords)
				.rotation(vector![x, y, z]),
		);

		let mut blocks = HashMap::with_capacity_and_hasher(1, ShiftHasherBuilder);
		blocks.insert(nalgebra::vector![0, 0, 0], block);

		Self {
			id: Id::new(),
			location,

			rigid_body,

			blocks,
		}
	}

	pub fn new_from_sync(physics: &mut Physics, SyncStructure { id, location, blocks }: SyncStructure) -> Self {
		let (x, y, z) = location.rotation.euler_angles();

		let rigid_body = physics.insert_rigid_body(
			RigidBodyBuilder::dynamic()
				.translation(location.position.coords)
				.rotation(vector![x, y, z]),
		);

		Self {
			id,
			location,

			rigid_body,

			blocks,
		}
	}

	pub fn build_sync(&self) -> SyncStructure {
		SyncStructure {
			id: self.id,
			location: self.location,
			blocks: self.blocks.clone(),
		}
	}

	pub fn iter_blocks(&self) -> impl Iterator<Item = (&Vector3<i16>, &Block)> {
		self.blocks.iter()
	}

	pub fn num_blocks(&self) -> usize {
		self.blocks.len()
	}
}
