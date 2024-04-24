#![warn(clippy::nursery)]

use crate::connection::{Connection, Event};
use crate::{camera::Camera, types::Degrees, world::Chunk, world::Sector, world::Voxject};
use log::{info, LevelFilter::Trace};
use nalgebra::{Isometry3, IsometryMatrix3, Point3, Vector3};
use solarscape_shared::messages::clientbound::{AddVoxject, ClientboundMessage, RemoveChunk, SyncChunk, SyncVoxject};
use solarscape_shared::StdLogger;
use std::{borrow::Cow, env, error::Error, iter::once, time::Instant};
use thiserror::Error;
use tokio::runtime::Builder;
use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};
use wgpu::{
	Backends, Color, CommandEncoderDescriptor, CompositeAlphaMode::Opaque, DeviceDescriptor, Dx12Compiler, Extent3d,
	Features, Gles3MinorVersion::Version0, Instance, InstanceDescriptor, InstanceFlags, LoadOp::Clear, Operations,
	PowerPreference::HighPerformance, PresentMode::AutoNoVsync, RenderPassColorAttachment,
	RenderPassDepthStencilAttachment, RenderPassDescriptor, RequestAdapterOptions, StoreOp::Store,
	SurfaceConfiguration, TextureDescriptor, TextureDimension::D2, TextureFormat, TextureFormat::Depth32Float,
	TextureUsages, TextureViewDescriptor,
};
use winit::event::WindowEvent::{CloseRequested, Destroyed, RedrawRequested, Resized};
use winit::event::{Event::AboutToWait, Event::UserEvent, Event::WindowEvent};
use winit::{dpi::PhysicalSize, event_loop::EventLoopBuilder, window::WindowBuilder};

mod camera;
mod connection;
mod data;
mod types;
mod world;

