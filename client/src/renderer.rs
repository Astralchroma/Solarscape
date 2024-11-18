use crate::{
	client::{AnyState, State},
	login::Login,
	world::Sector,
	ClArgs,
};
use bytemuck::cast_slice;
use egui::{Align2, Color32, Context, Pos2, ViewportId};
use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use egui_winit::State as EguiState;
use image::GenericImageView;
use log::{error, info, warn};
use nalgebra::{vector, Isometry3, Perspective3, Translation3, Vector3};
use solarscape_shared::data::world::BlockType;
use std::{
	collections::{HashMap, VecDeque},
	fmt::Write,
	iter::once,
	str::FromStr,
	sync::Arc,
	time::{Duration, Instant},
};
use thiserror::Error;
use tobj::GPU_LOAD_OPTIONS;
use tokio::runtime::Handle;
use wgpu::{
	include_wgsl,
	rwh::HandleError,
	util::{BufferInitDescriptor, DeviceExt, TextureDataOrder::LayerMajor},
	vertex_attr_array, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry,
	BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
	Buffer, BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoderDescriptor,
	CompareFunction::LessEqual,
	CompositeAlphaMode::Opaque,
	CreateSurfaceError, DepthStencilState, Device, DeviceDescriptor, Dx12Compiler, Extent3d,
	Face::Back,
	Features, FragmentState,
	FrontFace::Ccw,
	Gles3MinorVersion::Version0,
	IndexFormat, Instance, InstanceDescriptor, InstanceFlags, Limits,
	LoadOp::Clear,
	MemoryHints::Performance,
	MultisampleState, Operations, PipelineCompilationOptions, PipelineLayoutDescriptor,
	PolygonMode::Fill,
	PowerPreference::HighPerformance,
	PresentMode::AutoNoVsync,
	PrimitiveState,
	PrimitiveTopology::{LineList, TriangleList},
	PushConstantRange, Queue, RenderPass, RenderPassColorAttachment,
	RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipeline,
	RenderPipelineDescriptor, RequestAdapterOptions, RequestDeviceError,
	SamplerBindingType::NonFiltering,
	SamplerDescriptor, ShaderStages,
	StoreOp::Store,
	Surface, SurfaceConfiguration, SurfaceTargetUnsafe, Texture, TextureDescriptor,
	TextureDimension::{self, D2},
	TextureFormat::{self, Depth32Float, Rgba8UnormSrgb},
	TextureSampleType::Float,
	TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension, VertexBufferLayout,
	VertexState, VertexStepMode,
};
use winit::{
	dpi::{LogicalPosition, PhysicalSize},
	error::OsError,
	event::WindowEvent,
	event_loop::ActiveEventLoop,
	window::{CursorGrabMode, Window},
};

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

	// Frame time information, we will probably improve the infrastructure
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

	// Structure Rendering
	// Might also be worth moving later
	structure_block_pipeline: RenderPipeline,
	structure_block_data: HashMap<BlockType, Arc<BlockRenderData>>,
	structure_block_bind_group: BindGroup,

	// Debug Rendering
	debug_line_pipeline: RenderPipeline,
}

struct BlockRenderData {
	positions: Buffer,
	texture_coordinates: Buffer,
	indices: Buffer,

	index_count: u32,
}

