use crate::{chunk::Chunk, connection::ClientConnection, object::Object, orbit_camera::OrbitCamera, sector::Sector};
use anyhow::Result;
use hecs::{Entity, Without, World};
use solarscape_shared::protocol::{encode, DisconnectReason, Event, Message, SyncEntity};
use std::{iter, mem, mem::size_of};
use tokio::{runtime::Runtime, sync::mpsc::error::TryRecvError};
use wgpu::VertexFormat::{Float32, Uint32};
use wgpu::{
	include_wgsl, Backends, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType::Buffer, BlendState,
	BufferAddress, BufferBindingType::Uniform, Color, ColorTargetState, ColorWrites, CommandEncoderDescriptor,
	CompareFunction::Greater, CompositeAlphaMode::Auto, DepthBiasState, DepthStencilState, Device, DeviceDescriptor,
	Extent3d, Face::Back, Features, FragmentState, FrontFace::Ccw, Gles3MinorVersion, Instance, InstanceDescriptor,
	LoadOp::Clear, MultisampleState, Operations, PipelineLayoutDescriptor, PolygonMode::Fill,
	PowerPreference::HighPerformance, PresentMode::AutoVsync, PrimitiveState, PrimitiveTopology::TriangleList, Queue,
	RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipeline,
	RenderPipelineDescriptor, RequestAdapterOptions, ShaderStages, StencilState, StoreOp::Store, Surface,
	SurfaceConfiguration, Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat::Depth32Float,
	TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension, VertexAttribute, VertexBufferLayout,
	VertexFormat::Float32x3, VertexState, VertexStepMode,
};
use winit::event::Event::{AboutToWait, DeviceEvent, WindowEvent};
use winit::event::WindowEvent::{CloseRequested, Destroyed, MouseInput, MouseWheel, RedrawRequested, Resized};
use winit::window::{Window, WindowBuilder};
use winit::{dpi::PhysicalSize, error::EventLoopError, event_loop::EventLoop};

pub struct Client {
	window: Window,
	surface: Surface,
	device: Device,
	queue: Queue,
	size: PhysicalSize<u32>,
	config: SurfaceConfiguration,
	trans_pipeline: RenderPipeline,
	depth_texture: Texture,
	depth_view: TextureView,

	world: World,
	current_sector: Option<Entity>,

	camera: OrbitCamera,
}

impl Client {
	// TODO: A lot of this renderer initialization should be moved to it's own function so we can re-init the pipeline.
	pub fn run(runtime: Runtime) -> Result<()> {
		let connection = runtime.block_on(ClientConnection::connect("[::1]:23500"))?;

		let instance = Instance::new(InstanceDescriptor {
			// Vulkan covers everything we care about.
			// GL is for that one guy with a 2014 GPU, will be dropped if it becomes too inconvenient to support.
			backends: Backends::VULKAN | Backends::GL,
			// Also as low as possible for that one guy with a 2014 GPU, same deal
			gles_minor_version: Gles3MinorVersion::Version0,
			// Don't care, we won't use it anyway :pineapplesquish:
			dx12_shader_compiler: Default::default(),
			// Will be debug if debug, good enough:tm:
			flags: Default::default(),
		});

		let event_loop = EventLoop::new()?;

		let window = WindowBuilder::new()
			.with_inner_size(PhysicalSize::new(960, 540))
			.with_title("Solarscape")
			.build(&event_loop)?;

		let surface = unsafe { instance.create_surface(&window) }?;

		let adapter = runtime
			.block_on(instance.request_adapter(&RequestAdapterOptions {
				compatible_surface: Some(&surface),
				force_fallback_adapter: false,
				power_preference: HighPerformance,
			}))
			.expect("requested adapter");

		// Set everything to the minimum we need.
		// This allows older hardware to run the game, although no promise it will be playable.
		let (device, queue) = runtime.block_on(adapter.request_device(
			&DeviceDescriptor {
				label: None,
				features: Features::empty(),
				limits: adapter.limits(),
			},
			None,
		))?;

		let surface_capabilities = surface.get_capabilities(&adapter);

		let surface_format = surface_capabilities
			.formats
			.iter()
			.copied()
			.find(|format| format.is_srgb())
			.expect("format that supports srgb");

		let size = window.inner_size();

		let config = SurfaceConfiguration {
			usage: TextureUsages::RENDER_ATTACHMENT,
			format: surface_format,
			width: size.width,
			height: size.height,
			present_mode: AutoVsync,
			alpha_mode: Auto,
			view_formats: vec![],
		};

		surface.configure(&device, &config);

		// TODO: Maybe load from a file at runtime to allow modification? If anyone actually wants this, feel free to PR.
		let shader = device.create_shader_module(include_wgsl!("shader.wgsl"));

		let camera_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
			label: Some("camera_group_layout"),
			entries: &[BindGroupLayoutEntry {
				binding: 0,
				visibility: ShaderStages::VERTEX,
				ty: Buffer {
					ty: Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			}],
		});

