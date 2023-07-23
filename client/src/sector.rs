use crate::object::Object;
use std::{cell::RefCell, collections::HashMap, sync::Arc};

pub struct Sector {
	pub name: Box<str>,
	pub display_name: Box<str>,
	pub objects: RefCell<HashMap<u32, Object>>,
}

impl Sector {
	pub fn new(name: Box<str>, display_name: Box<str>) -> Arc<Sector> {
		Arc::new(Self {
			name,
			display_name,
			objects: RefCell::new(HashMap::new()),
		})
	}
}
