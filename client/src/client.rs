use crate::{login::Login, renderer::Renderer, world::Sector, ClArgs};
use egui::Context;
use std::fmt::Write;
use winit::{
	application::ApplicationHandler,
	event::{DeviceEvent, DeviceId, WindowEvent},
	event_loop::ActiveEventLoop,
	window::WindowId,
};

#[cfg(debug)]
use crate::gui_test::GuiTest;

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

	fn window_event(
		&mut self,
		event_loop: &ActiveEventLoop,
		window_id: WindowId,
		event: WindowEvent,
	) {
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

				let mut debug_text = String::new();
				writeln!(
					debug_text,
					"Solarscape (Client) v{}",
					env!("CARGO_PKG_VERSION")
				)
				.expect("should be able to write to a string");

				renderer.build_debug_text(&mut debug_text);
				self.state.build_debug_text(&mut debug_text);

				renderer.render(&self.cl_args, &mut self.state, debug_text);
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
			state: {
				#[cfg(debug)]
				match cl_args.gui_test {
					true => AnyState::GuiTest(GuiTest::default()),
					false => AnyState::Login(Login::from_cl_args(&mut cl_args)),
				}

				#[cfg(not(debug))]
				AnyState::Login(Login::default())
			},

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

	fn build_debug_text(&mut self, debug_text: &mut String) {}

	fn draw_ui(&mut self, cl_args: &ClArgs, context: &Context) {}

	fn window_event(&mut self, event: &WindowEvent) {}

	fn device_event(&mut self, event: &DeviceEvent) {}
}

pub enum AnyState {
	Login(Login),
	Sector(Sector),

	#[cfg(debug)]
	GuiTest(crate::gui_test::GuiTest),
}

impl State for AnyState {
	fn build_debug_text(&mut self, debug_text: &mut String) {
		match self {
			Self::Login(state) => state as &mut dyn State,
			Self::Sector(state) => state as &mut dyn State,

			#[cfg(debug)]
			Self::GuiTest(state) => state as &mut dyn State,
		}
		.build_debug_text(debug_text)
	}

	fn draw_ui(&mut self, cl_args: &ClArgs, context: &Context) {
		match self {
			Self::Login(state) => state as &mut dyn State,
			Self::Sector(state) => state as &mut dyn State,

			#[cfg(debug)]
			Self::GuiTest(state) => state as &mut dyn State,
		}
		.draw_ui(cl_args, context)
	}

	fn tick(&mut self) -> Option<AnyState> {
		match self {
			Self::Login(state) => state as &mut dyn State,
			Self::Sector(state) => state as &mut dyn State,

			#[cfg(debug)]
			Self::GuiTest(state) => state as &mut dyn State,
		}
		.tick()
	}

	fn window_event(&mut self, event: &WindowEvent) {
		match self {
			Self::Login(state) => state as &mut dyn State,
			Self::Sector(state) => state as &mut dyn State,

			#[cfg(debug)]
			Self::GuiTest(state) => state as &mut dyn State,
		}
		.window_event(event)
	}

	fn device_event(&mut self, event: &DeviceEvent) {
		match self {
			Self::Login(state) => state as &mut dyn State,
			Self::Sector(state) => state as &mut dyn State,

			#[cfg(debug)]
			Self::GuiTest(state) => state as &mut dyn State,
		}
		.device_event(event)
	}
}
