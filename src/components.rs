use crate::sprite;
use allegro::*;
use na::{Point2, Point3, Vector3};
use nalgebra as na;
use rand::prelude::*;

use std::f32::consts::PI;

#[derive(Copy, Clone, Debug)]
pub struct Position
{
	pub pos: Point3<f32>,
	pub dir: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct Tilt
{
	pub tilt: f32,
	pub target_tilt: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct Velocity
{
	pub vel: Vector3<f32>,
	pub dir_vel: f32,
}

#[derive(Copy, Clone, Debug)]
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
	Pause
	{
		time_to_unpause: f64,
	},
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
pub enum CollideKind
{
	Big,
	Small,
}

impl CollideKind
{
	pub fn collides_with(&self, other: &CollideKind) -> bool
	{
		match (self, other)
		{
			(CollideKind::Big, CollideKind::Big) => true,
			(CollideKind::Big, CollideKind::Small) => true,
			(CollideKind::Small, CollideKind::Big) => true,
			(CollideKind::Small, CollideKind::Small) => false,
		}
	}
}

#[derive(Copy, Clone, Debug)]
pub struct Solid
{
	pub size: f32,
	pub mass: f32,
	pub kind: CollideKind,
	pub parent: Option<hecs::Entity>,
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

impl ItemKind
{
	pub fn description(&self) -> String
	{
		match self
		{
			ItemKind::Weapon(weapon) =>
			{
				let fire_interval = weapon.stats.fire_interval;
				let arc = (weapon.stats.arc / PI * 180.) as i32;
				[
					format!("Cannon"),
					"".into(),
					format!("Reload Time: {fire_interval:.1} sec"),
					format!("Arc: {arc}°"),
				]
				.join("\n")
			}
		}
	}
}

#[derive(Clone, Debug)]
pub struct Item
{
	pub kind: ItemKind,
	pub price: i32,
}

#[derive(Clone, Debug)]
pub struct ItemSlot
{
	pub item: Option<Item>,
	pub pos: Point2<f32>,
	pub dir: Option<f32>,
	pub is_inventory: bool,
}

#[derive(Clone, Debug)]
pub struct Equipment
{
	pub slots: Vec<ItemSlot>,
	pub want_action_1: bool,
	pub target_pos: Point3<f32>,
	pub allow_out_of_arc_shots: bool,
}

impl Equipment
{
	pub fn new(
		inventory_size: usize, allow_out_of_arc_shots: bool, mut slots: Vec<ItemSlot>,
	) -> Self
	{
		for i in 0..inventory_size
		{
			let x = (i as i32 % 4) as f32 - 1.5;
			let y = (i as i32 / 4) as f32 + 2.;
			slots.push(ItemSlot {
				item: None,
				pos: Point2::new(-y, -x),
				dir: None,
				is_inventory: true,
			})
		}
		Self {
			slots: slots,
			want_action_1: false,
			target_pos: Point3::origin(),
			allow_out_of_arc_shots: allow_out_of_arc_shots,
		}
	}
}

#[derive(Clone, Debug)]
pub struct TimeToDie
{
	pub time_to_die: f64,
}

#[derive(Clone, Debug)]
pub struct AffectedByGravity;

#[derive(Clone, Debug)]
pub struct CollidesWithWater;

#[derive(Copy, Clone, Debug)]
pub struct Damage
{
	pub damage: f32,
	pub team: Team,
}

#[derive(Copy, Clone, Debug)]
pub enum ContactEffect
{
	Die,
	Hurt
	{
		damage: Damage,
	},
}

#[derive(Clone, Debug)]
pub struct OnContactEffect
{
	pub effects: Vec<ContactEffect>,
}

#[derive(Clone, Debug)]
pub struct ShipState
{
	pub hull: f32,
	pub crew: i32,
	pub team: Team,
}

impl ShipState
{
	pub fn damage(&mut self, damage: &Damage) -> bool
	{
		if damage.team.can_damage(&self.team)
		{
			self.hull -= damage.damage;
			self.crew -= 1;
			self.hull = self.hull.max(0.);
			self.crew = self.crew.max(0);
			true
		}
		else
		{
			false
		}
	}

	pub fn is_active(&self) -> bool
	{
		self.is_structurally_sound() && self.has_crew()
	}

	pub fn is_structurally_sound(&self) -> bool
	{
		self.hull > 0.
	}

	pub fn has_crew(&self) -> bool
	{
		self.crew > 0
	}
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Team
{
	English,
	French,
	Neutral,
}

impl Team
{
	pub fn is_enemy(&self, other: &Team) -> bool
	{
		if *self == Team::Neutral || *other == Team::Neutral
		{
			false
		}
		else
		{
			*self != *other
		}
	}

	pub fn trade_with(&self, other: &Team) -> bool
	{
		if *self == Team::Neutral || *other == Team::Neutral
		{
			false
		}
		else
		{
			*self == *other
		}
	}

	pub fn dock_with(&self, other: &Team) -> bool
	{
		if *self == Team::Neutral || *other == Team::Neutral
		{
			true
		}
		else
		{
			*self == *other
		}
	}

	pub fn can_damage(&self, other: &Team) -> bool
	{
		*self != *other
	}
}
