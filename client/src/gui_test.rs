#![allow(unused)]

use crate::{client::AnyState, client::State, ClArgs};
use egui::{Align2, Context, Window};

#[derive(Default)]
pub struct GuiTest {}

impl State for GuiTest {
	fn tick(&mut self) -> Option<AnyState> {
		None
	}

	fn draw_ui(&mut self, cl_args: &ClArgs, context: &Context) {
		Window::new("Gui Test")
			.anchor(Align2::CENTER_CENTER, (0.0, 0.0))
			.resizable(false)
			.collapsible(false)
			.auto_sized()
			.max_width(400.0)
			.show(context, |window| {
				window.label("Hello, World!\n\nThis is an experimental space for designing new UIs without having to worry about game state straight away, it is only avalible in debug builds, and is accessible through the --gui-test command line flag.");
			});
	}
}
