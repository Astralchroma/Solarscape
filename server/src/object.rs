use crate::{chunk::Chunk, connection::Connection, sector::Sector};
use nalgebra::Vector3;
use solarscape_shared::protocol::Clientbound;
use std::{
	collections::HashMap,
	sync::{atomic::Ordering::Relaxed, Arc},
};
use tokio::sync::RwLock;

pub const CHUNK_RADIUS: i32 = 2;
pub const RADIUS: f64 = (CHUNK_RADIUS << 4) as f64;

pub struct Object {
	pub object_id: u32,
	pub chunks: RwLock<HashMap<Vector3<i32>, Chunk>>,
}

impl Object {
	/// TODO: Temporary
	pub fn sphere(sector: &Arc<Sector>) -> Arc<Self> {
		let mut star = Arc::new(Self {
			object_id: sector.object_id_counter.fetch_add(1, Relaxed),
			chunks: RwLock::new(HashMap::new()),
		});
		star.populate_sphere();
		star
	}

	/// TODO: Temporary
	fn populate_sphere(self: &Arc<Self>) {
		let mut chunks = self.chunks.blocking_write();

		for x in -CHUNK_RADIUS..CHUNK_RADIUS {
			for y in -CHUNK_RADIUS..CHUNK_RADIUS {
				for z in -CHUNK_RADIUS..CHUNK_RADIUS {
					let chunk_grid_position = Vector3::new(x, y, z);
					let chunk = Chunk::new_sphere(Arc::downgrade(self), chunk_grid_position);
					chunks.insert(chunk_grid_position, chunk);
				}
			}
		}
	}

	pub async fn subscribe(&self, connection: &Arc<Connection>) {
		connection.send(Clientbound::AddObject {
			object_id: self.object_id,
		});
		for chunk in self.chunks.read().await.values() {
			chunk.subscribe(connection).await
		}
	}
}
