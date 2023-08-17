use crate::object::Object;
use std::{collections::HashMap, sync::Arc};

pub struct SectorMeta {
	pub name: Box<str>,
	pub display_name: Box<str>,
}

impl SectorMeta {
	pub fn new(name: Box<str>, display_name: Box<str>) -> Arc<Self> {
		Arc::new(Self { name, display_name })
	}
}

pub struct Sector {
	pub meta: Arc<SectorMeta>,
	pub objects: HashMap<u32, Object>,
}

impl Sector {
	pub fn new(meta: Arc<SectorMeta>) -> Self {
		Self {
			meta,
			objects: HashMap::new(),
		}
	}
}
