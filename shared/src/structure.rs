use crate::{
	data::{
		world::{BlockType, Location},
		Id,
	},
	message::clientbound::SyncStructure,
	physics::{AutoCleanup, Physics},
	ShiftHasherBuilder,
};
use nalgebra::{vector, Isometry3, Point3, Vector3};
use rapier3d::{
	dynamics::{RigidBodyBuilder, RigidBodyHandle},
	geometry::{ColliderBuilder, ColliderHandle},
};
use std::collections::HashMap;

#[cfg(feature = "backend")]
use crate::message::serverbound::CreateStructure;

pub struct Structure {
	pub id: Id,
	pub rigid_body: AutoCleanup<RigidBodyHandle>,

	blocks: HashMap<Vector3<i16>, Block, ShiftHasherBuilder<3>>,
}

impl Structure {
	#[cfg(feature = "backend")]
	pub fn new(
		physics: &mut Physics,
		CreateStructure { location, block }: CreateStructure,
	) -> Self {
		let (x, y, z) = location.rotation.euler_angles();

		let rigid_body = physics.insert_rigid_body(
			RigidBodyBuilder::dynamic()
				.translation(location.position.coords)
				.rotation(vector![x, y, z]),
		);

		let mut blocks = HashMap::with_capacity_and_hasher(1, ShiftHasherBuilder);
		blocks.insert(
			nalgebra::vector![0, 0, 0],
			Block {
				typ: block,
				_collider: physics.insert_rigid_body_collider(
					*rigid_body,
					ColliderBuilder::cuboid(0.5, 0.5, 0.5),
				),
			},
		);

		Self {
			id: Id::new(),
			rigid_body,

			blocks,
		}
	}

	pub fn new_from_sync(
		physics: &mut Physics,
		SyncStructure {
			id,
			location,
			blocks,
		}: SyncStructure,
	) -> Self {
		let (x, y, z) = location.rotation.euler_angles();

		let rigid_body = physics.insert_rigid_body(
			RigidBodyBuilder::dynamic()
				.translation(location.position.coords)
				.rotation(vector![x, y, z]),
		);

		let blocks = blocks
			.into_iter()
			.map(|(position, typ)| {
				(
					position,
					Block {
						typ,
						_collider: physics.insert_rigid_body_collider(
							*rigid_body,
							ColliderBuilder::cuboid(0.5, 0.5, 0.5),
						),
					},
				)
			})
			.collect();

		Self {
			id,
			rigid_body,
			blocks,
		}
	}

	pub fn build_sync(&self, physics: &Physics) -> SyncStructure {
		let rigid_body = physics
			.get_rigid_body(*self.rigid_body)
			.expect("rigid body shouldn't be removed while structure still exists");

		let location = Location {
			position: Point3 {
				coords: *rigid_body.translation(),
			},
			rotation: *rigid_body.rotation(),
		};

		SyncStructure {
			id: self.id,
			location,
			blocks: self
				.blocks
				.iter()
				.map(|(position, block)| (*position, block.typ))
				.collect(),
		}
	}

	pub fn get_location<'p>(&self, physics: &'p Physics) -> &'p Isometry3<f32> {
		physics
			.get_rigid_body(*self.rigid_body)
			.expect("rigid body shouldn't be removed while structure still exists")
			.position()
	}

	pub fn iter_blocks(&self) -> impl Iterator<Item = (&Vector3<i16>, &Block)> {
		self.blocks.iter()
	}

	pub fn num_blocks(&self) -> usize {
		self.blocks.len()
	}
}

pub struct Block {
	pub typ: BlockType,
	_collider: AutoCleanup<ColliderHandle>,
}
