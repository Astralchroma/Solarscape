use crate::{chunk::Chunk, object::Object, world::World};
use anyhow::Result;
use bytemuck::cast_slice;
use log::info;
use nalgebra::{Matrix4, Point3, Vector3};
use solarscape_shared::io::{PacketRead, PacketWrite};
use solarscape_shared::protocol::Clientbound::{ActiveSector, AddObject, Disconnected, SyncChunk, SyncSector};
use solarscape_shared::protocol::{Serverbound, PROTOCOL_VERSION};
use std::{convert::Infallible, iter, mem::size_of, sync::Arc};
use tokio::{net::TcpStream, runtime::Runtime};
use wgpu::{
	include_wgsl, util::BufferInitDescriptor, util::DeviceExt, Backends, BindGroup, BindGroupDescriptor,
	BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType::Buffer, BlendState, BufferAddress,
	BufferBindingType::Uniform, BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoderDescriptor,
	CompositeAlphaMode::Auto, Device, DeviceDescriptor, Face::Back, Features, FragmentState, FrontFace::Ccw, Instance,
	InstanceDescriptor, Limits, LoadOp::Clear, MultisampleState, Operations, PipelineLayoutDescriptor,
	PolygonMode::Fill, PowerPreference::HighPerformance, PresentMode::AutoVsync, PrimitiveState,
	PrimitiveTopology::TriangleList, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
	RenderPipelineDescriptor, RequestAdapterOptions, ShaderStages, Surface, SurfaceConfiguration, TextureAspect,
	TextureUsages, TextureViewDescriptor, TextureViewDimension, VertexAttribute, VertexBufferLayout,
	VertexFormat::Float32x3, VertexState, VertexStepMode,
};
use winit::{
	dpi::PhysicalSize,
	event::{
		Event::{LoopDestroyed, MainEventsCleared, RedrawRequested, WindowEvent},
		WindowEvent::{CloseRequested, Destroyed, Resized, ScaleFactorChanged},
	},
	event_loop::{ControlFlow, EventLoop},
	window::{Window, WindowBuilder},
};

pub struct Client {
	window: Window,
	surface: Surface,
	device: Device,
	queue: Queue,
	camera_bind_group: BindGroup,
	trans_pipeline: RenderPipeline,
	world: Arc<World>,
}