impl Renderer {
	pub fn new(event_loop: &ActiveEventLoop) -> Result<Self, RenderInitError> {
		let start_time = Instant::now();

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

		let surface =
			unsafe { instance.create_surface_unsafe(SurfaceTargetUnsafe::from_window(&window)?) }?;

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
					max_buffer_size: u64::pow(2, 17),

					// Solarscape Required Limits
					max_bindings_per_bind_group: 2,
					max_color_attachment_bytes_per_sample: 8,
					max_color_attachments: 1,
					max_inter_stage_shader_components: 11,
					max_push_constant_size: 112,
					max_sampled_textures_per_shader_stage: 1,
					max_samplers_per_shader_stage: 1,
					max_texture_array_layers: 1,
					max_vertex_attributes: 7,
					max_vertex_buffer_array_stride: 68,
					max_vertex_buffers: 3,

					// This also determines the limit of our window resolution, so we'll request what the GPU supports
					max_texture_dimension_2d: adapter.limits().max_texture_dimension_2d,

					// These are minimums, not maximums, so we'll just request what the GPU supports
					min_storage_buffer_offset_alignment:
						adapter.limits().min_storage_buffer_offset_alignment,
					min_subgroup_size: adapter.limits().min_subgroup_size,
					min_uniform_buffer_offset_alignment:
						adapter.limits().min_uniform_buffer_offset_alignment,

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

		let terrain_textures_image =
			image::load_from_memory(include_bytes!("resources/terrain_textures.png"))
				.expect("terrain_textures.png must be valid");
		let terrain_textures_rgba8 = terrain_textures_image.to_rgba8();
		let (terrain_textures_width, terrain_textures_height) = terrain_textures_image.dimensions();
		let terrain_textures_size = Extent3d {
			width: terrain_textures_width,
			height: terrain_textures_height,
			depth_or_array_layers: 1,
		};

		let terrain_textures = device.create_texture_with_data(
			&queue,
			&TextureDescriptor {
				label: Some("renderer.voxject#texture"),
				size: terrain_textures_size,
				mip_level_count: 1,
				sample_count: 1,
				dimension: TextureDimension::D2,
				format: Rgba8UnormSrgb,
				usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
				view_formats: &[],
			},
			LayerMajor,
			&terrain_textures_rgba8,
		);

		let terrain_textures_view = terrain_textures.create_view(&TextureViewDescriptor::default());
		let terrain_textures_sampler = device.create_sampler(&SamplerDescriptor::default());

		let terrain_textures_bind_group_layout =
			device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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
				range: 0..76,
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

		let structure_block_data = {
			let (structure_block_models, _) = tobj::load_obj_buf(
				&mut &include_bytes!("resources/structure_blocks.obj")[..],
				&GPU_LOAD_OPTIONS,
				// We don't care about the material, but this is required so...
				|path| match path.file_name().unwrap().to_str().unwrap() == "structure_blocks.mtl" {
					true => tobj::load_mtl_buf(&mut &include_bytes!("resources/structure_blocks.mtl")[..]),
					false => panic!("attempted to use unknown material resource"),
				},
			)
			.expect("resources/structure_blocks.obj provided at compile time should be a valid .obj file");

			let mut missing_block = None;
			let mut structure_blocks = HashMap::with_capacity(BlockType::ALL.len());

			for mut model in structure_block_models {
				for coord in model.mesh.texcoords.iter_mut().skip(1).step_by(2) {
					*coord = 1.0 - *coord;
				}

				let block_render_data = Arc::new(BlockRenderData {
					positions: device.create_buffer_init(&BufferInitDescriptor {
						label: Some(&format!(
							"Block Renderer > Block '{}' > Positions",
							model.name
						)),
						contents: cast_slice(&model.mesh.positions),
						usage: BufferUsages::VERTEX,
					}),
					texture_coordinates: device.create_buffer_init(&BufferInitDescriptor {
						label: Some(&format!(
							"Block Renderer > Block '{}' > Texture Coordinates",
							model.name
						)),
						contents: cast_slice(&model.mesh.texcoords),
						usage: BufferUsages::VERTEX,
					}),
					indices: device.create_buffer_init(&BufferInitDescriptor {
						label: Some(&format!(
							"Block Renderer > Block '{}' > Indices",
							model.name
						)),
						contents: cast_slice(&model.mesh.indices),
						usage: BufferUsages::INDEX,
					}),
					index_count: model.mesh.indices.len() as u32,
				});

				match BlockType::from_str(&model.name) {
					Ok(block) => {
						if structure_blocks.insert(block, block_render_data).is_some() {
							warn!("Found duplicate model for block {block:?}! This may be a modelling error and could result in broken block models.");
						}
					}
					Err(_) if model.name == "MissingBlock" => {
						if missing_block.replace(block_render_data).is_some() {
							warn!("Found duplicate model for block MissingBlock! This may be a modelling error and could result in broken block models.");
						}
					}
					Err(_) => {}
				}
			}

			let missing_block = match missing_block {
				Some(missing_block) => missing_block,
				None => panic!("No model found for MissingBlock. This block is required as it serves as a placeholder for other missing blocks."),
			};

			for block in BlockType::ALL {
				if !structure_blocks.contains_key(block) {
					warn!("No model found for block {block:?}, a placeholder will be used instead. This will result in broken block models");
					structure_blocks.insert(*block, missing_block.clone());
				}
			}

			structure_blocks
		};

		let structure_block_textures_raw =
			image::load_from_memory(include_bytes!("resources/structure_block_textures.png"))
				.expect("structure_block_textures.png must be valid")
				.to_rgba8();
		let (structure_block_textures_width, structure_block_textures_height) =
			structure_block_textures_raw.dimensions();

		let structure_block_texture = device.create_texture_with_data(
			&queue,
			&TextureDescriptor {
				label: Some("Block Renderer > Texture"),
				size: Extent3d {
					width: structure_block_textures_width,
					height: structure_block_textures_height,
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: D2,
				format: Rgba8UnormSrgb,
				usage: TextureUsages::TEXTURE_BINDING,
				view_formats: &[],
			},
			LayerMajor,
			&structure_block_textures_raw,
		);

		let structure_block_texture_view =
			structure_block_texture.create_view(&TextureViewDescriptor::default());
		let structure_block_texture_sampler = device.create_sampler(&SamplerDescriptor::default());

		let structure_blocks_bind_group_layout =
			device.create_bind_group_layout(&BindGroupLayoutDescriptor {
				label: Some("Block Renderer > Bind Group Layout"),
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

		let structure_block_bind_group = device.create_bind_group(&BindGroupDescriptor {
			label: Some("Block Renderer > Bind Group"),
			layout: &structure_blocks_bind_group_layout,
			entries: &[
				BindGroupEntry {
					binding: 0,
					resource: BindingResource::TextureView(&structure_block_texture_view),
				},
				BindGroupEntry {
					binding: 1,
					resource: BindingResource::Sampler(&structure_block_texture_sampler),
				},
			],
		});

		let structure_block_shader = device.create_shader_module(include_wgsl!("structure.wgsl"));

		let structure_block_pipeline_layout =
			device.create_pipeline_layout(&PipelineLayoutDescriptor {
				label: Some("Block Renderer > Pipeline Layout"),
				bind_group_layouts: &[&structure_blocks_bind_group_layout],
				push_constant_ranges: &[PushConstantRange {
					stages: ShaderStages::VERTEX,
					range: 0..64,
				}],
			});

		let structure_block_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
			label: Some("Block Renderer > Pipeline"),
			layout: Some(&structure_block_pipeline_layout),
			vertex: VertexState {
				module: &structure_block_shader,
				entry_point: "vertex",
				compilation_options: PipelineCompilationOptions::default(),
				buffers: &[
					VertexBufferLayout {
						array_stride: 12,
						step_mode: VertexStepMode::Vertex,
						attributes: &vertex_attr_array![0 => Float32x3],
					},
					VertexBufferLayout {
						array_stride: 8,
						step_mode: VertexStepMode::Vertex,
						attributes: &vertex_attr_array![1 => Float32x2],
					},
					VertexBufferLayout {
						array_stride: 36,
						step_mode: VertexStepMode::Instance,
						attributes: &vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32],
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
				module: &structure_block_shader,
				entry_point: "fragment",
				compilation_options: PipelineCompilationOptions::default(),
				targets: &[Some(ColorTargetState {
					format: config.format,
					blend: Some(BlendState::ALPHA_BLENDING),
					write_mask: ColorWrites::ALL,
				})],
			}),
			multiview: None,
			cache: None,
		});

		let debug_line_shader = device.create_shader_module(include_wgsl!("debug_line.wgsl"));

		let debug_line_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
			label: Some("Debug Renderer > Pipeline Layout"),
			bind_group_layouts: &[],
			push_constant_ranges: &[
				PushConstantRange {
					stages: ShaderStages::VERTEX,
					range: 0..96,
				},
				PushConstantRange {
					stages: ShaderStages::FRAGMENT,
					range: 96..112,
				},
			],
		});

