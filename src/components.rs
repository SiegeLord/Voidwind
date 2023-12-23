use crate::sprite;
use allegro::*;
use na::{Point3, Vector3};
use nalgebra as na;
use rand::prelude::*;

#[derive(Debug, Copy, Clone)]
pub struct Position
{
	pub pos: Point3<f32>,
	pub dir: f32,
}

#[derive(Debug, Copy, Clone)]
pub struct Velocity
{
	pub vel: Vector3<f32>,
	pub dir_vel: f32,
}

#[derive(Debug, Copy, Clone)]
pub struct Waypoint
{
	pub pos: Point3<f32>,
	pub marker: Option<hecs::Entity>,
}

#[derive(Clone, Debug)]
pub struct Target
{
	pub waypoints: Vec<Waypoint>,
}

impl Target
{
	pub fn clear<T: FnMut(hecs::Entity) -> ()>(&mut self, mut to_die_fn: T)
	{
		for w in &self.waypoints
		{
			if let Some(marker) = w.marker
			{
				to_die_fn(marker);
			}
		}
		self.waypoints.clear();
	}
}

#[derive(Clone, Debug)]
pub struct Mesh
{
	pub mesh: String,
}

#[derive(Clone, Debug)]
pub struct AI;

#[derive(Clone, Debug)]
pub struct Stats
{
	pub speed: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct Solid
{
	pub size: f32,
	pub mass: f32,
}

impl Solid
{
	pub fn collides_with(&self, _other: &Solid) -> bool
	{
		true
	}
}
