use crate::world::Sector;
use crate::{login::Login, renderer::Renderer, ClArgs};
use egui::Context;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::{application::ApplicationHandler, event_loop::ActiveEventLoop, window::WindowId};

pub struct Client {
	renderer: Option<Renderer>,
	state: AnyState,

	pub cl_args: ClArgs,
}

impl ApplicationHandler for Client {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		self.renderer = match Renderer::new(event_loop) {
			Ok(renderer) => Some(renderer),
			Err(error) => panic!("{error}"),
		};
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
		let renderer = match &mut self.renderer {
			Some(renderer) if renderer.window.id() != window_id => return,
			Some(renderer) => renderer,
			None => return,
		};

		match event {
			WindowEvent::Resized(size) => renderer.resize(size),
			WindowEvent::CloseRequested | WindowEvent::Destroyed => event_loop.exit(),
			WindowEvent::RedrawRequested => {
				loop {
					if let Some(new_state) = self.state.tick() {
						self.state = new_state;
					} else {
						break;
					}
				}

				renderer.render(&self.cl_args, &mut self.state);
			}
			_ => {
				self.state.window_event(&event);
				renderer.handle_window_event(&event);
			}
		}
	}

	fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
		self.state.device_event(&event)
	}

	// This should only ever be called on iOS, Android, and Web, none of which we support, so this is untested.
	fn suspended(&mut self, _: &ActiveEventLoop) {
		self.renderer = None;
	}

	fn exiting(&mut self, _: &ActiveEventLoop) {
		// We have to do this otherwise we segfault once we exit the event loop
		self.renderer = None;
	}
}

impl From<ClArgs> for Client {
	fn from(mut cl_args: ClArgs) -> Self {
		Self {
			state: AnyState::Login(match cfg!(debug) {
				true => Login::from_cl_args(&mut cl_args),
				false => Login::default(),
			}),

			renderer: None,

			cl_args,
		}
	}
}

#[allow(unused_variables)]
pub trait State {
	fn tick(&mut self) -> Option<AnyState> {
		None
	}

	fn draw_ui(&mut self, cl_args: &ClArgs, context: &Context) {}

	fn window_event(&mut self, event: &WindowEvent) {}

	fn device_event(&mut self, event: &DeviceEvent) {}
}

pub enum AnyState {
	Login(Login),
	Sector(Sector),
}

impl State for AnyState {
	fn tick(&mut self) -> Option<AnyState> {
		match self {
			Self::Login(state) => state as &mut dyn State,
			Self::Sector(state) => state as &mut dyn State,
		}
		.tick()
	}

	fn draw_ui(&mut self, cl_args: &ClArgs, context: &Context) {
		match self {
			Self::Login(state) => state as &mut dyn State,
			Self::Sector(state) => state as &mut dyn State,
		}
		.draw_ui(cl_args, context)
	}

	fn window_event(&mut self, event: &WindowEvent) {
		match self {
			Self::Login(state) => state as &mut dyn State,
			Self::Sector(state) => state as &mut dyn State,
		}
		.window_event(event)
	}

	fn device_event(&mut self, event: &DeviceEvent) {
		match self {
			Self::Login(state) => state as &mut dyn State,
			Self::Sector(state) => state as &mut dyn State,
		}
		.device_event(event)
	}
}
