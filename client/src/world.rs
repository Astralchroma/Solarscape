use crate::sector::Sector;
use log::info;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct World {
	sectors: RwLock<Vec<Sector>>,
	pub active_sector: RwLock<Option<Sector>>,
}

impl World {
	pub fn new() -> Arc<Self> {
		Arc::new(Self {
			sectors: RwLock::new(vec![]),
			active_sector: RwLock::new(None),
		})
	}

	pub async fn add_sector(&self, name: Box<str>, display_name: Box<str>) {
		info!("Added sector \"{name}\"");
		self.sectors.write().await.push(Sector::new(name, display_name))
	}

	pub async fn set_active_sector(&self, name: Box<str>) {
		info!("Active sector is now \"{name}\"");

		let mut sectors = self.sectors.write().await;
		let (index, _) = sectors
			.iter()
			.enumerate()
			.find(|(_, other)| other.name == name)
			.expect("active sector should exist");
		let sector = sectors.remove(index);

		*self.active_sector.write().await = Some(sector);
	}
}
