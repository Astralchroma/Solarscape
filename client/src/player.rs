use nalgebra::{vector, IsometryMatrix3, Rotation3, Translation3};
use solarscape_shared::connection::{ClientEnd, Connection};
use std::{ops::Deref, ops::DerefMut};
use winit::event::{DeviceEvent, ElementState, KeyEvent, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey::Code};

/// Locality is used to distinguish between Local and Remote players, though Remote
/// doesn't currently exist as there is not yet any syncing of players on the server.
pub trait Locality {}

pub struct Player<L: Locality> {
	pub location: IsometryMatrix3<f32>,

	locality: L,
}

impl<L: Locality> Deref for Player<L> {
	type Target = L;

	fn deref(&self) -> &Self::Target {
		&self.locality
	}
}

impl<L: Locality> DerefMut for Player<L> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.locality
	}
}

pub struct Local {
	pub connection: Connection<ClientEnd>,

	left_state: OppositeKeyState,
	right_state: OppositeKeyState,

	forward_state: OppositeKeyState,
	backward_state: OppositeKeyState,

	up_state: OppositeKeyState,
	down_state: OppositeKeyState,

	roll_left_state: OppositeKeyState,
	roll_right_state: OppositeKeyState,
}

enum OppositeKeyState {
	// Key was pressed down while it's opposite was released
	PressedFirst,

	// Key was pressed down while it's opposite was also pressed, so it has priority
	PressedSecond,

	// Key is not pressed down
	Released,
}

impl Locality for Local {}

impl Player<Local> {
	pub fn new(connection: Connection<ClientEnd>) -> Self {
		Self {
			location: IsometryMatrix3::translation(0.0, 0.0, 0.0),

			locality: Local {
				connection,

				left_state: OppositeKeyState::Released,
				right_state: OppositeKeyState::Released,

				forward_state: OppositeKeyState::Released,
				backward_state: OppositeKeyState::Released,

				up_state: OppositeKeyState::Released,
				down_state: OppositeKeyState::Released,

				roll_left_state: OppositeKeyState::Released,
				roll_right_state: OppositeKeyState::Released,
			},
		}
	}

	pub fn translate_local(&mut self, translation: Translation3<f32>) {
		self.location.append_translation_mut(&translation);
	}

	pub fn rotate(&mut self, rotation: Rotation3<f32>) {
		self.location.append_rotation_mut(&rotation)
	}

	pub fn handle_window_event(&mut self, event: WindowEvent) {
		if let WindowEvent::KeyboardInput {
			event: KeyEvent {
				physical_key,
				state,
				repeat: false,
				..
			},
			..
		} = event
		{
			if let Code(code) = physical_key {
				// Really this should be a function, but borrowing rules got in the way
				macro_rules! handle_key_state {
					($old_state:expr, $other_state:expr) => {
						match state {
							ElementState::Pressed => match $other_state {
								OppositeKeyState::PressedFirst => $old_state = OppositeKeyState::PressedSecond,

								// Technically an invalid state, oh well
								OppositeKeyState::PressedSecond => {
									$other_state = OppositeKeyState::PressedFirst;
									$old_state = OppositeKeyState::PressedSecond;
								}

								OppositeKeyState::Released => $old_state = OppositeKeyState::PressedFirst,
							},
							ElementState::Released => match $other_state {
								OppositeKeyState::PressedFirst => $old_state = OppositeKeyState::Released,

								OppositeKeyState::PressedSecond => {
									$other_state = OppositeKeyState::PressedFirst;
									$old_state = OppositeKeyState::Released;
								}

								OppositeKeyState::Released => $old_state = OppositeKeyState::Released,
							},
						}
					};
				}

				match code {
					KeyCode::KeyA => handle_key_state!(self.right_state, self.left_state),
					KeyCode::KeyD => handle_key_state!(self.left_state, self.right_state),

					KeyCode::KeyW => handle_key_state!(self.forward_state, self.backward_state),
					KeyCode::KeyS => handle_key_state!(self.backward_state, self.forward_state),

					KeyCode::KeyR => handle_key_state!(self.up_state, self.down_state),
					KeyCode::KeyF => handle_key_state!(self.down_state, self.up_state),

					KeyCode::KeyQ => handle_key_state!(self.roll_left_state, self.roll_right_state),
					KeyCode::KeyE => handle_key_state!(self.roll_right_state, self.roll_left_state),

					_ => {}
				}
			}
		}
	}

	pub fn handle_device_event(&mut self, event: DeviceEvent) {
		if let DeviceEvent::MouseMotion { delta: (x, y) } = event {
			self.rotate(Rotation3::from_euler_angles(y as f32 / 1000.0, x as f32 / 1000.0, 0.0));
		}
	}

	pub fn tick(&mut self, delta: f32) {
		fn key_state_to_float(negative_state: &OppositeKeyState, positive_state: &OppositeKeyState) -> f32 {
			match negative_state {
				OppositeKeyState::PressedFirst => match positive_state {
					OppositeKeyState::PressedSecond => 1.0,
					_ => -1.0,
				},
				OppositeKeyState::PressedSecond => -1.0,
				OppositeKeyState::Released => match positive_state {
					OppositeKeyState::Released => 0.0,
					_ => 1.0,
				},
			}
		}

		let mut translation = vector![
			key_state_to_float(&self.left_state, &self.right_state),
			key_state_to_float(&self.up_state, &self.down_state),
			key_state_to_float(&self.backward_state, &self.forward_state),
		];

		if translation.normalize_mut().is_normal() {
			translation *= delta * 10.0;
			self.translate_local(translation.into());
		}

		let rotation = Rotation3::from_euler_angles(
			0.0,
			0.0,
			key_state_to_float(&self.roll_left_state, &self.roll_right_state) * delta,
		);

		self.rotate(rotation);

		// Lol, apparently we never accounted for the fact that players move, so the server
		// will unload everything and load it again, re-sending everything to the client
		// self.connection.send(self.location);
	}
}
