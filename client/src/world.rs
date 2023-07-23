use crate::sector::Sector;
use log::info;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct World {
	sectors: RwLock<Vec<Arc<Sector>>>,
	active_sector: RwLock<Option<Arc<Sector>>>,
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
		*self.active_sector.write().await = Some(
			self.sectors
				.read()
				.await
				.iter()
				.find(|other| other.name == name)
				.expect("should not set active sector that doesnt exist")
				.clone(),
		);
	}

	pub async fn active_sector(&self) -> Arc<Sector> {
		self.active_sector
			.read()
			.await
			.clone()
			.expect("active_sector should be set")
	}
}
