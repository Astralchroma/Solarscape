#![warn(clippy::nursery)]

use crate::{connection::Connection, connection::Event, types::Degrees, world::Chunk, world::Voxject, world::World};
use bytemuck::cast_slice;
use camera::Camera;
use log::{info, LevelFilter::Trace};
use nalgebra::{convert, Isometry3, IsometryMatrix3, Point3, Similarity3, Translation, Vector3};
use solarscape_shared::messages::clientbound::{AddVoxject, ClientboundMessage, SyncChunk, SyncVoxject};
use solarscape_shared::StdLogger;
use std::{borrow::Cow, env, error::Error, iter::once, mem::size_of, time::Instant};
use thiserror::Error;
use tokio::runtime::Builder;
use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};
use wgpu::{
	include_wgsl, util::BufferInitDescriptor, util::DeviceExt, Backends, BlendState, BufferUsages, Color,
	ColorTargetState, ColorWrites, CommandEncoderDescriptor, CompositeAlphaMode::Opaque, DeviceDescriptor,
	Dx12Compiler, Features, FragmentState, FrontFace, Gles3MinorVersion::Version0, IndexFormat, Instance,
	InstanceDescriptor, InstanceFlags, LoadOp::Clear, MultisampleState, Operations, PipelineLayoutDescriptor,
	PolygonMode, PowerPreference::HighPerformance, PresentMode::AutoNoVsync, PrimitiveState, PrimitiveTopology,
	RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, RequestAdapterOptions, StoreOp::Store,
	SurfaceConfiguration, TextureFormat, TextureUsages, TextureViewDescriptor, VertexAttribute, VertexBufferLayout,
	VertexFormat, VertexState, VertexStepMode,
};
use winit::event::WindowEvent::{CloseRequested, Destroyed, RedrawRequested, Resized};
use winit::event::{Event::AboutToWait, Event::UserEvent, Event::WindowEvent};
use winit::{dpi::PhysicalSize, event_loop::EventLoopBuilder, window::WindowBuilder};

mod camera;
mod connection;
mod types;
mod world;

#[rustfmt::skip]
pub const CHUNK_DEBUG_VERTICES: [f32; 24] = [
	0.0, 0.0, 0.0,
	0.0, 0.0, 1.0,
	0.0, 1.0, 0.0,
	0.0, 1.0, 1.0,
	1.0, 0.0, 0.0,
	1.0, 0.0, 1.0,
	1.0, 1.0, 0.0,
	1.0, 1.0, 1.0
];

#[rustfmt::skip]
pub const CHUNK_DEBUG_INDICES: [u16; 19] = [
	0, 1, 3, 2, 0, 4, 5, 7, 6, 4, 0xFFFF, 1, 5, 0xFFFF, 2, 6, 0xFFFF, 3, 7
];

