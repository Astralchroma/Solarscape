use crate::{
	camera::Camera, camera::OrbitCamera, chunk::ChunkMesh, components::LocationBuffer, connection::ClientConnection,
	connection::ConnectionError, renderer::Renderer, renderer::RendererInitializationError, Arguments,
};
use hecs::{Component, Entity, Without, World};
use log::{error, info, warn};
use nalgebra::Vector3;
use solarscape_shared::protocol::{encode, DisconnectReason, Event, Message, SyncEntity};
use solarscape_shared::{chunk::Chunk, components::Sector};
use std::{mem, time::SystemTime};
use thiserror::Error;
use tokio::{runtime::Runtime, sync::mpsc::error::TryRecvError};
use winit::event::Event::{AboutToWait, DeviceEvent, WindowEvent};
use winit::event::WindowEvent::{CloseRequested, Destroyed, MouseInput, MouseWheel, RedrawRequested, Resized};
use winit::{error::EventLoopError, event_loop::EventLoop};

pub struct Client {
	pub renderer: Renderer,
	discord_rpc: discord_rpc_client::Client,

	pub camera: Camera,
	pub camera_controller: OrbitCamera,
	current_sector: Option<Entity>,

	pub world: World,
}

impl Client {
	pub fn run(arguments: Arguments, runtime: Runtime) -> Result<(), ClientInitializationError> {
		let event_loop = EventLoop::new()?;
		let renderer = Renderer::init(&event_loop, &arguments, &runtime)?;

		let mut discord_rpc = discord_rpc_client::Client::new(1178300453872746516);
		discord_rpc.on_ready(|_| info!("Discord RPC Ready!"));
		discord_rpc.on_error(|error| warn!("Discord RPC Error! {error:?}"));
		discord_rpc.start();

		let connection = runtime.block_on(ClientConnection::connect("[::1]:23500"))?;

		let mut client = Self {
			camera: Camera::new(&renderer),
			camera_controller: OrbitCamera::default(),

			renderer,
			discord_rpc,

			current_sector: None,

			world: World::new(),
		};

		client.update_discord_rpc(Some("Loading"), None);

		Ok(client.event_loop(event_loop, connection)?)
	}

	pub fn update_discord_rpc(&mut self, state: Option<&str>, details: Option<&str>) {
		let _ = self.discord_rpc.set_activity(|a| {
			a.state(state.unwrap_or(""))
				.details(details.unwrap_or(""))
				.timestamps(|t| {
					t.start(
						SystemTime::now()
							.duration_since(SystemTime::UNIX_EPOCH)
							.expect("time since epoch")
							.as_secs(),
					)
				})
		});
	}

	// TODO: This looks very messy, I hate it, clean it up if possible.
	fn event_loop(mut self, event_loop: EventLoop<()>, mut connection: ClientConnection) -> Result<(), EventLoopError> {
		event_loop.run(move |event, control_flow| match event {
			WindowEvent { event, .. } => match event {
				Resized(new_size) => {
					self.renderer.resize(new_size);
					self.camera.update(&self.renderer, &self.camera_controller);
				}
				CloseRequested | Destroyed => control_flow.exit(),
				MouseWheel { delta, .. } => {
					self.camera_controller.handle_mouse_wheel(delta);
					self.camera.update(&self.renderer, &self.camera_controller);
				}
				MouseInput { state, button, .. } => {
					self.camera_controller.handle_mouse_input(state, button);
					self.camera.update(&self.renderer, &self.camera_controller);
				}
				RedrawRequested => Renderer::render(&mut self).expect("aaaaaaaaaa"),
				_ => {}
			},
			DeviceEvent { event, .. } => {
				self.camera_controller.handle_device_event(event);
				self.camera.update(&self.renderer, &self.camera_controller);
			}
			AboutToWait => {
				if self.camera.use_position_changed() {
					connection.send(encode(Message::Event(Event::PositionUpdated(
						*self.camera.get_position(),
					))));
				}

				loop {
					match connection.receive().try_recv() {
						Ok(packet) => match self.process_message(packet) {
							Ok(_) => {}
							Err(disconnect_reason) => {
								// Cursed hack to steal the connection so we can disconnect it
								#[allow(invalid_value)] // I know, we don't use it, so just pretend it is valid
								let stolen_connection = mem::replace(&mut connection, unsafe { mem::zeroed() });
								stolen_connection.disconnect(disconnect_reason);
								panic!("Disconnecting from server, reason: {disconnect_reason:?}");
							}
						},
						Err(error) => match error {
							TryRecvError::Empty => break,
							TryRecvError::Disconnected => panic!("Disconnected from Server!"),
						},
					}
				}

				self.renderer.window.request_redraw();
			}
			_ => {}
		})
	}

