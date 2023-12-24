use crate::sprite;
use allegro::*;
use na::{Point2, Point3, Vector3};
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
pub enum AIState
{
	Idle,
	Pursuing(hecs::Entity),
	Attacking(hecs::Entity),
}

#[derive(Clone, Debug)]
pub struct AI
{
	pub state: AIState,
}

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

#[derive(Clone, Debug)]
pub struct WeaponStats
{
	pub fire_interval: f32,
	pub arc: f32,
}

#[derive(Clone, Debug)]
pub struct Weapon
{
	pub want_to_fire: bool,
	pub time_to_fire: f64,
	pub stats: WeaponStats,
}

impl Weapon
{
	pub fn new(stats: WeaponStats) -> Self
	{
		Self {
			want_to_fire: false,
			time_to_fire: stats.fire_interval as f64,
			stats: stats,
		}
	}
}

#[derive(Clone, Debug)]
pub enum ItemKind
{
	Weapon(Weapon),
}

#[derive(Clone, Debug)]
pub struct Item
{
	pub kind: ItemKind,
}

#[derive(Clone, Debug)]
pub struct ItemSlot
{
	pub item: Option<Item>,
	pub pos: Point2<f32>,
	pub dir: f32,
}

#[derive(Clone, Debug)]
pub struct Equipment
{
	pub slots: Vec<ItemSlot>,
	pub want_action_1: bool,
	pub target_pos: Point3<f32>,
	pub allow_out_of_arc_shots: bool,
}

#[derive(Debug, Clone)]
pub struct TimeToDie
{
	pub time_to_die: f64,
}

#[derive(Debug, Clone)]
pub struct AffectedByGravity;

#[derive(Debug, Clone)]
pub struct CollidesWithWater;