fn main() -> Result<(), Box<dyn Error>> {
	let start_time = Instant::now();

	log::set_logger(&StdLogger).expect("logger must not already be set");
	log::set_max_level(Trace);

	info!("Solarscape (Client) v{}", env!("CARGO_PKG_VERSION"));

	info!("Command Line: {:?}", env::args().collect::<Vec<_>>().join(" "));
	info!("Working Directory: {:?}", env::current_dir()?);

	let runtime = Builder::new_multi_thread()
		.thread_name("io-worker")
		.worker_threads(1)
		.enable_io()
		.enable_time()
		.build()?;

	info!("Started Async Runtime with 1 worker thread");

	let event_loop = EventLoopBuilder::with_user_event().build()?;

	let connection_task = runtime.spawn(Connection::new(
		"ws://localhost:8000/example",
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

	let mut camera = Camera::new(width as f32 / height as f32, Degrees(90.0), &device);
	camera.set_view(IsometryMatrix3::look_at_rh(
		&Point3::new(512.0, 0.0, 0.0),
		&Point3::origin(),
		&Vector3::y(),
	));

	let chunk_debug_shader = device.create_shader_module(include_wgsl!("chunk_debug.wgsl"));

	let chunk_debug_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
		label: None,
		bind_group_layouts: &[camera.bind_group_layout()],
		push_constant_ranges: &[],
	});

	let chunk_debug_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
		label: None,
		layout: Some(&chunk_debug_pipeline_layout),
		vertex: VertexState {
			module: &chunk_debug_shader,
			entry_point: "vertex",
			buffers: &[
				VertexBufferLayout {
					array_stride: size_of::<f32>() as u64 * 3,
					step_mode: VertexStepMode::Vertex,
					attributes: &[VertexAttribute { offset: 0, shader_location: 0, format: VertexFormat::Float32x3 }],
				},
				VertexBufferLayout {
					array_stride: (size_of::<f32>() * 4 * 4) as u64,
					step_mode: VertexStepMode::Instance,
					attributes: &[
						VertexAttribute { offset: 0, shader_location: 1, format: VertexFormat::Float32x4 },
						VertexAttribute {
							offset: (size_of::<f32>() * 4) as u64,
							shader_location: 2,
							format: VertexFormat::Float32x4,
						},
						VertexAttribute {
							offset: (size_of::<f32>() * 4 * 2) as u64,
							shader_location: 3,
							format: VertexFormat::Float32x4,
						},
						VertexAttribute {
							offset: (size_of::<f32>() * 4 * 3) as u64,
							shader_location: 4,
							format: VertexFormat::Float32x4,
						},
					],
				},
			],
		},
		primitive: PrimitiveState {
			topology: PrimitiveTopology::LineStrip,
			strip_index_format: Some(IndexFormat::Uint16),
			front_face: FrontFace::Ccw,
			cull_mode: None,
			unclipped_depth: false,
			polygon_mode: PolygonMode::Fill,
			conservative: false,
		},
		depth_stencil: None,
		multisample: MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
		fragment: Some(FragmentState {
			module: &chunk_debug_shader,
			entry_point: "fragment",
			targets: &[Some(ColorTargetState {
				format: config.format,
				blend: Some(BlendState::REPLACE),
				write_mask: ColorWrites::ALL,
			})],
		}),
		multiview: None,
	});

	let chunk_debug_vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
		label: None,
		contents: cast_slice(&CHUNK_DEBUG_VERTICES),
		usage: BufferUsages::VERTEX,
	});

	let chunk_debug_index_buffer = device.create_buffer_init(&BufferInitDescriptor {
		label: None,
		contents: cast_slice(&CHUNK_DEBUG_INDICES),
		usage: BufferUsages::INDEX,
	});

	let mut chunk_debug_instance_buffer =
		device.create_buffer_init(&BufferInitDescriptor { label: None, contents: &[], usage: BufferUsages::VERTEX });

	let mut chunk_count = 0;
	let mut chunks_changed = false;

	let connection = runtime.block_on(connection_task).unwrap().unwrap();
	let mut world = World { voxjects: vec![] };

	let end_time = Instant::now();
	let load_time = end_time - start_time;
	info!("Ready! {load_time:?}");

	connection.send(Isometry3::default());

	event_loop.run(|event, control_flow| match event {
		WindowEvent { event, .. } => match event {
			Resized(new_size) => {
				width = new_size.width;
				height = new_size.height;
				config.width = width;
				config.height = height;
				surface.configure(&device, &config);
				camera.set_aspect(width as f32 / height as f32);
			}
			CloseRequested | Destroyed => control_flow.exit(),
			RedrawRequested => {
				if chunks_changed {
					// TODO: costly, dumb, and jank, good thing its temporary!
					let chunk_debug_instances = world
						.voxjects
						.iter()
						.flat_map(|voxject| {
							voxject.chunks.iter().enumerate().flat_map(move |(level, chunks)| {
								chunks.keys().map(move |grid_position| {
									let position: Vector3<f32> =
										convert(grid_position.map(|value| value as i64 * (16 << level)));
									Similarity3::from_parts(
										Translation::from(position),
										voxject.location.rotation,
										(16u64 << level) as f32,
									)
									.to_homogeneous()
								})
							})
						})
						.collect::<Vec<_>>();

					chunk_count = chunk_debug_instances.len() as u32;
					info!("Updated chunk_debug_buffer with {chunk_count} chunks");

					chunk_debug_instance_buffer = device.create_buffer_init(&BufferInitDescriptor {
						label: None,
						contents: cast_slice(&chunk_debug_instances),
						usage: BufferUsages::VERTEX,
					});

					chunks_changed = false;
				}

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
						..Default::default()
					});

					camera.use_camera(&queue, &mut render_pass);

					render_pass.set_pipeline(&chunk_debug_pipeline);
					render_pass.set_vertex_buffer(0, chunk_debug_vertex_buffer.slice(..));
					render_pass.set_vertex_buffer(1, chunk_debug_instance_buffer.slice(..));
					render_pass.set_index_buffer(chunk_debug_index_buffer.slice(..), IndexFormat::Uint16);
					render_pass.draw_indexed(0..CHUNK_DEBUG_INDICES.len() as u32, 0, 0..chunk_count);
				}

				queue.submit(once(encoder.finish()));
				output.present();
			}
			_ => {}
		},
		UserEvent(event) => match event {
			Event::Message(message) => match message {
				ClientboundMessage::AddVoxject(AddVoxject { voxject_index, name }) => {
					info!("Added Voxject {voxject_index} {name:?}");
					world.voxjects.insert(
						voxject_index,
						Voxject { name, location: Isometry3::default(), chunks: Default::default() },
					);
				}
				ClientboundMessage::SyncVoxject(SyncVoxject { voxject_index, location }) => {
					world.voxjects[voxject_index].location = location;
				}
				ClientboundMessage::SyncChunk(SyncChunk { voxject_index, level, coordinates, .. }) => {
					world.voxjects[voxject_index].chunks[level as usize].insert(coordinates, Chunk);
					chunks_changed = true;
				}
			},
		},
		AboutToWait => {
			window.request_redraw();
		}
		_ => {}
	})?;

	connection.close(Some(CloseFrame {
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