	fn process_message(&mut self, message: Message) -> Result<(), DisconnectReason> {
		// Unfortunately impl Trait isn't allowed on closures (yet at least), so this has to be a function.
		fn insert_or_spawn_at(world: &mut World, entity: Entity, component: impl Component) {
			match world.contains(entity) {
				true => world
					.insert_one(entity, component)
					.expect("entity will exist, we explicitly check that it does"),
				false => world.spawn_at(entity, (component,)),
			}
		}

		match message {
			Message::SyncEntity { entity, sync } => match sync {
				SyncEntity::Sector(sector) => insert_or_spawn_at(&mut self.world, entity, sector),
				SyncEntity::VoxelObject(voxel_object) => insert_or_spawn_at(&mut self.world, entity, voxel_object),
				SyncEntity::Chunk(chunk) => {
					if let Some(chunk_mesh) = ChunkMesh::new(&self.world, &chunk, &self.renderer.device) {
						insert_or_spawn_at(&mut self.world, entity, chunk_mesh)
					}
					insert_or_spawn_at(&mut self.world, entity, chunk);

					// Rebuild dependent chunks
					// This is probably really bad for performance as we are likely double building a lot of chunks
					let positions: [Vector3<i32>; 7] = [
						chunk.grid_position + Vector3::new(0, 0, -1),
						chunk.grid_position + Vector3::new(0, -1, 0),
						chunk.grid_position + Vector3::new(0, -1, -1),
						chunk.grid_position + Vector3::new(-1, 0, 0),
						chunk.grid_position + Vector3::new(-1, 0, -1),
						chunk.grid_position + Vector3::new(-1, -1, 0),
						chunk.grid_position + Vector3::new(-1, -1, -1),
					];

					let mut chunks_to_rebuild = vec![];

					for (other_entity, other) in self.world.query_mut::<&Chunk>() {
						if other.voxel_object != chunk.voxel_object {
							continue;
						}

						for position in positions {
							if other.grid_position == position {
								chunks_to_rebuild.push(other_entity);
								break;
							}
						}
					}

					let mut chunks_to_insert = vec![];

					for chunk_entity in chunks_to_rebuild {
						let mut chunk_query = self.world.query_one::<&Chunk>(chunk_entity).expect("chunk we just got");
						let chunk = chunk_query.get().expect("chunk we just got");

						if let Some(chunk_mesh) = ChunkMesh::new(&self.world, chunk, &self.renderer.device) {
							chunks_to_insert.push((chunk_entity, chunk_mesh));
						}
					}

					for (chunk_entity, chunk_mesh) in chunks_to_insert {
						insert_or_spawn_at(&mut self.world, chunk_entity, chunk_mesh);
					}
				}
				SyncEntity::Location(location) => {
					insert_or_spawn_at(
						&mut self.world,
						entity,
						LocationBuffer::new(&self.renderer.device, &location),
					);
					insert_or_spawn_at(&mut self.world, entity, location);
				}
			},
			Message::Event(event) => match event {
				Event::ActiveSector(entity) => {
					self.current_sector = Some(entity);
					let mut to_remove = vec![];
					for (entity, _) in self.world.query::<Without<(), &Sector>>().into_iter() {
						to_remove.push(entity);
					}
					for entity in to_remove {
						let _ = self.world.despawn(entity);
					}
					let sector = self.world.query_one_mut::<&Sector>(entity).expect("sector must exist");
					let details = &*format!("Exploring {}", sector.display_name);
					self.update_discord_rpc(None, Some(details));
				}
				_ => return Err(DisconnectReason::ProtocolViolation),
			},
		}

		Ok(())
	}
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum ClientInitializationError {
	EventLoop(#[from] EventLoopError),
	RendererInitialization(#[from] RendererInitializationError),
	Connection(#[from] ConnectionError),
}
