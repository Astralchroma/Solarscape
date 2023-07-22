use std::sync::Arc;

pub struct Sector {
	pub name: Box<str>,
	pub display_name: Box<str>,
}

impl Sector {
	pub fn new(name: Box<str>, display_name: Box<str>) -> Arc<Sector> {
		Arc::new(Self { name, display_name })
	}
}