		let debug_line_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
			label: Some("Debug Renderer > Pipeline"),
			layout: Some(&debug_line_pipeline_layout),
			vertex: VertexState {
				module: &debug_line_shader,
				entry_point: "vertex",
				compilation_options: PipelineCompilationOptions::default(),
				buffers: &[],
			},
			primitive: PrimitiveState {
				topology: LineList,
				strip_index_format: None,
				front_face: Ccw,
				cull_mode: None,
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
				module: &debug_line_shader,
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

		let debug_state = EguiState::new(
			Context::default(),
			ViewportId::default(),
			&window,
			None,
			None,
			None,
		);
		let egui_renderer = EguiRenderer::new(&device, config.format, Some(Depth32Float), 1, false);

		info!(
			"Renderer initialized in {:.0?}",
			Instant::now() - start_time
		);

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

			perspective: Perspective3::new(
				width as f32 / height as f32,
				f32::to_radians(90.0),
				0.05,
				f32::MAX,
			),

			chunk_pipeline,
			terrain_textures_bind_group,

			structure_block_pipeline,
			structure_block_data,
			structure_block_bind_group,

			debug_line_pipeline,
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
		self.depth_buffer_view = self
			.depth_buffer
			.create_view(&TextureViewDescriptor::default());

		self.perspective.set_aspect(width as f32 / height as f32);
	}

