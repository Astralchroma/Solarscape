use crate::data::{world::Block, world::Location, Id};
use crate::{message::serverbound::CreateStructure, ShiftHasherBuilder};
use nalgebra::{vector, Vector3};
use std::collections::HashMap;

pub struct Structure {
	pub id: Id,
	pub location: Location,

	blocks: HashMap<Vector3<i16>, Block, ShiftHasherBuilder<3>>,
}

#[cfg(feature = "backend")]
impl From<CreateStructure> for Structure {
	fn from(CreateStructure { location, block }: CreateStructure) -> Self {
		let mut blocks = HashMap::with_capacity_and_hasher(1, ShiftHasherBuilder);
		blocks.insert(vector![0, 0, 0], block);

		Self {
			id: Id::new(),
			location,

			blocks,
		}
	}
}
