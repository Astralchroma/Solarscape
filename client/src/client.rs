use crate::{connection::Connection, world::Chunk, world::Sector, world::Voxject};
use bytemuck::cast_slice;
use core::f32;
use image::GenericImageView;
use log::{error, info};
use nalgebra::{Isometry3, Perspective3};
use solarscape_shared::messages::clientbound::{AddVoxject, ClientboundMessage, RemoveChunk, SyncChunk, SyncVoxject};
use std::{iter::once, sync::Arc, time::Instant};
use thiserror::Error;
use tokio::{runtime::Handle, spawn};
use wgpu::{
	include_wgsl, vertex_attr_array, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry,
	BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Color, ColorTargetState,
	ColorWrites, CommandEncoderDescriptor, CompareFunction::LessEqual, CompositeAlphaMode::Opaque, CreateSurfaceError,
	DepthStencilState, Device, DeviceDescriptor, Extent3d, Face::Back, Features, FragmentState, FrontFace,
	ImageCopyTexture, ImageDataLayout, Instance, InstanceDescriptor, Limits, LoadOp::Clear, MultisampleState,
	Operations, Origin3d, PipelineCompilationOptions, PipelineLayoutDescriptor, PolygonMode,
	PowerPreference::HighPerformance, PresentMode::AutoNoVsync, PrimitiveState, PrimitiveTopology, PushConstantRange,
	Queue, RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipeline,
	RenderPipelineDescriptor, RequestAdapterOptions, RequestDeviceError, SamplerBindingType::NonFiltering,
	SamplerDescriptor, ShaderStages, StoreOp::Store, Surface, SurfaceConfiguration, SurfaceError, Texture,
	TextureAspect::All, TextureDescriptor, TextureDimension, TextureDimension::D2, TextureFormat,
	TextureFormat::Depth32Float, TextureFormat::Rgba8UnormSrgb, TextureSampleType, TextureUsages, TextureView,
	TextureViewDescriptor, TextureViewDimension, VertexBufferLayout, VertexState, VertexStepMode,
};
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::{CursorGrabMode::Confined, CursorGrabMode::Locked, Window, WindowId};
use winit::{application::ApplicationHandler, dpi::LogicalPosition, dpi::PhysicalSize, error::OsError};

pub struct Client {
	pub name: Box<str>,
	pub sector_endpoint: Box<str>,
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

	fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
		if let Some(state) = &mut self.state {
			state.device_event(event);
		}
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

	frame_start: Instant,

	perspective: Perspective3<f32>,

	chunk_pipeline: RenderPipeline,
	terrain_textures_bind_group: BindGroup,

	sector: Sector,
}