	pub fn build_debug_text(&mut self, debug_text: &mut String) {
		writeln!(
			debug_text,
			"{} FPS ({:.0?}/frame)",
			self.frames_per_second, self.frame_time_average
		)
		.expect("should be able to write to string");
	}

	pub fn render(&mut self, cl_args: &ClArgs, state: &mut AnyState, debug_text: String) {
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
				debug_text.trim_end(),
			);
		});

		self.egui_state
			.handle_platform_output(&self.window, gui_output.platform_output);

		let paint_jobs = self
			.egui_state
			.egui_ctx()
			.tessellate(gui_output.shapes, 1.0);
		let screen_descriptor = &ScreenDescriptor {
			size_in_pixels: [self.config.width, self.config.height],
			pixels_per_point: 1.0, // Don't know how to calculate this, come back to it later.
		};

		for (id, image_delta) in gui_output.textures_delta.set {
			self.egui_renderer
				.update_texture(&self.device, &self.queue, id, &image_delta);
		}

		let view = output
			.texture
			.create_view(&TextureViewDescriptor::default());
		let mut encoder = self
			.device
			.create_command_encoder(&CommandEncoderDescriptor::default());

		self.egui_renderer.update_buffers(
			&self.device,
			&self.queue,
			&mut encoder,
			&paint_jobs,
			&screen_descriptor,
		);

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

		self.frames_per_second =
			(self.frame_times.len() as f64 / self.frame_time_total.as_secs_f64()).round() as usize;

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

			#[cfg(debug)]
			Self::GuiTest(_) => return,
		}
		.render(renderer, render_pass)
	}
}

impl Render for Login {}

