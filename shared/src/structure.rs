use crate::data::Id;

#[non_exhaustive]
pub struct Structure {
	pub id: Id,
}

impl Structure {
	#[cfg(feature = "backend")]
	pub fn new() -> Self {
		Self { id: Id::new() }
	}
}
