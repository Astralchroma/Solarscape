use nalgebra::Vector3;
use rapier3d::dynamics::{
	CCDSolver, ImpulseJointHandle, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointHandle,
	MultibodyJointSet, RigidBody, RigidBodyHandle, RigidBodySet,
};
use rapier3d::geometry::{Collider, ColliderHandle, ColliderSet, DefaultBroadPhase, NarrowPhase};
use rapier3d::pipeline::PhysicsPipeline;
use std::ops::{Deref, DerefMut};
use tokio::sync::mpsc::{unbounded_channel as channel, UnboundedReceiver as Receiver, UnboundedSender as Sender};

pub struct Physics {
	handle_drop_receiver: Receiver<HandleDrop>,
	handle_drop_sender: Sender<HandleDrop>,

	pipeline: PhysicsPipeline,
	integration_parameters: IntegrationParameters,
	islands: IslandManager,
	broad_phase: DefaultBroadPhase,
	narrow_phase: NarrowPhase,
	rigid_bodies: RigidBodySet,
	colliders: ColliderSet,
	impulse_joints: ImpulseJointSet,
	multibody_joints: MultibodyJointSet,
	ccd_solver: CCDSolver,
}

impl Physics {
	pub fn new() -> Self {
		let (handle_drop_sender, handle_drop_receiver) = channel();

		Self {
			handle_drop_receiver,
			handle_drop_sender,

			pipeline: PhysicsPipeline::default(),
			integration_parameters: IntegrationParameters::default(),
			islands: IslandManager::default(),
			broad_phase: DefaultBroadPhase::default(),
			narrow_phase: NarrowPhase::default(),
			rigid_bodies: RigidBodySet::default(),
			colliders: ColliderSet::default(),
			impulse_joints: ImpulseJointSet::default(),
			multibody_joints: MultibodyJointSet::default(),
			ccd_solver: CCDSolver::default(),
		}
	}

	pub fn tick(&mut self, delta: f32) {
		self.integration_parameters.dt = delta;

		// Err variant is ignored, an error can either be:
		// TryRecvError::Empty - There are no more messages, at which point we will break from the loop and continue on
		// TryRecvError::Disconnected - This is impossible as we also hold a Sender
		while let Ok(handle_drop) = self.handle_drop_receiver.try_recv() {
			match handle_drop {
				HandleDrop::Collider(handle) => {
					self.colliders
						.remove(handle, &mut self.islands, &mut self.rigid_bodies, false);
				}
				HandleDrop::RigidBody(handle) => {
					self.rigid_bodies.remove(
						handle,
						&mut self.islands,
						&mut self.colliders,
						&mut self.impulse_joints,
						&mut self.multibody_joints,
						true,
					);
				}
				HandleDrop::ImpulseJoint(handle) => {
					self.impulse_joints.remove(handle, false);
				}
				HandleDrop::MultibodyJoint(handle) => {
					self.multibody_joints.remove(handle, false);
				}
			}
		}

		self.pipeline.step(
			&Vector3::zeros(),
			&self.integration_parameters,
			&mut self.islands,
			&mut self.broad_phase,
			&mut self.narrow_phase,
			&mut self.rigid_bodies,
			&mut self.colliders,
			&mut self.impulse_joints,
			&mut self.multibody_joints,
			&mut self.ccd_solver,
			None,
			&(),
			&(),
		);
	}

	pub fn insert_rigid_body(&mut self, rigid_body: impl Into<RigidBody>) -> AutoCleanup<RigidBodyHandle> {
		AutoCleanup {
			handle: self.rigid_bodies.insert(rigid_body),
			handle_drop_sender: self.handle_drop_sender.clone(),
		}
	}

	pub fn insert_rigid_body_collider(
		&mut self,
		rigid_body_handle: RigidBodyHandle,
		collider: impl Into<Collider>,
	) -> AutoCleanup<ColliderHandle> {
		AutoCleanup {
			handle: self
				.colliders
				.insert_with_parent(collider, rigid_body_handle, &mut self.rigid_bodies),
			handle_drop_sender: self.handle_drop_sender.clone(),
		}
	}
}

enum HandleDrop {
	Collider(ColliderHandle),
	RigidBody(RigidBodyHandle),
	ImpulseJoint(ImpulseJointHandle),
	MultibodyJoint(MultibodyJointHandle),
}

impl From<ColliderHandle> for HandleDrop {
	fn from(handle: ColliderHandle) -> Self {
		Self::Collider(handle)
	}
}

impl From<RigidBodyHandle> for HandleDrop {
	fn from(handle: RigidBodyHandle) -> Self {
		Self::RigidBody(handle)
	}
}

impl From<ImpulseJointHandle> for HandleDrop {
	fn from(handle: ImpulseJointHandle) -> Self {
		Self::ImpulseJoint(handle)
	}
}

impl From<MultibodyJointHandle> for HandleDrop {
	fn from(handle: MultibodyJointHandle) -> Self {
		Self::MultibodyJoint(handle)
	}
}

#[allow(private_bounds)] // Don't really want to expose HandleDrop
pub struct AutoCleanup<T: Into<HandleDrop> + Copy> {
	pub handle: T,
	handle_drop_sender: Sender<HandleDrop>,
}

impl<T: Into<HandleDrop> + Copy> Deref for AutoCleanup<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.handle
	}
}

impl<T: Into<HandleDrop> + Copy> DerefMut for AutoCleanup<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.handle
	}
}

impl<T: Into<HandleDrop> + Copy> Drop for AutoCleanup<T> {
	fn drop(&mut self) {
		// If this is an error, then the Physics and whatever this handle was pointing to has already been dropped
		let _ = self.handle_drop_sender.send(self.handle.into());
	}
}
