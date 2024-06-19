use crate::{camera::Camera, connection::Connection, types::Degrees, world::Chunk, world::Sector, world::Voxject};
use log::{error, info};
use nalgebra::{Isometry3, IsometryMatrix3, Point3, Vector3};
use solarscape_shared::messages::clientbound::{AddVoxject, ClientboundMessage, RemoveChunk, SyncChunk, SyncVoxject};
use std::{iter::once, sync::Arc, time::Instant};
use thiserror::Error;
use tokio::{runtime::Handle, spawn};
use wgpu::{
	Backends, Color, CommandEncoderDescriptor, CompositeAlphaMode::Opaque, CreateSurfaceError, Device,
	DeviceDescriptor, Extent3d, Instance, InstanceDescriptor, LoadOp::Clear, Operations,
	PowerPreference::HighPerformance, PresentMode::AutoNoVsync, Queue, RenderPassColorAttachment,
	RenderPassDepthStencilAttachment, RenderPassDescriptor, RequestAdapterOptions, RequestDeviceError, StoreOp::Store,
	Surface, SurfaceConfiguration, SurfaceError, Texture, TextureDescriptor, TextureDimension::D2, TextureFormat,
	TextureFormat::Depth32Float, TextureUsages, TextureView, TextureViewDescriptor,
};
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::EventLoopProxy;
use winit::window::{Window, WindowId};
use winit::{application::ApplicationHandler, dpi::PhysicalSize, error::OsError, event_loop::ActiveEventLoop};

pub struct Client {
	pub name: Box<str>,
	pub event_loop_proxy: EventLoopProxy<Event>,
	pub state: Option<State>,
}

impl ApplicationHandler<Event> for Client {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		self.state = match State::new(self, event_loop) {
			Ok(state) => Some(state),
			Err(error) => {
				error!("{error}");
				event_loop.exit();
				return;
			}
		}
	}

	fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
		if let Some(state) = &mut self.state {
			state.user_event(event_loop, event);
		}
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
		if let Some(state) = &mut self.state {
			state.window_event(event_loop, window_id, event);
		}
	}

	fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, _: DeviceEvent) {
		// Currently unused, however we are going to leave it here as we know we will use it later
	}

	// This should only ever be called on iOS, Android, and Web, none of which we support, so this is untested.
	fn suspended(&mut self, _: &ActiveEventLoop) {
		self.state = None;
	}

	fn exiting(&mut self, _: &ActiveEventLoop) {
		self.state = None;
	}
}

pub enum Event {
	Message(ClientboundMessage),
}

pub struct State {
	window: Arc<Window>,
	surface: Surface<'static>,
	device: Device,
	queue: Queue,
	width: u32,
	height: u32,
	config: SurfaceConfiguration,
	depth_texture_descriptor: TextureDescriptor<'static>,
	depth_texture: Texture,
	depth_texture_view: TextureView,

	connection: Connection,

	camera: Camera,
	sector: Sector,
}

impl State {
	pub fn new(client: &Client, event_loop: &ActiveEventLoop) -> Result<Self, ClientError> {
		let connection_task = spawn(Connection::new(
			format!("ws://localhost:8000/example?name={}", client.name),
			client.event_loop_proxy.clone(),
		));

		let start_time = Instant::now();

		let instance = Instance::new(InstanceDescriptor {
			backends: Backends::VULKAN | Backends::GL,
			..InstanceDescriptor::default()
		});

		let window = Arc::new(
			event_loop.create_window(
				Window::default_attributes()
					.with_maximized(true)
					.with_inner_size(PhysicalSize { width: 854, height: 480 })
					.with_title("Solarscape"),
			)?,
		);

		let surface = instance.create_surface(window.clone())?;

		let adapter = Handle::current()
			.block_on(instance.request_adapter(&RequestAdapterOptions {
				power_preference: HighPerformance,
				compatible_surface: Some(&surface),
				..RequestAdapterOptions::default()
			}))
			.ok_or(ClientError::Adapter)?;

		let (device, queue) = Handle::current().block_on(adapter.request_device(
			&DeviceDescriptor { label: Some("device"), ..DeviceDescriptor::default() },
			None,
		))?;

		let surface_capabilities = surface.get_capabilities(&adapter);

		let surface_format = surface_capabilities
			.formats
			.iter()
			.copied()
			.find(TextureFormat::is_srgb)
			.ok_or(ClientError::SurfaceFormat)?;

		let PhysicalSize { width, height } = window.inner_size();

		let config = SurfaceConfiguration {
			usage: TextureUsages::RENDER_ATTACHMENT,
			format: surface_format,
			width,
			height,
			present_mode: AutoNoVsync,
			desired_maximum_frame_latency: 4,
			alpha_mode: Opaque,
			view_formats: vec![],
		};

		surface.configure(&device, &config);

		// We aren't even done initializing yet, but the sooner we render something the better, it makes launching the
		// game feel more responsive. So lets render a frame real quick. This doesn't make a big difference right now,
		// but it will later when we start doing more work during initialization.
		{
			let output = match surface.get_current_texture() {
				Ok(output) => output,
				Err(error) => {
					return Err(ClientError::Surface(error));
				}
			};

			let view = output.texture.create_view(&TextureViewDescriptor::default());
			let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());

			encoder.begin_render_pass(&RenderPassDescriptor {
				label: Some("render_pass"),
				color_attachments: &[Some(RenderPassColorAttachment {
					ops: Operations { load: Clear(Color::BLACK), store: Store },
					resolve_target: None,
					view: &view,
				})],
				..Default::default()
			});

			queue.submit(once(encoder.finish()));
			output.present();

			window.request_redraw();
		}

