use crate::object::Object;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

pub struct Sector {
	pub name: Box<str>,
	pub display_name: Box<str>,
	pub objects: RwLock<HashMap<u32, Object>>,
}

impl Sector {
	pub fn new(name: Box<str>, display_name: Box<str>) -> Arc<Sector> {
		Arc::new(Self {
			name,
			display_name,
			objects: RwLock::new(HashMap::new()),
		})
	}
}
