use bincode::{Decode, Encode};

#[derive(Clone, Debug, Decode, Encode)]
pub struct SectorData {
	pub name: Box<str>,
	pub display_name: Box<str>,
}