fn main() -> Result<(), Box<dyn Error>> {
	let start_time = Instant::now();

	log::set_logger(&StdLogger).expect("logger must not already be set");
	log::set_max_level(Trace);

	info!("Solarscape (Client) v{}", env!("CARGO_PKG_VERSION"));

	info!("Command Line: {:?}", env::args().collect::<Vec<_>>().join(" "));
	info!("Working Directory: {:?}", env::current_dir()?);

	let name = env::args().nth(1).expect("name").into_boxed_str();

	info!("Setting name to {name:?}");

	let runtime = Builder::new_multi_thread()
		.thread_name("io-worker")
		.worker_threads(1)
		.enable_io()
		.enable_time()
		.build()?;

	info!("Started Async Runtime with 1 worker thread");

	let event_loop = EventLoopBuilder::with_user_event().build()?;

	let connection_task = runtime.spawn(Connection::new(
		format!("ws://localhost:8000/example?name={name}"),
		event_loop.create_proxy(),
	));

	let window = WindowBuilder::new()
		.with_active(true)
		.with_inner_size(PhysicalSize::new(1280, 720))
		.with_maximized(true)
		.with_title("Solarscape")
		.build(&event_loop)?;

	let instance = Instance::new(InstanceDescriptor {
		backends: Backends::VULKAN | Backends::GL,
		flags: InstanceFlags::default(),
		dx12_shader_compiler: Dx12Compiler::default(),
		gles_minor_version: Version0,
	});

	let surface = instance.create_surface(&window)?;

	let adapter = runtime
		.block_on(instance.request_adapter(&RequestAdapterOptions {
			compatible_surface: Some(&surface),
			force_fallback_adapter: false,
			power_preference: HighPerformance,
		}))
		.ok_or(NoAdapter)?;

	let (device, queue) = runtime.block_on(adapter.request_device(
		&DeviceDescriptor {
			label: Some("device"),
			required_features: Features::empty(),
			required_limits: adapter.limits(),
		},
		None,
	))?;

	let surface_capabilities = surface.get_capabilities(&adapter);

	let surface_format = surface_capabilities
		.formats
		.iter()
		.copied()
		.find(TextureFormat::is_srgb)
		.ok_or(NoSurfaceFormat)?;

	let PhysicalSize { mut width, mut height } = window.inner_size();

	let mut config = SurfaceConfiguration {
		usage: TextureUsages::RENDER_ATTACHMENT,
		format: surface_format,
		width,
		height,
		present_mode: AutoNoVsync,
		desired_maximum_frame_latency: 0,
		alpha_mode: Opaque,
		view_formats: vec![],
	};

	surface.configure(&device, &config);

	let mut depth_texture_descriptor = TextureDescriptor {
		label: Some("depth_buffer"),
		size: Extent3d { width, height, depth_or_array_layers: 1 },
		mip_level_count: 1,
		sample_count: 1,
		dimension: D2,
		format: Depth32Float,
		usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
		view_formats: &[],
	};

	let mut depth_texture = device.create_texture(&depth_texture_descriptor);
	let mut depth_texture_view = depth_texture.create_view(&TextureViewDescriptor::default());

	let mut camera = Camera::new(width as f32 / height as f32, Degrees(90.0), &device);

	camera.set_view(IsometryMatrix3::look_at_rh(
		&Point3::new(8.0, 8.0, 8.0),
		&Point3::origin(),
		&Vector3::y(),
	));

	let connection = runtime.block_on(connection_task).unwrap().unwrap();
	let mut sector = Sector::new(&config, &camera, &device, &queue);

	let end_time = Instant::now();
	let load_time = end_time - start_time;
	info!("Ready! {load_time:.0?}");

	connection.send(Isometry3::default());

	event_loop.run(|event, control_flow| match event {
		WindowEvent { event, .. } => match event {
			Resized(new_size) => {
				width = new_size.width;
				height = new_size.height;
				config.width = width;
				config.height = height;
				surface.configure(&device, &config);
				depth_texture_descriptor.size = Extent3d { width, height, depth_or_array_layers: 1 };
				depth_texture = device.create_texture(&depth_texture_descriptor);
				depth_texture_view = depth_texture.create_view(&TextureViewDescriptor::default());
				camera.set_aspect(width as f32 / height as f32);
			}
			CloseRequested | Destroyed => control_flow.exit(),
			RedrawRequested => {
				let output = if let Ok(output) = surface.get_current_texture() {
					output
				} else {
					config.width = window.inner_size().width;
					config.height = window.inner_size().height;
					surface.configure(&device, &config);
					surface.get_current_texture().expect("no texture?")
				};

				let view = output.texture.create_view(&TextureViewDescriptor::default());
				let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());

				{
					let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
						color_attachments: &[Some(RenderPassColorAttachment {
							ops: Operations { load: Clear(Color::BLACK), store: Store },
							resolve_target: None,
							view: &view,
						})],
						depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
							view: &depth_texture_view,
							depth_ops: Some(Operations { load: Clear(0.0), store: Store }),
							stencil_ops: None,
						}),
						..Default::default()
					});

					camera.use_camera(&queue, &mut render_pass);
					sector.render(&mut render_pass);
				}

				queue.submit(once(encoder.finish()));
				output.present();
			}
			_ => {}
		},
		UserEvent(event) => match event {
			Event::Message(message) => match message {
				ClientboundMessage::AddVoxject(AddVoxject { voxject: voxject_index, name }) => {
					info!("Added Voxject {voxject_index} {name:?}");
					sector.voxjects.insert(
						voxject_index,
						Voxject {
							name,
							location: Isometry3::default(),
							chunks: Default::default(),
							dependent_chunks: Default::default(),
						},
					);
				}
				ClientboundMessage::SyncVoxject(SyncVoxject { voxject: voxject_index, location }) => {
					sector.voxjects[voxject_index].location = location;
				}
				ClientboundMessage::SyncChunk(SyncChunk { voxject, data }) => {
					let chunk = Chunk { data, mesh: None };
					let voxject = &mut sector.voxjects[voxject];
					voxject.add_chunk(&device, chunk);
				}
				ClientboundMessage::RemoveChunk(RemoveChunk { voxject, coordinates: grid_coordinates }) => {
					let voxject = &mut sector.voxjects[voxject];
					voxject.remove_chunk(&device, grid_coordinates);
				}
			},
		},
		AboutToWait => {
			window.request_redraw();
		}
		_ => {}
	})?;

	connection.disconnect(Some(CloseFrame {
		code: CloseCode::Away,
		reason: Cow::from("Disconnected"),
	}));

	Ok(())
}

#[derive(Debug, Error)]
#[error("no adapter found")]
struct NoAdapter;

#[derive(Debug, Error)]
#[error("no surface format found")]
struct NoSurfaceFormat;
