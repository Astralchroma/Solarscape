use crate::sector::Sector;
use log::info;
use std::{cell::RefCell, sync::Arc};

pub struct World {
	sectors: RefCell<Vec<Arc<Sector>>>,
	active_sector: RefCell<Option<Arc<Sector>>>,
}

impl World {
	pub fn new() -> Arc<Self> {
		Arc::new(Self {
			sectors: RefCell::new(vec![]),
			active_sector: RefCell::new(None),
		})
	}

	pub fn add_sector(&self, name: Box<str>, display_name: Box<str>) {
		info!("Added sector \"{name}\"");
		self.sectors.borrow_mut().push(Sector::new(name, display_name))
	}

	pub fn set_active_sector(&self, name: Box<str>) {
		info!("Active sector is now \"{name}\"");
		*self.active_sector.borrow_mut() = Some(
			self.sectors
				.borrow()
				.iter()
				.find(|other| other.name == name)
				.expect("should not set active sector that doesnt exist")
				.clone(),
		);
	}
}
