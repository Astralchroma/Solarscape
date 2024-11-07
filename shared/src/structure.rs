use crate::data::{world::Location, Id};

#[non_exhaustive]
pub struct Structure {
	pub id: Id,
	pub location: Location,
}

impl Structure {
	#[cfg(feature = "backend")]
	pub fn new(location: Location) -> Self {
		Self {
			id: Id::new(),
			location,
		}
	}
}