impl Client {
	// TODO: A lot of this renderer initialization should be moved to it's own function so we can re-init the pipeline.
	pub fn run(runtime: Runtime) -> Result<Infallible> {
		let event_loop = EventLoop::new();

		let window = WindowBuilder::new()
			.with_inner_size(PhysicalSize::new(960, 540))
			.with_title("Solarscape")
			.build(&event_loop)?;

		let instance = Instance::new(InstanceDescriptor {
			// Vulkan covers everything we care about.
			// GL is for that one guy with a 2014 GPU, will be dropped if it becomes too inconvenient to support.
			backends: Backends::VULKAN | Backends::GL,
			// Don't care, we won't use it anyway :pineapplesquish:
			dx12_shader_compiler: Default::default(),
		});

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
				label: Some("device"),
				features: Features::empty(),
				limits: Limits {
					max_bind_groups: 1,
					max_bindings_per_bind_group: 0,
					max_buffer_size: 37728,
					max_compute_invocations_per_workgroup: 0,
					max_compute_workgroup_size_x: 0,
					max_compute_workgroup_size_y: 0,
					max_compute_workgroup_size_z: 0,
					max_compute_workgroup_storage_size: 0,
					max_compute_workgroups_per_dimension: 0,
					max_dynamic_storage_buffers_per_pipeline_layout: 0,
					max_dynamic_uniform_buffers_per_pipeline_layout: 0,
					max_inter_stage_shader_components: 0,
					max_push_constant_size: 0,
					max_sampled_textures_per_shader_stage: 0,
					max_samplers_per_shader_stage: 0,
					max_storage_buffer_binding_size: 0,
					max_storage_buffers_per_shader_stage: 0,
					max_storage_textures_per_shader_stage: 0,
					max_texture_array_layers: 0,
					max_texture_dimension_1d: 0,
					max_texture_dimension_2d: 0,
					max_texture_dimension_3d: 0,
					max_uniform_buffer_binding_size: 64,
					max_uniform_buffers_per_shader_stage: 1,
					max_vertex_attributes: 1,
					max_vertex_buffer_array_stride: 12,
					max_vertex_buffers: 1,
					min_storage_buffer_offset_alignment: 32,
					min_uniform_buffer_offset_alignment: 32,
				},
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

		let position = Point3::new(32.0, 32.0, 32.0);
		let target = Point3::new(0.0, 0.0, 0.0);

		let view = Matrix4::look_at_rh(&position, &target, &Vector3::new(0.0, 1.0, 0.0));
		let projection = Matrix4::new_perspective(16.0 / 9.0, f32::to_radians(45.0), 0.0, f32::MAX);

		let matrix = projection * view;

		let camera_buffer = device.create_buffer_init(&BufferInitDescriptor {
			label: Some("camera_buffer"),
			contents: cast_slice(&matrix.as_slice()),
			usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
		});

		let camera_bind_group = device.create_bind_group(&BindGroupDescriptor {
			label: Some("camera_bind_group"),
			layout: &camera_group_layout,
			entries: &[BindGroupEntry {
				binding: 0,
				resource: camera_buffer.as_entire_binding(),
			}],
		});

		// TODO: Maybe load from a file at runtime to allow modification? If anyone actually wants this, feel free to PR.
		let shader = device.create_shader_module(include_wgsl!("shader.wgsl"));

		// Trans rights!
		// Any PR attempting to remove the variable name will be rejected, and the submitter potentially blocked.
		let trans_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
			label: Some("trans_pipeline"),
			layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor {
				label: Some("trans_pipeline_layout"),
				bind_group_layouts: &[&camera_group_layout],
				push_constant_ranges: &[],
			})),
			vertex: VertexState {
				module: &shader,
				entry_point: "vertex",
				buffers: &[VertexBufferLayout {
					array_stride: size_of::<f32>() as BufferAddress * 3,
					step_mode: VertexStepMode::Vertex,
					attributes: &[VertexAttribute {
						format: Float32x3,
						offset: 0,
						shader_location: 0,
					}],
				}],
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
			depth_stencil: None,
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

		let world = World::new();

		let client = Arc::new(Self {
			window,
			surface,
			device,
			queue,
			camera_bind_group,
			trans_pipeline,
			world,
		});

		// At this point this thread becomes the event loop thread, we spin off a tokio task for networking.
		let client_clone = client.clone();
		runtime.spawn(async move { client_clone.handle_connection().await });

		client.event_loop(config, event_loop);
	}

	// We pass ownership of the config because the event_loop needs mutable access, and nothing else needs it.
	fn event_loop(self: Arc<Self>, mut config: SurfaceConfiguration, event_loop: EventLoop<()>) -> ! {
		event_loop.run(move |event, _, control_flow| {
			let mut resize = |new_size: PhysicalSize<u32>| {
				config.width = new_size.width;
				config.height = new_size.height;
				self.surface.configure(&self.device, &config);
			};

			match event {
				WindowEvent { event, window_id } if window_id == self.window.id() => match event {
					Resized(new_size) => resize(new_size),
					CloseRequested | Destroyed => *control_flow = ControlFlow::Exit,
					ScaleFactorChanged { new_inner_size, .. } => resize(*new_inner_size),
					_ => {}
				},
				MainEventsCleared => self.window.request_redraw(),
				LoopDestroyed => *control_flow = ControlFlow::Exit,
				RedrawRequested(window_id) if window_id == self.window.id() => {
					let output = match self.surface.get_current_texture() {
						Ok(value) => value,
						Err(_) => {
							self.surface.configure(&self.device, &config);
							self.surface.get_current_texture().expect("next surface texture")
						}
					};

					let view = output.texture.create_view(&TextureViewDescriptor {
						label: Some("view"),
						format: Some(config.format),
						dimension: Some(TextureViewDimension::D2),
						aspect: TextureAspect::All,
						base_mip_level: 0,
						mip_level_count: None,
						base_array_layer: 0,
						array_layer_count: None,
					});

					let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor::default());

					let _ = {
						let active_sector_guard = self.world.active_sector.blocking_write();
						let active_sector = active_sector_guard.as_ref().unwrap();

						let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
							label: Some("render_pass"),
							color_attachments: &[Some(RenderPassColorAttachment {
								ops: Operations {
									load: Clear(Color::BLACK),
									store: true,
								},
								resolve_target: None,
								view: &view,
							})],
							depth_stencil_attachment: None,
						});

						render_pass.set_pipeline(&self.trans_pipeline);

						render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

						for object in active_sector.objects.values() {
							for chunk in object.chunks.values() {
								chunk.render(&mut render_pass);
							}
						}
					};

					self.queue.submit(iter::once(encoder.finish()));
					output.present();
				}
				_ => {}
			}
		});
	}

	pub async fn handle_connection(&self) -> Result<Infallible> {
		let mut stream = TcpStream::connect("[::1]:23500").await?;
		info!("Connecting to [::1]:23500");

		stream
			.write_packet(&Serverbound::Hello {
				major_version: *PROTOCOL_VERSION,
			})
			.await?;

		loop {
			match stream.read_packet().await? {
				Disconnected { reason } => panic!("Disconnected: {reason:?}"),
				SyncSector { name, display_name } => self.world.add_sector(name, display_name).await,
				ActiveSector { name } => self.world.set_active_sector(name).await,
				AddObject { object_id } => {
					info!("Added object {object_id}");

					self.world
						.active_sector
						.write()
						.await
						.as_mut()
						.expect("active sector should be set")
						.objects
						.insert(object_id, Object::new(object_id));
				}
				SyncChunk {
					object_id,
					grid_position,
					data,
				} => {
					info!("Added chunk {grid_position:?} to {object_id}");

					let mut chunk = Chunk::new(&self.device, grid_position, data);
					chunk.build_mesh(&self.queue);
					self.world
						.active_sector
						.write()
						.await
						.as_mut()
						.expect("active sector should be set")
						.objects
						.get_mut(&object_id)
						.expect("object_id of chunk should exist")
						.chunks
						.insert(grid_position, chunk);
				}
			}
		}
	}
}
