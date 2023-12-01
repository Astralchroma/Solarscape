use crate::{connection::ServerConnection, server::Server};
use hecs::{Entity, EntityRef, NoSuchEntity};
use once_cell::sync::Lazy;
use paste::paste;
use solarscape_shared::protocol::{encode, Event, Message, SyncEntity};
use solarscape_shared::{chunk::Chunk, components::Location, components::Sector, components::VoxelObject};
use std::{any::TypeId, collections::HashMap};

pub type Subscribers = Vec<Entity>;

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

pub fn subscribe(server: &Server, target: &Entity, player: &Entity) -> Result<(), NoSuchEntity> {
	let target_ref = server.world.entity(*target)?;

	let mut target_sub = target_ref.get::<&mut Subscribers>().ok_or(NoSuchEntity)?;

	{
		let mut player_con_query = server.world.query_one::<&ServerConnection>(*player)?;
		let player_con = player_con_query.get().ok_or(NoSuchEntity)?;

		for component in target_ref.component_types() {
			match SYNCERS.get(&component) {
				None => {}
				Some(syncer) => player_con.send(encode(Message::SyncEntity {
					entity: *target,
					sync: syncer.sync(&target_ref)?,
				})),
			}
		}
	}

	target_sub.push(*player);

	Ok(())
}

pub fn unsubscribe(server: &Server, target: &Entity, player: &Entity) -> Result<(), NoSuchEntity> {
	{
		let mut target_sub_query = server.world.query_one::<&mut Subscribers>(*target)?;
		let target_sub = target_sub_query.get().ok_or(NoSuchEntity)?;

		target_sub.retain(|entity| entity != player);
	}

	{
		let mut player_con_query = server.world.query_one::<&ServerConnection>(*player)?;
		let player_con = player_con_query.get().ok_or(NoSuchEntity)?;

		player_con.send(encode(Message::Event(Event::DespawnEntity(*target))));
	}

	Ok(())
}
