use crate::{chunk::ChunkMesh, client::Client, components::LocationBuffer, Arguments, Backend};
use std::{iter, ops::Deref, ops::DerefMut};
use thiserror::Error;
use tokio::runtime::Runtime;
use wgpu::{
	include_wgsl, Backends, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType::Buffer,
	BlendState, BufferBindingType::Uniform, Color, ColorTargetState, ColorWrites, CommandEncoderDescriptor,
	CompareFunction::Greater, CompositeAlphaMode::Opaque, CreateSurfaceError, DepthBiasState, DepthStencilState,
	Device, DeviceDescriptor, Extent3d, Face::Back, Features, FragmentState, Gles3MinorVersion::Version0, Instance,
	InstanceDescriptor, InstanceFlags, LoadOp::Clear, MultisampleState, Operations, PipelineLayoutDescriptor,
	PowerPreference::HighPerformance, PresentMode::AutoVsync, PrimitiveState, Queue, RenderPassColorAttachment,
	RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
	RequestAdapterOptions, RequestDeviceError, ShaderStages, StencilState, StoreOp::Store, Surface,
	SurfaceConfiguration, SurfaceError, Texture, TextureDescriptor, TextureDimension, TextureFormat::Depth32Float,
	TextureUsages, TextureView, TextureViewDescriptor, VertexAttribute, VertexBufferLayout, VertexFormat::Float32,
	VertexFormat::Float32x3, VertexState, VertexStepMode,
};
use winit::{dpi::PhysicalSize, error::OsError, event_loop::EventLoop, window::Window, window::WindowBuilder};
use RendererInitializationError::{RequestAdapterError, SurfaceFormatError};

pub struct Renderer {
	surface: Surface,
	pub device: Device,
	pub queue: Queue,
	config: SurfaceConfiguration,
	pub camera_bind_group_layout: BindGroupLayout,
	trans_pipeline: RenderPipeline,

	pub size: LateInit<PhysicalSize<u32>>,
	depth_texture: LateInit<Texture>,
	depth_view: LateInit<TextureView>,

	// Surface expects that the Window remains valid until after Surface is dropped, Rust will drop struct fields in
	// order of declaration, so we place the Window at the end, everything else is just in order of initialization.
	pub window: Window,
}

