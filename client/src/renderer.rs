use crate::{client::AnyState, client::State, login::Login, world::Sector, ClArgs};
use bytemuck::cast_slice;
use egui::{Align2, Color32, Context, Pos2, ViewportId};
use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use egui_winit::State as EguiState;
use image::GenericImageView;
use log::error;
use nalgebra::Perspective3;
use std::{collections::VecDeque, iter::once, time::Duration, time::Instant};
use thiserror::Error;
use tokio::runtime::Handle;
use wgpu::{
	include_wgsl, rwh::HandleError, vertex_attr_array, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry,
	BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Color, ColorTargetState,
	ColorWrites, CommandEncoderDescriptor, CompareFunction::LessEqual, CompositeAlphaMode::Opaque, CreateSurfaceError,
	DepthStencilState, Device, DeviceDescriptor, Dx12Compiler, Extent3d, Face::Back, Features, FragmentState,
	FrontFace::Ccw, Gles3MinorVersion::Version0, ImageCopyTexture, ImageDataLayout, Instance, InstanceDescriptor,
	InstanceFlags, Limits, LoadOp::Clear, MemoryHints::Performance, MultisampleState, Operations, Origin3d,
	PipelineCompilationOptions, PipelineLayoutDescriptor, PolygonMode::Fill, PowerPreference::HighPerformance,
	PresentMode::AutoNoVsync, PrimitiveState, PrimitiveTopology::TriangleList, PushConstantRange, Queue, RenderPass,
	RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipeline,
	RenderPipelineDescriptor, RequestAdapterOptions, RequestDeviceError, SamplerBindingType::NonFiltering,
	SamplerDescriptor, ShaderStages, StoreOp::Store, Surface, SurfaceConfiguration, SurfaceTargetUnsafe, Texture,
	TextureAspect::All, TextureDescriptor, TextureDimension, TextureDimension::D2, TextureFormat,
	TextureFormat::Depth32Float, TextureFormat::Rgba8UnormSrgb, TextureSampleType::Float, TextureUsages, TextureView,
	TextureViewDescriptor, TextureViewDimension, VertexBufferLayout, VertexState, VertexStepMode,
};
use winit::window::{CursorGrabMode::Confined, CursorGrabMode::Locked, Window};
use winit::{dpi::LogicalPosition, dpi::PhysicalSize, error::OsError, event::WindowEvent, event_loop::ActiveEventLoop};

pub struct Renderer {
	// Window & Surface
	// SAFETY: Window must be first so that it outlives Surface!
	pub window: Window,
	surface: Surface<'static>,
	config: SurfaceConfiguration,

	// Device & Queue
	// This may be worth splitting out into it's own struct stored in an Arc<T> later
	device: Device,
	queue: Queue,

	// Frame time information, we will probably improve the infrustructure
	// around this later to deliver a more detailed breakdown
	frame_times: VecDeque<Duration>,
	frame_time_total: Duration,
	frame_time_average: Duration,
	frames_per_second: usize,

	// Egui
	egui_state: EguiState,
	egui_renderer: EguiRenderer,

	// Depth Buffer
	depth_buffer_descriptor: TextureDescriptor<'static>,
	depth_buffer: Texture,
	depth_buffer_view: TextureView,

	// Camera
	// Might be worth moving later
	perspective: Perspective3<f32>,

	// World Rendering
	// Might be worth moving later
	chunk_pipeline: RenderPipeline,
	terrain_textures_bind_group: BindGroup,
}

