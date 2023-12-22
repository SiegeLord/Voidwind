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
	pub marker: hecs::Entity,
}

#[derive(Debug, Clone)]
pub struct Target
{
	pub waypoints: Vec<Waypoint>,
}

#[derive(Debug, Clone)]
pub struct Mesh
{
	pub mesh: String,
}
