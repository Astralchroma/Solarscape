use crate::connection::ServerConnection;
use hecs::{Entity, EntityRef, NoSuchEntity, World};
use once_cell::sync::Lazy;
use paste::paste;
use solarscape_shared::protocol::{encode, Event, Message, SyncEntity};
use solarscape_shared::{chunk::Chunk, components::Location, components::Sector, components::VoxelObject};
use std::{any::TypeId, collections::HashMap};

pub type Subscribers = Vec<usize>;

trait Syncer {
	fn sync(&self, entity_ref: &EntityRef) -> Result<SyncEntity, NoSuchEntity>;
}

macro_rules! syncer {
	($component:ident, |$name:ident| $convert:expr) => {
		paste! { syncer!([<$component Syncer>], $component, |$name| $convert); }
	};
	($syncer:ident, $component:ident, |$name:ident| $convert:expr) => {
		struct $syncer;

		impl Syncer for $syncer {
			fn sync(&self, entity_ref: &EntityRef) -> Result<SyncEntity, NoSuchEntity> {
				entity_ref
					.get::<&$component>()
					.ok_or(NoSuchEntity)
					.map(|$name| SyncEntity::$component($convert))
			}
		}
	};
}

syncer!(Sector, |sector| (*sector).clone());
syncer!(VoxelObject, |voxject| *voxject);
syncer!(Chunk, |chunk| *chunk);
syncer!(Location, |location| *location);

static SYNCERS: Lazy<HashMap<TypeId, Box<dyn Syncer + Send + Sync>>> = Lazy::new(|| {
	let mut hashmap = HashMap::<TypeId, Box<dyn Syncer + Send + Sync>>::new();
	hashmap.insert(TypeId::of::<Sector>(), Box::new(SectorSyncer));
	hashmap.insert(TypeId::of::<VoxelObject>(), Box::new(VoxelObjectSyncer));
	hashmap.insert(TypeId::of::<Chunk>(), Box::new(ChunkSyncer));
	hashmap.insert(TypeId::of::<Location>(), Box::new(LocationSyncer));
	hashmap
});

pub fn subscribe(
	world: &World,
	target: &Entity,
	connection_id: usize,
	connection: &ServerConnection,
) -> Result<(), NoSuchEntity> {
	let target_ref = world.entity(*target)?;

	let mut target_sub = target_ref.get::<&mut Subscribers>().ok_or(NoSuchEntity)?;

	if target_sub.contains(&connection_id) {
		// Already subscribed, don't bother
		return Ok(());
	}

	{
		for component in target_ref.component_types() {
			match SYNCERS.get(&component) {
				None => {}
				Some(syncer) => connection.send(encode(Message::SyncEntity {
					entity: *target,
					sync: syncer.sync(&target_ref)?,
				})),
			}
		}
	}

	target_sub.push(connection_id);

	Ok(())
}

pub fn unsubscribe(
	world: &World,
	target: &Entity,
	connection_id: usize,
	connection: &ServerConnection,
) -> Result<(), NoSuchEntity> {
	{
		let mut target_sub_query = world.query_one::<&mut Subscribers>(*target)?;
		let target_sub = target_sub_query.get().ok_or(NoSuchEntity)?;

		target_sub.retain(|other| other != &connection_id);
	}

	{
		connection.send(encode(Message::Event(Event::DespawnEntity(*target))));
	}

	Ok(())
}
