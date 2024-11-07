use crate::data::{world::Block, world::Location, Id};
use crate::{message::clientbound::SyncStructure, ShiftHasherBuilder};
use nalgebra::Vector3;
use std::collections::HashMap;

#[cfg(feature = "backend")]
use crate::message::serverbound::CreateStructure;

pub struct Structure {
	pub id: Id,
	pub location: Location,

	blocks: HashMap<Vector3<i16>, Block, ShiftHasherBuilder<3>>,
}

impl Structure {
	pub fn sync(&self) -> SyncStructure {
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

#[cfg(feature = "backend")]
impl From<CreateStructure> for Structure {
	fn from(CreateStructure { location, block }: CreateStructure) -> Self {
		let mut blocks = HashMap::with_capacity_and_hasher(1, ShiftHasherBuilder);
		blocks.insert(nalgebra::vector![0, 0, 0], block);

		Self {
			id: Id::new(),
			location,

			blocks,
		}
	}
}

impl From<SyncStructure> for Structure {
	fn from(SyncStructure { id, location, blocks }: SyncStructure) -> Self {
		Self { id, location, blocks }
	}
}