impl State {
	pub fn new(client: &Client, event_loop: &ActiveEventLoop) -> Result<Self, ClientError> {
		let connection_task = spawn(Connection::new(
			format!("{}?name={}", client.sector_endpoint, client.name),
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
					.with_inner_size(PhysicalSize {
						width: 854,
						height: 480,
					})
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
			&DeviceDescriptor {
				label: Some("device"),
				required_features: Features::PUSH_CONSTANTS,
				required_limits: Limits {
					max_push_constant_size: 64,
					..Limits::default()
				},
				..DeviceDescriptor::default()
			},
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

		window
			.set_cursor_grab(Confined)
			.or_else(|_| window.set_cursor_grab(Locked));

		window.set_cursor_visible(false);
		window.set_cursor_position(LogicalPosition {
			x: width as f32 / 2.0,
			y: height as f32 / 2.0,
		});

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

		let frame_start = Instant::now();

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
					ops: Operations {
						load: Clear(Color::BLACK),
						store: Store,
					},
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

		let terrain_textures_image = image::load_from_memory(include_bytes!("terrain_textures.png"))
			.expect("terrain_textures.png must be valid");
		let terrain_textures_rgba8 = terrain_textures_image.to_rgba8();
		let (terrain_textures_width, terrain_textures_height) = terrain_textures_image.dimensions();
		let terrain_textures_size = Extent3d {
			width: terrain_textures_width,
			height: terrain_textures_height,
			depth_or_array_layers: 1,
		};

		let terrain_textures = device.create_texture(&TextureDescriptor {
			label: Some("sector.terrain_textures"),
			size: terrain_textures_size,
			mip_level_count: 1,
			sample_count: 1,
			dimension: TextureDimension::D2,
			format: Rgba8UnormSrgb,
			usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
			view_formats: &[],
		});

		queue.write_texture(
			ImageCopyTexture {
				texture: &terrain_textures,
				mip_level: 0,
				origin: Origin3d::ZERO,
				aspect: All,
			},
			&terrain_textures_rgba8,
			ImageDataLayout {
				offset: 0,
				bytes_per_row: Some(4 * terrain_textures_width),
				rows_per_image: Some(terrain_textures_height),
			},
			terrain_textures_size,
		);

		let terrain_textures_view = terrain_textures.create_view(&TextureViewDescriptor::default());
		let terrain_textures_sampler = device.create_sampler(&SamplerDescriptor::default());

		let terrain_textures_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
			label: Some("sector.terrain_textures_bind_group_layout"),
			entries: &[
				BindGroupLayoutEntry {
					binding: 0,
					visibility: ShaderStages::FRAGMENT,
					ty: BindingType::Texture {
						sample_type: TextureSampleType::Float { filterable: false },
						view_dimension: TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
				BindGroupLayoutEntry {
					binding: 1,
					visibility: ShaderStages::FRAGMENT,
					ty: BindingType::Sampler(NonFiltering),
					count: None,
				},
			],
		});

		let terrain_textures_bind_group = device.create_bind_group(&BindGroupDescriptor {
			label: Some("sector.terrain_textures_bind_group"),
			layout: &terrain_textures_bind_group_layout,
			entries: &[
				BindGroupEntry {
					binding: 0,
					resource: BindingResource::TextureView(&terrain_textures_view),
				},
				BindGroupEntry {
					binding: 1,
					resource: BindingResource::Sampler(&terrain_textures_sampler),
				},
			],
		});

		let chunk_shader = device.create_shader_module(include_wgsl!("chunk.wgsl"));

		let chunk_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
			label: Some("sector.chunk_pipeline_layout"),
			bind_group_layouts: &[&terrain_textures_bind_group_layout],
			push_constant_ranges: &[PushConstantRange {
				stages: ShaderStages::VERTEX,
				range: 0..64,
			}],
		});

		let chunk_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
			label: Some("sector.chunk_pipeline"),
			layout: Some(&chunk_pipeline_layout),
			vertex: VertexState {
				module: &chunk_shader,
				entry_point: "vertex",
				compilation_options: PipelineCompilationOptions::default(),
				buffers: &[
					VertexBufferLayout {
						array_stride: 32,
						step_mode: VertexStepMode::Vertex,
						attributes: &vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Uint8x2, 3 => Uint8x2, 4 => Float32],
					},
					VertexBufferLayout {
						array_stride: 16,
						step_mode: VertexStepMode::Instance,
						attributes: &vertex_attr_array![5 => Float32x3, 6 => Float32],
					},
				],
			},
			primitive: PrimitiveState {
				topology: PrimitiveTopology::TriangleList,
				strip_index_format: None,
				front_face: FrontFace::Ccw,
				cull_mode: Some(Back),
				unclipped_depth: false,
				polygon_mode: PolygonMode::Fill,
				conservative: false,
			},
			depth_stencil: Some(DepthStencilState {
				format: Depth32Float,
				depth_write_enabled: true,
				depth_compare: LessEqual,
				stencil: Default::default(),
				bias: Default::default(),
			}),
			multisample: MultisampleState {
				count: 1,
				mask: !0,
				alpha_to_coverage_enabled: false,
			},
			fragment: Some(FragmentState {
				module: &chunk_shader,
				entry_point: "fragment",
				compilation_options: PipelineCompilationOptions::default(),
				targets: &[Some(ColorTargetState {
					format: config.format,
					blend: Some(BlendState::REPLACE),
					write_mask: ColorWrites::ALL,
				})],
			}),
			multiview: None,
			cache: None,
		});

		let depth_texture_descriptor = TextureDescriptor {
			label: Some("depth_texture"),
			size: Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: D2,
			format: Depth32Float,
			usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		};

		let depth_texture_view_descriptor = TextureViewDescriptor {
			label: Some("depth_texture_view"),
			..TextureViewDescriptor::default()
		};

		let depth_texture = device.create_texture(&depth_texture_descriptor);
		let depth_texture_view = depth_texture.create_view(&depth_texture_view_descriptor);

		let connection = Handle::current().block_on(connection_task).unwrap().unwrap();
		connection.send(Isometry3::default());

		let sector = Sector::new(connection);

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

			frame_start,

			perspective: Perspective3::new(width as f32 / height as f32, f32::to_radians(90.0), 0.05, f32::MAX),

			chunk_pipeline,
			terrain_textures_bind_group,

			sector,
		})
	}

	fn resized(&mut self, PhysicalSize { width, height }: PhysicalSize<u32>) {
		self.width = width;
		self.height = height;
		self.config.width = width;
		self.config.height = height;
		self.surface.configure(&self.device, &self.config);
		self.depth_texture_descriptor.size = Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		};
		self.depth_texture = self.device.create_texture(&self.depth_texture_descriptor);
		self.depth_texture_view = self.depth_texture.create_view(&TextureViewDescriptor::default());
		self.perspective.set_aspect(width as f32 / height as f32);
	}

	fn render(&mut self, event_loop: &ActiveEventLoop) {
		let frame_start = Instant::now();
		let delta = (frame_start - self.frame_start).as_secs_f32();
		self.frame_start = frame_start;

		self.sector.player.frame(delta);

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
				ops: Operations {
					load: Clear(Color::BLACK),
					store: Store,
				},
				resolve_target: None,
				view: &view,
			})],
			depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
				view: &self.depth_texture_view,
				depth_ops: Some(Operations {
					load: Clear(1.0),
					store: Store,
				}),
				stencil_ops: None,
			}),
			..Default::default()
		});

		render_pass.set_pipeline(&self.chunk_pipeline);

		let camera_matrix = self.perspective.to_homogeneous() * self.sector.player.location.to_homogeneous();
		render_pass.set_push_constants(ShaderStages::VERTEX, 0, cast_slice(&[camera_matrix]));

		render_pass.set_bind_group(0, &self.terrain_textures_bind_group, &[]);
		self.sector
			.voxjects
			.iter()
			.flat_map(|(_, voxject)| voxject.chunks.iter().flat_map(|level| level.values()))
			.filter(|chunk| *chunk.coordinates.level == 0)
			.filter_map(|chunk| chunk.mesh.as_ref())
			.for_each(|chunk| {
				render_pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
				render_pass.set_vertex_buffer(1, chunk.instance_buffer.slice(..));
				render_pass.draw(0..chunk.vertex_count, 0..1);
			});

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
				ClientboundMessage::SyncChunk(SyncChunk {
					coordinates,
					materials,
					densities,
				}) => {
					let chunk = Chunk {
						coordinates,
						materials,
						densities,
						mesh: None,
					};
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

		self.sector.player.handle_window_event(event);
	}

	fn device_event(&mut self, event: DeviceEvent) {
		self.sector.player.handle_device_event(event);

		self.window.set_cursor_visible(false);
		self.window.set_cursor_position(LogicalPosition {
			x: self.width as f32 / 2.0,
			y: self.height as f32 / 2.0,
		});
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