impl Renderer {
	pub fn new(event_loop: &ActiveEventLoop) -> Result<Self, RenderInitError> {
		let instance = Instance::new(InstanceDescriptor {
			backends: Backends::VULKAN | Backends::GL,
			flags: InstanceFlags::empty(),
			dx12_shader_compiler: Dx12Compiler::default(), // DirectX is not supported, don't care
			gles_minor_version: Version0,
		});

		let window = event_loop.create_window(
			Window::default_attributes()
				.with_maximized(true)
				.with_inner_size(PhysicalSize {
					width: 854,
					height: 480,
				})
				.with_title("Solarscape"),
		)?;

		let surface = unsafe { instance.create_surface_unsafe(SurfaceTargetUnsafe::from_window(&window)?) }?;

		let adapter = Handle::current()
			.block_on(instance.request_adapter(&RequestAdapterOptions {
				power_preference: HighPerformance,
				force_fallback_adapter: false,
				compatible_surface: Some(&surface),
			}))
			.ok_or(RenderInitError::NoAdapter)?;

		let (device, queue) = Handle::current().block_on(adapter.request_device(
			&DeviceDescriptor {
				label: Some("renderer#device"),
				required_features: Features::PUSH_CONSTANTS,
				required_limits: Limits {
					// General Limits
					max_buffer_size: 54720,

					// Solarscape Required Limits
					max_bindings_per_bind_group: 2,
					max_color_attachment_bytes_per_sample: 8,
					max_color_attachments: 1,
					max_inter_stage_shader_components: 11,
					max_push_constant_size: 64,
					max_sampled_textures_per_shader_stage: 1,
					max_samplers_per_shader_stage: 1,
					max_texture_array_layers: 1,
					max_vertex_attributes: 7,
					max_vertex_buffer_array_stride: 20,
					max_vertex_buffers: 3,

					// This also determines the limit of our window resolution, so we'll request what the GPU supports
					max_texture_dimension_2d: adapter.limits().max_texture_dimension_2d,

					// These are minimums, not maximums, so we'll just request what the GPU supports
					min_storage_buffer_offset_alignment: adapter.limits().min_storage_buffer_offset_alignment,
					min_subgroup_size: adapter.limits().min_subgroup_size,
					min_uniform_buffer_offset_alignment: adapter.limits().min_uniform_buffer_offset_alignment,

					// Limits that seem to be imposed by Egui
					max_bind_groups: 2,
					max_uniform_buffer_binding_size: 16,
					max_uniform_buffers_per_shader_stage: 1,

					// Unused / Undetermined
					max_compute_invocations_per_workgroup: 0,
					max_compute_workgroup_size_x: 0,
					max_compute_workgroup_size_y: 0,
					max_compute_workgroup_size_z: 0,
					max_compute_workgroup_storage_size: 0,
					max_compute_workgroups_per_dimension: 0,
					max_dynamic_storage_buffers_per_pipeline_layout: 0,
					max_dynamic_uniform_buffers_per_pipeline_layout: 0,
					max_non_sampler_bindings: 0,
					max_storage_buffer_binding_size: 0,
					max_storage_buffers_per_shader_stage: 0,
					max_storage_textures_per_shader_stage: 0,
					max_subgroup_size: 0,
					max_texture_dimension_1d: 0,
					max_texture_dimension_3d: 0,
				},
				memory_hints: Performance,
			},
			None,
		))?;

		let surface_capabilities = surface.get_capabilities(&adapter);

		let surface_format = surface_capabilities
			.formats
			.iter()
			.copied()
			.find(TextureFormat::is_srgb)
			.ok_or(RenderInitError::NoSurfaceFormat)?;

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
			label: Some("renderer.voxject#texture"),
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
			label: Some("renderer.voxject#texture_bind_group_layout"),
			entries: &[
				BindGroupLayoutEntry {
					binding: 0,
					visibility: ShaderStages::FRAGMENT,
					ty: BindingType::Texture {
						sample_type: Float { filterable: false },
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
			label: Some("renderer.voxject#texture_bind_group"),
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
			label: Some("renderer.voxject#pipeline_layout"),
			bind_group_layouts: &[&terrain_textures_bind_group_layout],
			push_constant_ranges: &[PushConstantRange {
				stages: ShaderStages::VERTEX,
				range: 0..64,
			}],
		});

		let chunk_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
			label: Some("renderer.voxject#pipeline"),
			layout: Some(&chunk_pipeline_layout),
			vertex: VertexState {
				module: &chunk_shader,
				entry_point: "vertex",
				compilation_options: PipelineCompilationOptions::default(),
				buffers: &[
					VertexBufferLayout {
						array_stride: 12,
						step_mode: VertexStepMode::Vertex,
						attributes: &vertex_attr_array![0 => Float32x3],
					},
					VertexBufferLayout {
						array_stride: 20,
						step_mode: VertexStepMode::Vertex,
						attributes: &vertex_attr_array![1 => Float32x3, 2 => Uint8x2, 3 => Uint8x2, 4 => Float32],
					},
					VertexBufferLayout {
						array_stride: 16,
						step_mode: VertexStepMode::Instance,
						attributes: &vertex_attr_array![5 => Float32x3, 6 => Float32],
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

		let depth_buffer_descriptor = TextureDescriptor {
			label: Some("renderer.depth_buffer#buffer"),
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

		let depth_buffer_view_descriptor = TextureViewDescriptor {
			label: Some("renderer.depth_buffer#view"),
			..TextureViewDescriptor::default()
		};

		let depth_buffer = device.create_texture(&depth_buffer_descriptor);
		let depth_buffer_view = depth_buffer.create_view(&depth_buffer_view_descriptor);

		let debug_state = EguiState::new(Context::default(), ViewportId::default(), &window, None, None, None);
		let egui_renderer = EguiRenderer::new(&device, config.format, Some(Depth32Float), 1, false);

		Ok(Self {
			window,
			surface,
			config,

			device,
			queue,

			frame_times: VecDeque::new(),
			frame_time_total: Duration::default(),
			frame_time_average: Duration::default(),
			frames_per_second: 0,

			egui_state: debug_state,
			egui_renderer,

			depth_buffer_descriptor,
			depth_buffer,
			depth_buffer_view,

			perspective: Perspective3::new(width as f32 / height as f32, f32::to_radians(90.0), 0.05, f32::MAX),

			chunk_pipeline,
			terrain_textures_bind_group,
		})
	}

	pub fn resize(&mut self, PhysicalSize { width, height }: PhysicalSize<u32>) {
		self.config.width = width;
		self.config.height = height;
		self.surface.configure(&self.device, &self.config);

		self.depth_buffer_descriptor.size = Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		};
		self.depth_buffer = self.device.create_texture(&self.depth_buffer_descriptor);
		self.depth_buffer_view = self.depth_buffer.create_view(&TextureViewDescriptor::default());

		self.perspective.set_aspect(width as f32 / height as f32);
	}

	pub fn render(&mut self, cl_args: &ClArgs, state: &mut AnyState) {
		let frame_start = Instant::now();

		let output = match self.surface.get_current_texture() {
			Ok(output) => output,
			Err(error) => panic!("{error}"), // We can probably handle this more elegantly later
		};

		// Handle the GUI
		let gui_input = self.egui_state.take_egui_input(&self.window);

		let gui_output = self.egui_state.egui_ctx().run(gui_input, |context| {
			state.draw_ui(cl_args, &context);

			// Debug Text, we'll add a keybind to toggle this later
			context.debug_painter().debug_text(
				Pos2::default(),
				Align2::LEFT_TOP,
				Color32::WHITE,
				format!(
					"Solarscape v{}\n{} FPS / {:.0?} Frame Time",
					env!("CARGO_PKG_VERSION"),
					self.frames_per_second,
					self.frame_time_average
				),
			);
		});

		self.egui_state
			.handle_platform_output(&self.window, gui_output.platform_output);

		let paint_jobs = self.egui_state.egui_ctx().tessellate(gui_output.shapes, 1.0);
		let screen_descriptor = &ScreenDescriptor {
			size_in_pixels: [self.config.width, self.config.height],
			pixels_per_point: 1.0, // Don't know how to calculate this, come back to it later.
		};

		for (id, image_delta) in gui_output.textures_delta.set {
			self.egui_renderer
				.update_texture(&self.device, &self.queue, id, &image_delta);
		}

		let view = output.texture.create_view(&TextureViewDescriptor::default());
		let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor::default());

		self.egui_renderer
			.update_buffers(&self.device, &self.queue, &mut encoder, &paint_jobs, &screen_descriptor);

		{
			let mut render_pass = encoder
				.begin_render_pass(&RenderPassDescriptor {
					color_attachments: &[Some(RenderPassColorAttachment {
						ops: Operations {
							load: Clear(Color::BLACK),
							store: Store,
						},
						resolve_target: None,
						view: &view,
					})],
					depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
						view: &self.depth_buffer_view,
						depth_ops: Some(Operations {
							load: Clear(1.0),
							store: Store,
						}),
						stencil_ops: None,
					}),
					..Default::default()
				})
				.forget_lifetime();

			state.render(self, &mut render_pass);

			self.egui_renderer
				.render(&mut render_pass, &paint_jobs, &screen_descriptor);
		}

		self.queue.submit(once(encoder.finish()));
		output.present();

		let frame_time = Instant::now() - frame_start;

		self.frame_times.push_back(frame_time);
		self.frame_time_total += frame_time;

		while self.frame_time_total > Duration::from_secs(1) {
			let old_frame_time = self.frame_times.pop_front()
				.expect("pop_front should not fail as it is only called if frame_time_total is more than 1 second which requires frame_times to be populated");
			self.frame_time_total -= old_frame_time;
		}

		self.frame_time_average = match self.frame_times.is_empty() {
			true => frame_time,
			false => self.frame_time_total / self.frame_times.len() as u32,
		};

		self.frames_per_second = (self.frame_times.len() as f64 / self.frame_time_total.as_secs_f64()).round() as usize;

		self.window.request_redraw();
	}

	pub fn handle_window_event(&mut self, event: &WindowEvent) {
		let _ = self.egui_state.on_window_event(&self.window, &event);
	}
}

#[allow(unused_variables)]
trait Render {
	fn render(&mut self, renderer: &mut Renderer, render_pass: &mut RenderPass) {}
}

impl Render for AnyState {
	fn render(&mut self, renderer: &mut Renderer, render_pass: &mut RenderPass) {
		match self {
			Self::Login(state) => state as &mut dyn Render,
			Self::Sector(state) => state as &mut dyn Render,
		}
		.render(renderer, render_pass)
	}
}

impl Render for Login {}

impl Render for Sector {
	fn render(&mut self, renderer: &mut Renderer, render_pass: &mut RenderPass) {
		let _ = renderer
			.window
			.set_cursor_grab(Confined)
			.or_else(|_| renderer.window.set_cursor_grab(Locked));
		let _ = renderer.window.set_cursor_visible(false);
		let _ = renderer.window.set_cursor_position(LogicalPosition {
			x: renderer.config.width as f32 / 2.0,
			y: renderer.config.height as f32 / 2.0,
		});

		self.process_messages(&renderer.device);

		render_pass.set_pipeline(&renderer.chunk_pipeline);

		let camera_matrix = renderer.perspective.to_homogeneous() * self.player.location.to_homogeneous();
		render_pass.set_push_constants(ShaderStages::VERTEX, 0, cast_slice(&[camera_matrix]));

		render_pass.set_bind_group(0, &renderer.terrain_textures_bind_group, &[]);

		for chunk in self.chunks.iter() {
			if *chunk.coordinates.level != 0 {
				continue;
			}

			if let Some(mesh) = chunk.mesh.as_ref() {
				render_pass.set_vertex_buffer(0, mesh.vertex_position_buffer.slice(..));
				render_pass.set_vertex_buffer(1, mesh.vertex_data_buffer.slice(..));
				render_pass.set_vertex_buffer(2, mesh.instance_buffer.slice(..));
				render_pass.draw(0..mesh.vertex_count, 0..1);
			}
		}
	}
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum RenderInitError {
	WindowCreationFailed(#[from] OsError),

	SurfaceHandleCreationFailed(#[from] HandleError),

	SurfaceCreationFailed(#[from] CreateSurfaceError),

	#[error("unable to find suitable adapter")]
	NoAdapter,

	RequestDeviceFailed(#[from] RequestDeviceError),

	#[error("unable to find suitable surface format")]
	NoSurfaceFormat,
}