impl Render for Sector {
	// To anyone that may be reading this code and is experienced, I am well aware this is *terrible*. It's all prototype code though so I
	// am not dealing with it for now.
	//
	// To anyone new to graphics programming, take what you see here as an example of what not to do.
	fn render(&mut self, renderer: &mut Renderer, render_pass: &mut RenderPass) {
		if !self.inventory_gui_open {
			let _ = renderer
				.window
				.set_cursor_grab(CursorGrabMode::Confined)
				.or_else(|_| renderer.window.set_cursor_grab(CursorGrabMode::Locked));
			let _ = renderer.window.set_cursor_visible(false);
			let _ = renderer.window.set_cursor_position(LogicalPosition {
				x: renderer.config.width as f32 / 2.0,
				y: renderer.config.height as f32 / 2.0,
			});
		} else {
			let _ = renderer.window.set_cursor_grab(CursorGrabMode::None);
			let _ = renderer.window.set_cursor_visible(true);
		}

		self.process_messages(&renderer.device);

		let view = self
			.player
			.location
			.rotation
			.to_rotation_matrix()
			.to_homogeneous()
			* Translation3::from(-self.player.location.position.coords).to_homogeneous();
		let camera_matrix = renderer.perspective.to_homogeneous() * view;

		render_pass.set_pipeline(&renderer.chunk_pipeline);
		render_pass.set_push_constants(ShaderStages::VERTEX, 0, cast_slice(&[camera_matrix]));
		render_pass.set_bind_group(0, &renderer.terrain_textures_bind_group, &[]);

		// This should all be indirect multi-draw
		for chunk in self.chunks.iter() {
			// Currently broken, will fix later
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

		render_pass.set_pipeline(&renderer.structure_block_pipeline);

		// Not sure why this is getting cleared? But oh well.
		render_pass.set_push_constants(ShaderStages::VERTEX, 0, cast_slice(&[camera_matrix]));

		// This should also be indirect multi-draw
		for structure in &self.structures {
			for (position, block) in structure.iter_blocks() {
				let mut location = *structure.get_location(&self.physics);
				location.append_translation_mut(&Translation3::from(position.cast()));

				// Yes, we are going to allocate a temporary buffer for every. single. block.
				// This is how you're supposed to do things... right? *It's not*
				let mut instance_buffer_data = [0u8; 68];
				instance_buffer_data[..64]
					.copy_from_slice(cast_slice(&[location.to_homogeneous()]));
				instance_buffer_data[64..].copy_from_slice(cast_slice(&[1.0f32]));

				let instance_buffer = renderer.device.create_buffer_init(&BufferInitDescriptor {
					label: Some("GPU Torture Buffer"),
					contents: instance_buffer_data.as_slice(),
					usage: BufferUsages::VERTEX,
				});

				let block_data = &renderer.structure_block_data[&block.typ];

				render_pass.set_vertex_buffer(0, block_data.positions.slice(..));
				render_pass.set_vertex_buffer(1, block_data.texture_coordinates.slice(..));
				render_pass.set_vertex_buffer(2, instance_buffer.slice(..));
				render_pass.set_index_buffer(block_data.indices.slice(..), IndexFormat::Uint32);
				render_pass.set_bind_group(0, &renderer.structure_block_bind_group, &[]);
				render_pass.draw_indexed(0..block_data.index_count, 0, 0..1);
			}
		}

		// Draw a block to act as a placement indicator
		let location = Isometry3::<f32>::from(
			self.player.location.position
				+ (self
					.player
					.location
					.rotation
					.inverse_transform_vector(&-Vector3::z())
					* 3.0),
		);
		let mut instance_buffer_data = [0u8; 68];
		instance_buffer_data[..64].copy_from_slice(cast_slice(&[location.to_homogeneous()]));
		instance_buffer_data[64..].copy_from_slice(cast_slice(&[0.25f32]));

		let instance_buffer = renderer.device.create_buffer_init(&BufferInitDescriptor {
			label: Some("GPU Torture Buffer"),
			contents: instance_buffer_data.as_slice(),
			usage: BufferUsages::VERTEX,
		});

		let block_data = &renderer.structure_block_data[&BlockType::Block];

		render_pass.set_vertex_buffer(0, block_data.positions.slice(..));
		render_pass.set_vertex_buffer(1, block_data.texture_coordinates.slice(..));
		render_pass.set_vertex_buffer(2, instance_buffer.slice(..));
		render_pass.set_index_buffer(block_data.indices.slice(..), IndexFormat::Uint32);
		render_pass.set_bind_group(0, &renderer.structure_block_bind_group, &[]);
		render_pass.draw_indexed(0..block_data.index_count, 0, 0..1);

		// The dumbest debug line drawer you will ever see.
		// This is the definition of temporary code.
		render_pass.set_pipeline(&renderer.debug_line_pipeline);
		render_pass.set_push_constants(ShaderStages::VERTEX, 0, cast_slice(&[camera_matrix]));

		let color = vector![1.0f32, 1.0, 1.0];
		render_pass.set_push_constants(ShaderStages::FRAGMENT, 96, cast_slice(&[color]));

		// Oh you thought structure block rendering was bad? You haven't seen nothing yet.
		// *GPU bandwidth screams in pain*
		for structure in &self.structures {
			let location = structure.get_location(&self.physics);

			let position_a = location.translation.vector + vector![1.0, 0.0, 0.0];
			let position_b = location.translation.vector - vector![1.0, 0.0, 0.0];
			render_pass.set_push_constants(ShaderStages::VERTEX, 64, cast_slice(&[position_a]));
			render_pass.set_push_constants(ShaderStages::VERTEX, 80, cast_slice(&[position_b]));
			render_pass.draw(0..2, 0..1);

			let position_a = location.translation.vector + vector![0.0, 1.0, 0.0];
			let position_b = location.translation.vector - vector![0.0, 1.0, 0.0];
			render_pass.set_push_constants(ShaderStages::VERTEX, 64, cast_slice(&[position_a]));
			render_pass.set_push_constants(ShaderStages::VERTEX, 80, cast_slice(&[position_b]));
			render_pass.draw(0..2, 0..1);

			let position_a = location.translation.vector + vector![0.0, 0.0, 1.0];
			let position_b = location.translation.vector - vector![0.0, 0.0, 1.0];
			render_pass.set_push_constants(ShaderStages::VERTEX, 64, cast_slice(&[position_a]));
			render_pass.set_push_constants(ShaderStages::VERTEX, 80, cast_slice(&[position_b]));
			render_pass.draw(0..2, 0..1);
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