		info!("First frame in {:.0?}", Instant::now() - start_time);

		let depth_texture_descriptor = TextureDescriptor {
			label: Some("depth_texture"),
			size: Extent3d { width, height, depth_or_array_layers: 1 },
			mip_level_count: 1,
			sample_count: 1,
			dimension: D2,
			format: Depth32Float,
			usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		};

		let depth_texture_view_descriptor =
			TextureViewDescriptor { label: Some("depth_texture_view"), ..TextureViewDescriptor::default() };

		let mut camera = Camera::new(width as f32 / height as f32, Degrees(90.0), &device);

		camera.set_view(IsometryMatrix3::look_at_rh(
			&Point3::new(8.0, 8.0, 8.0),
			&Point3::origin(),
			&Vector3::y(),
		));

		let sector = Sector::new(&config, &camera, &device, &queue);

		let depth_texture = device.create_texture(&depth_texture_descriptor);
		let depth_texture_view = depth_texture.create_view(&depth_texture_view_descriptor);

		let connection = Handle::current().block_on(connection_task).unwrap().unwrap();
		connection.send(Isometry3::default());

		info!("Ready in {:.0?}", Instant::now() - start_time);

		Ok(Self {
			window,
			surface,
			device,
			queue,
			width,
			height,
			config,
			depth_texture_descriptor,
			depth_texture,
			depth_texture_view,

			connection,

			camera,
			sector,
		})
	}

	fn resized(&mut self, PhysicalSize { width, height }: PhysicalSize<u32>) {
		self.width = width;
		self.height = height;
		self.config.width = width;
		self.config.height = height;
		self.surface.configure(&self.device, &self.config);
		self.depth_texture_descriptor.size = Extent3d { width, height, depth_or_array_layers: 1 };
		self.depth_texture = self.device.create_texture(&self.depth_texture_descriptor);
		self.depth_texture_view = self.depth_texture.create_view(&TextureViewDescriptor::default());
		self.camera.set_aspect(width as f32 / height as f32);
	}

	fn render(&mut self, event_loop: &ActiveEventLoop) {
		let output = match self.surface.get_current_texture() {
			Ok(output) => output,
			Err(error) => {
				error!("{error}");
				event_loop.exit();
				return;
			}
		};

		let view = output.texture.create_view(&TextureViewDescriptor::default());
		let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor::default());

		let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
			color_attachments: &[Some(RenderPassColorAttachment {
				ops: Operations { load: Clear(Color::BLACK), store: Store },
				resolve_target: None,
				view: &view,
			})],
			depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
				view: &self.depth_texture_view,
				depth_ops: Some(Operations { load: Clear(0.0), store: Store }),
				stencil_ops: None,
			}),
			..Default::default()
		});

		self.camera.use_camera(&self.queue, &mut render_pass);
		self.sector.render(&mut render_pass);

		drop(render_pass);

		self.queue.submit(once(encoder.finish()));
		output.present();

		self.window.request_redraw();
	}

	fn user_event(&mut self, _: &ActiveEventLoop, event: Event) {
		match event {
			Event::Message(message) => match message {
				ClientboundMessage::AddVoxject(AddVoxject { id, name }) => {
					info!("Added Voxject {id} {name:?}");
					self.sector.voxjects.insert(
						id,
						Voxject {
							id,
							name,
							location: Isometry3::default(),
							chunks: Default::default(),
							dependent_chunks: Default::default(),
						},
					);
				}
				ClientboundMessage::SyncVoxject(SyncVoxject { id, location }) => {
					self.sector.voxjects.get_mut(&id).unwrap().location = location;
				}
				ClientboundMessage::SyncChunk(SyncChunk { coordinates, materials, densities }) => {
					let chunk = Chunk { coordinates, materials, densities, mesh: None };
					let voxject = self.sector.voxjects.get_mut(&coordinates.voxject).unwrap();
					voxject.add_chunk(&self.device, chunk);
				}
				ClientboundMessage::RemoveChunk(RemoveChunk(coordinates)) => {
					let voxject = self.sector.voxjects.get_mut(&coordinates.voxject).unwrap();
					voxject.remove_chunk(&self.device, coordinates);
				}
			},
		}
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
		if window_id != self.window.id() {
			return;
		}

		match event {
			WindowEvent::CloseRequested | WindowEvent::Destroyed => event_loop.exit(),
			WindowEvent::Resized(new_size) => self.resized(new_size),
			WindowEvent::RedrawRequested => self.render(event_loop),
			_ => {}
		}
	}
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum ClientError {
	Os(#[from] OsError),

	CreateSurface(#[from] CreateSurfaceError),

	Surface(#[from] SurfaceError),

	#[error("unable to find suitable adapter")]
	Adapter,

	RequestDevice(#[from] RequestDeviceError),

	#[error("unable to find suitable surface format")]
	SurfaceFormat,
}