impl Renderer {
	// The EventLoop isn't strictly to do with rendering, so it's initialised outside and brought in here.
	// Additionally, arguments and runtime are supposed to just be part of the Client, however initializing the Client
	// requires the Renderer to be initialized first, so we just pass them in directly here.
	pub fn init(
		event_loop: &EventLoop<()>,
		arguments: &Arguments,
		runtime: &Runtime,
	) -> Result<Self, RendererInitializationError> {
		// Many games don't make the window visible until they are actually ready. We prefer to make the window visible
		// as soon as possible as it provides feedback that things are actually happening.
		let window = WindowBuilder::new()
			.with_active(true)
			.with_inner_size(PhysicalSize::new(1280, 720))
			.with_maximized(true)
			.with_title("Solarscape")
			.build(event_loop)?;

		let instance = Instance::new(InstanceDescriptor {
			// OpenGL is mostly there to support that one guy with a 2014 GPU. Solarscape aims to support reasonable
			// hardware released in the past ~10 years or so, so this means supporting OpenGL, as we use wgpu, this
			// isn't an issue really. Although if supporting OpenGL becomes difficult, we may drop it if all reasonable
			// GPUs in that ten year window support Vulkan.
			#[rustfmt::skip]
			backends: match arguments.backend {
				Backend { gl: true, vulkan: false } => Backends::GL,
				Backend { gl: false, vulkan: true } => Backends::VULKAN,
				Backend { gl: false, vulkan: false} => Backends::VULKAN | Backends::GL,
				_ => panic!("opengl and vulkan should not be forced at the same time!"),
			},
			gles_minor_version: Version0,
			// We don't care what this is set to, we don't use DirectX because we already support Vulkan and OpenGL and
			// everything that we care about which would have DirectX would also have both of those.
			dx12_shader_compiler: Default::default(),
			flags: match arguments.debug {
				true => InstanceFlags::DEBUG | InstanceFlags::VALIDATION,
				false => InstanceFlags::DISCARD_HAL_LABELS,
			},
		});

		let surface = unsafe { instance.create_surface(&window) }?;

		// TODO: Allow changing adapter after initialization.
		// TODO: Iterate over adapters and try different ones until something works.
		let adapter = runtime
			.block_on(instance.request_adapter(&RequestAdapterOptions {
				compatible_surface: Some(&surface),
				force_fallback_adapter: false,
				power_preference: HighPerformance,
			}))
			.ok_or(RequestAdapterError)?;

		let (device, queue) = runtime.block_on(adapter.request_device(
			&DeviceDescriptor {
				label: Some("Renderer.device"),
				features: Features::empty(),
				// Just request whatever limits the adapter supports for now.
				// TODO: Request whatever Solarscape actually needs.
				limits: adapter.limits(),
			},
			arguments.tracing.as_deref(),
		))?;

		let surface_capabilities = surface.get_capabilities(&adapter);

		let surface_format = surface_capabilities
			.formats
			.iter()
			.copied()
			.find(|format| format.is_srgb())
			.ok_or(SurfaceFormatError)?;

		let config = SurfaceConfiguration {
			usage: TextureUsages::RENDER_ATTACHMENT,
			format: surface_format,
			// We set width and height to 0 for now, this is fine as we will immediately call resize and update this.
			width: 0,
			height: 0,
			// TODO: Vsync Configuration
			present_mode: AutoVsync,
			alpha_mode: Opaque,
			view_formats: vec![],
		};

		let camera_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
			label: Some("Renderer.camera_bind_group_layout"),
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

		// TODO: Load from a file at runtime to allow modification.
		let shader = device.create_shader_module(include_wgsl!("shader.wgsl"));

		// We'll have to make multiple Pipelines later, however for now the funny trans_pipeline name must stay.
		let trans_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
			label: Some("Renderer.trans_pipeline"),
			layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor {
				label: Some("Renderer::init()#_trans_pipeline_layout_descriptor"),
				bind_group_layouts: &[&camera_bind_group_layout],
				push_constant_ranges: &[],
			})),
			vertex: VertexState {
				module: &shader,
				entry_point: "vertex",
				buffers: &[
					VertexBufferLayout {
						array_stride: (4 * 6) + 4,
						step_mode: VertexStepMode::Instance,
						attributes: &[
							VertexAttribute {
								format: Float32x3,
								offset: 0,
								shader_location: 0,
							},
							VertexAttribute {
								format: Float32x3,
								offset: 4 * 3,
								shader_location: 1,
							},
							VertexAttribute {
								format: Float32,
								offset: 4 * 6,
								shader_location: 2,
							},
						],
					},
					VertexBufferLayout {
						array_stride: 4 * 3,
						step_mode: VertexStepMode::Vertex,
						attributes: &[VertexAttribute {
							format: Float32x3,
							offset: 0,
							shader_location: 3,
						}],
					},
				],
			},
			primitive: PrimitiveState {
				cull_mode: Some(Back),
				..Default::default()
			},
			depth_stencil: Some(DepthStencilState {
				format: Depth32Float,
				depth_write_enabled: true,
				depth_compare: Greater,
				stencil: StencilState::default(),
				bias: DepthBiasState::default(),
			}),
			// TODO: Should be a config option
			multisample: MultisampleState::default(),
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

		let mut renderer = Self {
			surface,
			device,
			queue,
			config,
			camera_bind_group_layout,
			trans_pipeline,

			// These get created and set in the resize() call.
			size: LateInit::new(),
			depth_texture: LateInit::new(),
			depth_view: LateInit::new(),

			window,
		};

		renderer.resize(renderer.window.inner_size());

		// Sometimes the client can crash if it is moved between windows before it renders a frame.
		// This is known to happen on Vulkan on Hyprland (wayland).
		// A dumb hacky solution is just to render a blank frame.
		let output = renderer.surface.get_current_texture()?;

		let view = output.texture.create_view(&TextureViewDescriptor {
			label: Some("Renderer::init()#view"),
			..Default::default()
		});

		let mut encoder = renderer.device.create_command_encoder(&CommandEncoderDescriptor {
			label: Some("Renderer::init()#encoder"),
		});

		encoder.begin_render_pass(&RenderPassDescriptor {
			label: Some("Renderer::init()#_render_pass"),
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

		renderer.queue.submit(iter::once(encoder.finish()));
		output.present();

		Ok(renderer)
	}

	pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
		self.config.width = new_size.width;
		self.config.height = new_size.height;
		self.size.set(new_size);

		let depth_texture = self.device.create_texture(&TextureDescriptor {
			label: Some("Renderer.depth_texture"),
			size: Extent3d {
				width: self.size.width,
				height: self.size.height,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: TextureDimension::D2,
			format: Depth32Float,
			usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		});

		let depth_view = depth_texture.create_view(&TextureViewDescriptor {
			label: Some("Renderer.depth_view"),
			..Default::default()
		});

		self.depth_texture.set(depth_texture);
		self.depth_view.set(depth_view);

		self.surface.configure(&self.device, &self.config);
	}

	pub fn render(client: &mut Client) -> Result<(), RendererError> {
		// TODO: This error can probably be handled better by reinitializing the surface.
		let output = match client.renderer.surface.get_current_texture() {
			Ok(value) => value,
			Err(_) => {
				client.renderer.resize(*client.renderer.size);
				client.renderer.surface.get_current_texture()?
			}
		};

		let view = output.texture.create_view(&TextureViewDescriptor {
			label: Some("Renderer::render()#view"),
			..Default::default()
		});

		let mut encoder = client
			.renderer
			.device
			.create_command_encoder(&CommandEncoderDescriptor {
				label: Some("Renderer::render()#encoder"),
			});

		// Currently anything used by render_pass must outlive it, so chunk_query must be here.
		let chunk_query = client.world.query_mut::<(&ChunkMesh, &LocationBuffer)>();

		{
			let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
				label: Some("Renderer::render()#render_pass"),
				color_attachments: &[Some(RenderPassColorAttachment {
					ops: Operations {
						load: Clear(Color::BLACK),
						store: Store,
					},
					resolve_target: None,
					view: &view,
				})],
				depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
					view: &client.renderer.depth_view,
					depth_ops: Some(Operations {
						load: Clear(0.0),
						store: Store,
					}),
					stencil_ops: None,
				}),
				..Default::default()
			});

			render_pass.set_bind_group(0, &client.camera.bind_group, &[]);

			render_pass.set_pipeline(&client.renderer.trans_pipeline);

			for (_, (chunk, location_buffer)) in chunk_query {
				render_pass.set_vertex_buffer(0, location_buffer.slice(..));
				chunk.render(&mut render_pass);
			}
		}

		client.renderer.queue.submit(iter::once(encoder.finish()));
		output.present();

		Ok(())
	}
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum RendererInitializationError {
	WindowSpawn(#[from] OsError),

	CreateSurface(#[from] CreateSurfaceError),

	#[error("Failed to request an adapter!")]
	RequestAdapterError,

	RequestDeviceError(#[from] RequestDeviceError),

	#[error("Failed to find a valid surface format!")]
	SurfaceFormatError,

	SurfaceError(#[from] SurfaceError),
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum RendererError {
	Surface(#[from] SurfaceError),
}

#[repr(transparent)]
pub struct LateInit<T>(Option<T>);

impl<T> LateInit<T> {
	#[must_use]
	pub fn new() -> Self {
		Self(None)
	}

	pub fn set(&mut self, inner: T) {
		self.0 = Some(inner);
	}
}

impl<T> Deref for LateInit<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.as_ref().expect("LateInit value accessed before Initialization")
	}
}

impl<T> DerefMut for LateInit<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.0.as_mut().expect("LateInit value accessed before Initialization")
	}
}