		// Trans rights!
		// Any PR attempting to remove the variable name will be rejected, and the submitter potentially blocked.
		let trans_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
			label: None,
			layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor {
				label: None,
				bind_group_layouts: &[&camera_group_layout],
				push_constant_ranges: &[],
			})),
			vertex: VertexState {
				module: &shader,
				entry_point: "vertex",
				buffers: &[
					VertexBufferLayout {
						array_stride: size_of::<f32>() as BufferAddress * 4,
						step_mode: VertexStepMode::Instance,
						attributes: &[
							VertexAttribute {
								format: Float32x3,
								offset: 0,
								shader_location: 0,
							},
							VertexAttribute {
								format: Float32,
								offset: 12,
								shader_location: 1,
							},
						],
					},
					VertexBufferLayout {
						array_stride: size_of::<f32>() as BufferAddress * 3,
						step_mode: VertexStepMode::Vertex,
						attributes: &[VertexAttribute {
							format: Float32x3,
							offset: 0,
							shader_location: 2,
						}],
					},
				],
			},
			primitive: PrimitiveState {
				topology: TriangleList,
				strip_index_format: None,
				front_face: Ccw,
				cull_mode: Some(Back),
				unclipped_depth: false,
				polygon_mode: Fill,
				conservative: false,
			},
			depth_stencil: Some(DepthStencilState {
				format: Depth32Float,
				depth_write_enabled: true,
				depth_compare: Greater,
				stencil: StencilState::default(),
				bias: DepthBiasState::default(),
			}),
			// TODO: Should be a config option
			multisample: MultisampleState {
				count: 1,
				mask: !0,
				alpha_to_coverage_enabled: false,
			},
			fragment: Some(FragmentState {
				module: &shader,
				entry_point: "fragment",
				targets: &[Some(ColorTargetState {
					format: config.format,
					blend: Some(BlendState::REPLACE),
					write_mask: ColorWrites::COLOR,
				})],
			}),
			multiview: None,
		});

		let (depth_texture, depth_view) = Self::create_depth_buffer(&device, size.width, size.height);

		let client = Self {
			camera: OrbitCamera::new(&device, &camera_group_layout),

			window,
			surface,
			device,
			queue,
			size,
			config,
			trans_pipeline,
			depth_texture,
			depth_view,

			world: World::new(),
			current_sector: None,
		};

		Ok(client.event_loop(event_loop, connection)?)
	}

	fn create_depth_buffer(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
		let depth_texture = device.create_texture(&TextureDescriptor {
			label: Some("depth_texture"),
			size: Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: TextureDimension::D2,
			format: Depth32Float,
			usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		});

		let depth_view = depth_texture.create_view(&TextureViewDescriptor::default());

		(depth_texture, depth_view)
	}

	fn resize(&mut self, new_size: PhysicalSize<u32>) {
		self.config.width = new_size.width;
		self.config.height = new_size.height;

		let (depth_texture, depth_view) = Self::create_depth_buffer(&self.device, new_size.width, new_size.height);

		self.depth_texture = depth_texture;
		self.depth_view = depth_view;

		self.size = new_size;

		self.surface.configure(&self.device, &self.config);
	}

	// TODO: This looks very messy, I hate it, clean it up if possible.
	fn event_loop(mut self, event_loop: EventLoop<()>, mut connection: ClientConnection) -> Result<(), EventLoopError> {
		event_loop.run(move |event, control_flow| match event {
			WindowEvent { event, window_id } if window_id == self.window.id() => match event {
				Resized(new_size) => self.resize(new_size),
				CloseRequested | Destroyed => control_flow.exit(),
				MouseWheel { delta, .. } => self.camera.handle_mouse_wheel(delta),
				MouseInput { state, button, .. } => self.camera.handle_mouse_input(state, button),
				RedrawRequested if window_id == self.window.id() => self.render(),
				_ => {}
			},
			DeviceEvent { event, .. } => self.camera.handle_device_event(event),
			AboutToWait => {
				if self.camera.position_changed {
					self.camera.position_changed = false;

					connection.send(encode(Message::Event(Event::PositionUpdated(self.camera.position))));
				}

				loop {
					match connection.receive().try_recv() {
						Ok(packet) => match self.process_message(packet) {
							Ok(_) => {}
							Err(disconnect_reason) => {
								// Cursed hack to steal the connection so we can disconnect it
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

				self.window.request_redraw();
			}
			_ => {}
		})
	}

	fn render(&mut self) {
		let output = match self.surface.get_current_texture() {
			Ok(value) => value,
			Err(_) => {
				self.resize(self.size);
				self.surface.get_current_texture().expect("next surface texture")
			}
		};

		let view = output.texture.create_view(&TextureViewDescriptor {
			label: None,
			format: Some(self.config.format),
			dimension: Some(TextureViewDimension::D2),
			aspect: TextureAspect::All,
			base_mip_level: 0,
			mip_level_count: None,
			base_array_layer: 0,
			array_layer_count: None,
		});

		let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor::default());

		let mut chunks = self.world.query::<&Chunk>();

		{
			let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
				label: Some("render_pass"),
				color_attachments: &[Some(RenderPassColorAttachment {
					ops: Operations {
						load: Clear(Color::BLACK),
						store: Store,
					},
					resolve_target: None,
					view: &view,
				})],
				depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
					view: &self.depth_view,
					depth_ops: Some(Operations {
						load: Clear(0.0),
						store: Store,
					}),
					stencil_ops: None,
				}),
				timestamp_writes: None,
				occlusion_query_set: None,
			});

			render_pass.set_pipeline(&self.trans_pipeline);

			self.camera
				.update_matrix(&self.queue, self.config.width, self.config.height);
			self.camera.use_camera(&mut render_pass);

			chunks.iter().for_each(|(_, chunk)| chunk.render(&mut render_pass));
		}

		self.queue.submit(iter::once(encoder.finish()));
		output.present();
	}

	fn process_message(&mut self, message: Message) -> Result<(), DisconnectReason> {
		match message {
			Message::SyncEntity { entity, sync } => match sync {
				SyncEntity::Sector { name, display_name } => {
					self.world.spawn_at(entity, (Sector { name, display_name },))
				}
				SyncEntity::Object => self.world.spawn_at(entity, (Object,)),
				SyncEntity::Chunk {
					grid_position,
					chunk_type,
					data,
				} => self
					.world
					.spawn_at(entity, (Chunk::new(&self.device, grid_position, chunk_type, data),)),
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
				}
				_ => return Err(DisconnectReason::ProtocolViolation),
			},
		}

		Ok(())
	}
}
