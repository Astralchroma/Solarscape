use crate::object::Object;
use std::collections::HashMap;

pub struct Sector {
	pub name: Box<str>,
	pub display_name: Box<str>,
	pub objects: HashMap<u32, Object>,
}

impl Sector {
	pub fn new(name: Box<str>, display_name: Box<str>) -> Sector {
		Self {
			name,
			display_name,
			objects: HashMap::new(),
		}
	}
}
