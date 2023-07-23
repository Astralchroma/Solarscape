#![deny(clippy::unwrap_used)]

mod chunk;
mod object;
mod sector;
mod world;

use crate::{chunk::Chunk, object::Object, world::World};
use anyhow::Result;
use log::{error, info};
use solarscape_shared::{
	default,
	io::{PacketRead, PacketWrite},
	protocol::{Clientbound, Serverbound, PROTOCOL_VERSION},
	shared_main,
};
use std::{convert::Infallible, iter, sync::Arc};
use tokio::net::TcpStream;
use wgpu::{
	Color, CommandEncoderDescriptor, DeviceDescriptor, Instance, InstanceDescriptor, LoadOp::Clear, Operations,
	PowerPreference::HighPerformance, RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions,
	SurfaceConfiguration, SurfaceError, TextureUsages, TextureViewDescriptor,
};
use winit::{
	dpi::PhysicalSize,
	event::{Event, WindowEvent},
	event_loop::{ControlFlow, EventLoop},
	window::WindowBuilder,
};

fn main() -> Result<()> {
	let runtime = shared_main()?;

	let world = World::new();

	runtime.spawn(handle_connection(world));

	let event_loop = EventLoop::new();
	let window = WindowBuilder::new()
		.with_inner_size(PhysicalSize::new(960, 540))
		.with_title("Solarscape")
		.build(&event_loop)?;

	let instance = Instance::new(InstanceDescriptor::default());

	let surface = unsafe { instance.create_surface(&window) }?;

	let adapter = runtime
		.block_on(instance.request_adapter(&RequestAdapterOptions {
			power_preference: HighPerformance,
			compatible_surface: Some(&surface),
			..default()
		}))
		.expect("should be able to request adapter");

	let (device, queue) = runtime.block_on(adapter.request_device(&DeviceDescriptor::default(), None))?;

	let surface_capabilities = surface.get_capabilities(&adapter);

	let surface_format = surface_capabilities
		.formats
		.iter()
		.copied()
		.find(|format| format.is_srgb())
		.unwrap_or(surface_capabilities.formats[0]);

	let mut size = window.inner_size();

	let mut config = SurfaceConfiguration {
		usage: TextureUsages::RENDER_ATTACHMENT,
		format: surface_format,
		width: size.width,
		height: size.height,
		present_mode: surface_capabilities.present_modes[0],
		alpha_mode: surface_capabilities.alpha_modes[0],
		view_formats: vec![],
	};

	surface.configure(&device, &config);

	event_loop.run(move |event, _, control_flow| {
		let mut resize = |new_size: PhysicalSize<u32>| {
			size = new_size;
			config.width = size.width;
			config.height = size.height;
			surface.configure(&device, &config);
		};

		match event {
			Event::WindowEvent { ref event, window_id } if window_id == window.id() => match event {
				WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
				WindowEvent::Resized(new_size) => resize(*new_size),
				WindowEvent::ScaleFactorChanged { new_inner_size, .. } => resize(**new_inner_size),
				_ => {}
			},
			Event::RedrawRequested(window_id) if window_id == window.id() => {
				let output = match surface.get_current_texture() {
					Err(SurfaceError::Lost) => return surface.configure(&device, &config),
					Err(SurfaceError::OutOfMemory | SurfaceError::Outdated) => {
						return *control_flow = ControlFlow::Exit
					}
					Err(e) => return error!("{e:?}"),
					Ok(value) => value,
				};

				let view = output.texture.create_view(&TextureViewDescriptor::default());
				let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());

				{
					let render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
						color_attachments: &[Some(RenderPassColorAttachment {
							view: &view,
							resolve_target: None,
							ops: Operations {
								load: Clear(Color::BLACK),
								store: true,
							},
						})],
						..default()
					});
				}

				queue.submit(iter::once(encoder.finish()));
				output.present();
			}
			Event::MainEventsCleared => window.request_redraw(),
			_ => {}
		}
	});
}

async fn handle_connection(world: Arc<World>) -> Result<Infallible> {
	let mut stream = TcpStream::connect("[::1]:23500").await?;
	info!("Connecting to [::1]:23500");

	stream
		.write_packet(&Serverbound::Hello {
			major_version: *PROTOCOL_VERSION,
		})
		.await?;

	loop {
		use Clientbound::*;

		match stream.read_packet().await? {
			Disconnected { reason } => panic!("Disconnected: {reason:?}"),
			SyncSector { name, display_name } => world.add_sector(name, display_name).await,
			ActiveSector { name } => world.set_active_sector(name).await,
			AddObject { object_id } => {
				info!("Added object {object_id}");

				world
					.active_sector()
					.await
					.objects
					.write()
					.await
					.insert(object_id, Object::new(object_id));
			}
			SyncChunk {
				object_id,
				grid_position,
				data,
			} => {
				info!("Added chunk {grid_position:?} to {object_id}");

				let chunk = Chunk { grid_position, data };
				world
					.active_sector()
					.await
					.objects
					.read()
					.await
					.get(&object_id)
					.expect("object_id of chunk should exist")
					.chunks
					.write()
					.await
					.insert(grid_position, chunk);
			}
		}
	}
}
