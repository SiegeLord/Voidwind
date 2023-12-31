use crate::{game_state, sprite};
use allegro::*;
use na::{Point2, Point3, Vector3};
use nalgebra as na;
use rand::prelude::*;
use serde_derive::{Deserialize, Serialize};

use std::f32::consts::PI;

pub fn level_effectiveness(level: i32) -> f32
{
	(level as f32).powf(0.5)
}

pub fn level_experience(level: i32) -> f32
{
	(level as f32).powf(3.)
}

pub fn enemy_experience(level: i32) -> f32
{
	2. * (level as f32).powf(2.)
}

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
	pub name: String,
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
pub enum Rarity
{
	Normal,
	Magic,
	Rare,
}

#[derive(Clone, Debug)]
pub enum OfficerPrefix
{
	Rapid(usize, f32),
	Speed(usize, f32),
	Accurate(usize, f32),
	Critical(usize, f32),
}

pub const OFFICER_PREFIX_WEIGHTS: [i32; 4] = [4, 1, 10, 4];

impl OfficerPrefix
{
	pub fn name(&self) -> &'static str
	{
		match self
		{
			OfficerPrefix::Rapid(tier, _) => match tier
			{
				0 => "Wired ",
				1 => "Coffeed ",
				2 => "Quicksilvered ",
				_ => unreachable!(),
			},
			OfficerPrefix::Speed(tier, _) => match tier
			{
				0 => "Piloting ",
				1 => "Navigational ",
				2 => "Cartographic ",
				_ => unreachable!(),
			},
			OfficerPrefix::Accurate(tier, _) => match tier
			{
				0 => "Sparrow-eyed ",
				1 => "Crow-eyed ",
				2 => "Eagle-eyed ",
				_ => unreachable!(),
			},
			OfficerPrefix::Critical(tier, _) => match tier
			{
				0 => "Spectacled ",
				1 => "Telescoped ",
				2 => "Sectanted ",
				_ => unreachable!(),
			},
		}
	}

	pub fn apply(&self, stats: &mut DerivedShipStats)
	{
		match *self
		{
			OfficerPrefix::Rapid(tier, f) =>
			{
				let breakpoints = [0.1, 0.3, 0.5, 0.7];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.reload_speed += effect;
			}
			OfficerPrefix::Speed(tier, f) =>
			{
				let breakpoints = [0., 0.1, 0.2, 0.3];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.speed += effect;
			}
			OfficerPrefix::Accurate(tier, f) =>
			{
				let breakpoints = [0.1, 0.2, 0.3, 0.4];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.accuracy += effect;
			}
			OfficerPrefix::Critical(tier, f) =>
			{
				let breakpoints = [0.1, 0.3, 0.5, 0.7];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.critical_chance += effect;
			}
		}
	}
}

#[derive(Clone, Debug)]
pub enum OfficerSuffix
{
	ArmorRepair(usize, f32),
	HullRepair(usize, f32),
	InfirmaryRepair(usize, f32),
	SailRepair(usize, f32),
	ItemProtect(usize, f32),
	Medic(usize, f32),
}

impl OfficerSuffix
{
	pub fn name(&self) -> &'static str
	{
		match self
		{
			OfficerSuffix::ArmorRepair(tier, _) => match tier
			{
				0 => ", Apprentice Armourer",
				1 => ", Junior Armourer",
				2 => ", Master Armourer",
				_ => unreachable!(),
			},
			OfficerSuffix::HullRepair(tier, _) => match tier
			{
				0 => ", Woodworker",
				1 => ", Carpenter",
				2 => ", Artisan",
				_ => unreachable!(),
			},
			OfficerSuffix::InfirmaryRepair(tier, _) => match tier
			{
				0 => " of the Leech",
				1 => " of the Serpent",
				2 => " of the Ambrosia",
				_ => unreachable!(),
			},
			OfficerSuffix::SailRepair(tier, _) => match tier
			{
				0 => " of the Stitching",
				1 => " of the Fid",
				2 => " of the Sail",
				_ => unreachable!(),
			},
			OfficerSuffix::ItemProtect(tier, _) => match tier
			{
				0 => ", Arranger",
				1 => ", Keeper",
				2 => ", Quartermaster",
				_ => unreachable!(),
			},
			OfficerSuffix::Medic(tier, _) => match tier
			{
				0 => " of Mending",
				1 => " of Stiching",
				2 => " of Curing",
				_ => unreachable!(),
			},
		}
	}

	pub fn apply(&self, stats: &mut DerivedShipStats)
	{
		match *self
		{
			OfficerSuffix::ArmorRepair(tier, f) =>
			{
				let breakpoints = [0.1, 0.3, 0.5, 0.7];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.armor_repair += effect;
			}
			OfficerSuffix::HullRepair(tier, f) =>
			{
				let breakpoints = [0., 0.1, 0.2, 0.3];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.hull_repair += effect;
			}
			OfficerSuffix::InfirmaryRepair(tier, f) =>
			{
				let breakpoints = [0.1, 0.2, 0.3, 0.4];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.infirmary_repair += effect;
			}
			OfficerSuffix::SailRepair(tier, f) =>
			{
				let breakpoints = [0.1, 0.3, 0.5, 0.7];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.sail_repair += effect;
			}
			OfficerSuffix::ItemProtect(tier, f) =>
			{
				let breakpoints = [0.1, 0.3, 0.5, 0.7];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.item_protect += effect;
			}
			OfficerSuffix::Medic(tier, f) =>
			{
				let breakpoints = [0.1, 0.3, 0.5, 0.7];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.medic += effect;
			}
		}
	}
}

pub const OFFICER_SUFFIX_WEIGHTS: [i32; 6] = [10, 10, 10, 10, 1, 5];

#[derive(Clone, Debug)]
pub enum WeaponPrefix
{
	Rapid(usize, f32),
	Swivel(usize, f32),
	Fast(usize, f32),
	Accurate(usize, f32),
	CrewSelective(usize, f32),
	SailSelective(usize, f32),
	InfirmarySelective(usize, f32),
	HullSelective(usize, f32),
	Critical(usize, f32),
}

pub const WEAPON_PREFIX_WEIGHTS: [i32; 9] = [1, 10, 10, 10, 20, 5, 5, 5, 2];

impl WeaponPrefix
{
	pub fn name(&self) -> &'static str
	{
		match self
		{
			WeaponPrefix::Rapid(tier, _) => match tier
			{
				0 => "Rapid ",
				1 => "Accelerated ",
				2 => "Electric ",
				_ => unreachable!(),
			},
			WeaponPrefix::Swivel(tier, _) => match tier
			{
				0 => "Rigid ",
				1 => "Swiveled ",
				2 => "Articulated ",
				_ => unreachable!(),
			},
			WeaponPrefix::Fast(tier, _) => match tier
			{
				0 => "Fast ",
				1 => "Winged ",
				2 => "Fleet ",
				_ => unreachable!(),
			},
			WeaponPrefix::Accurate(tier, _) => match tier
			{
				0 => "Accurate ",
				1 => "Polished ",
				2 => "Rifled ",
				_ => unreachable!(),
			},
			WeaponPrefix::CrewSelective(tier, _) => match tier
			{
				0 => "Bloodthirsty ",
				1 => "Meat-loving ",
				2 => "Widow-making ",
				_ => unreachable!(),
			},
			WeaponPrefix::SailSelective(tier, _) => match tier
			{
				0 => "Punching ",
				1 => "Tearing ",
				2 => "Ripping ",
				_ => unreachable!(),
			},
			WeaponPrefix::InfirmarySelective(tier, _) => match tier
			{
				0 => "Cruel ",
				1 => "Ruthless ",
				2 => "Sadistic ",
				_ => unreachable!(),
			},
			WeaponPrefix::HullSelective(tier, _) => match tier
			{
				0 => "Breaking ",
				1 => "Crushing ",
				2 => "Destroying ",
				_ => unreachable!(),
			},
			WeaponPrefix::Critical(tier, _) => match tier
			{
				0 => "Observant ",
				1 => "Calculating ",
				2 => "Eagle-Eyed ",
				_ => unreachable!(),
			},
		}
	}

	pub fn apply(&self, stats: &mut WeaponStats)
	{
		match *self
		{
			WeaponPrefix::Rapid(tier, f) =>
			{
				let breakpoints = [0.9, 0.7, 0.5, 0.3];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.fire_interval *= effect;
			}
			WeaponPrefix::Swivel(tier, f) =>
			{
				let breakpoints = [1.1, 1.3, 1.5, 1.7];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.arc *= effect;
			}
			WeaponPrefix::Fast(tier, f) =>
			{
				let breakpoints = [1.1, 1.3, 1.5, 1.7];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.speed *= effect;
			}
			WeaponPrefix::Accurate(tier, f) =>
			{
				let breakpoints = [0.9, 0.6, 0.3, 0.0];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.spread *= effect;
			}
			WeaponPrefix::CrewSelective(tier, f) =>
			{
				let breakpoints = [1.1, 1.3, 1.5, 1.7];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.crew_weight *= effect;
			}
			WeaponPrefix::SailSelective(tier, f) =>
			{
				let breakpoints = [1.1, 1.3, 1.5, 1.7];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.sail_weight *= effect;
			}
			WeaponPrefix::InfirmarySelective(tier, f) =>
			{
				let breakpoints = [1.1, 1.3, 1.5, 1.7];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.infirmary_weight *= effect;
			}
			WeaponPrefix::HullSelective(tier, f) =>
			{
				let breakpoints = [1.1, 1.3, 1.5, 1.7];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.hull_weight *= effect;
			}
			WeaponPrefix::Critical(tier, f) =>
			{
				let breakpoints = [1.1, 1.4, 1.7, 2.];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.critical_chance *= effect;
			}
		}
	}
}

#[derive(Clone, Debug)]
pub enum WeaponSuffix
{
	OfDamage(usize, f32),
	OfCritMulti(usize, f32),
	OfCrewSlaying(usize, f32),
	OfSailSlaying(usize, f32),
	OfItemSlaying(usize, f32),
	OfArmorSlaying(usize, f32),
}

pub const WEAPON_SUFFIX_WEIGHTS: [i32; 6] = [5, 1, 5, 10, 10, 10];

impl WeaponSuffix
{
	pub fn name(&self) -> &'static str
	{
		match *self
		{
			WeaponSuffix::OfDamage(tier, _) => match tier
			{
				0 => " of the Newt",
				1 => " of the Whale",
				2 => " of the Leviathan",
				_ => unreachable!(),
			},
			WeaponSuffix::OfCritMulti(tier, _) => match tier
			{
				0 => " of Exploitation",
				1 => " of Domination",
				2 => " of Finality",
				_ => unreachable!(),
			},
			WeaponSuffix::OfCrewSlaying(tier, _) => match tier
			{
				0 => " of Misery",
				1 => " of Sickness",
				2 => " of Death",
				_ => unreachable!(),
			},
			WeaponSuffix::OfSailSlaying(tier, _) => match tier
			{
				0 => " of Chafing",
				1 => " of Tattering",
				2 => " of Fraying",
				_ => unreachable!(),
			},
			WeaponSuffix::OfItemSlaying(tier, _) => match tier
			{
				0 => " of Frugality",
				1 => " of Scarcity",
				2 => " of Nothingness",
				_ => unreachable!(),
			},
			WeaponSuffix::OfArmorSlaying(tier, _) => match tier
			{
				0 => " of the Trickle",
				1 => " of Gushing",
				2 => " of Flooding",
				_ => unreachable!(),
			},
		}
	}

	pub fn apply(&self, stats: &mut WeaponStats)
	{
		match *self
		{
			WeaponSuffix::OfDamage(tier, f) =>
			{
				let breakpoints = [1.1, 1.4, 1.7, 2.];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.damage *= effect;
			}
			WeaponSuffix::OfCritMulti(tier, f) =>
			{
				let breakpoints = [1.1, 1.4, 1.7, 2.];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.critical_multiplier *= effect;
			}
			WeaponSuffix::OfCrewSlaying(tier, f) =>
			{
				let breakpoints = [1.1, 1.4, 1.7, 2.];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.crew_damage *= effect;
			}
			WeaponSuffix::OfSailSlaying(tier, f) =>
			{
				let breakpoints = [1.1, 1.4, 1.7, 2.];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.sail_damage *= effect;
			}
			WeaponSuffix::OfItemSlaying(tier, f) =>
			{
				let breakpoints = [1.1, 1.4, 1.7, 2.];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.item_chance *= effect;
			}
			WeaponSuffix::OfArmorSlaying(tier, f) =>
			{
				let breakpoints = [1.1, 1.4, 1.7, 2.];
				let min = breakpoints[tier];
				let max = breakpoints[tier + 1];
				let effect = min + f * (max - min);

				stats.armor_damage *= effect;
			}
		}
	}
}

#[derive(Copy, Clone, Debug)]
pub struct WeaponStats
{
	pub fire_interval: f32,
	pub speed: f32,
	pub arc: f32,
	pub spread: f32,
	pub damage: f32,
	pub critical_chance: f32,
	pub critical_multiplier: f32,
	pub armor_damage: f32,
	pub sail_damage: f32,
	pub crew_damage: f32,
	pub item_chance: f32,
	pub hull_weight: f32,
	pub sail_weight: f32,
	pub crew_weight: f32,
	pub infirmary_weight: f32,
}

#[derive(Clone, Debug)]
pub struct Weapon
{
	pub readiness: f32,
	pub time_to_fire: Option<f64>,
	pub rarity: Rarity,
	pub prefixes: Vec<WeaponPrefix>,
	pub suffixes: Vec<WeaponSuffix>,
	pub name: String,
	pub level: i32,
}

impl Weapon
{
	pub fn stats(&self) -> WeaponStats
	{
		let mut stats = default_weapon_stats(self.level);
		for prefix in &self.prefixes
		{
			prefix.apply(&mut stats);
		}
		for suffix in &self.suffixes
		{
			suffix.apply(&mut stats);
		}
		stats
	}
}

fn default_weapon_stats(level: i32) -> WeaponStats
{
	WeaponStats {
		fire_interval: 1.,
		speed: 50.,
		arc: PI / 2.,
		spread: PI / 12.,
		damage: 10. * level_effectiveness(level),
		critical_chance: 0.05,
		critical_multiplier: 1.,
		armor_damage: 1.,
		sail_damage: 1.,
		crew_damage: 1.,
		item_chance: 1.,
		hull_weight: 1.,
		sail_weight: 0.5,
		crew_weight: 3.,
		infirmary_weight: 1.,
	}
}

#[derive(Clone, Debug)]
pub struct Officer
{
	name: String,
	level: i32,
	prefixes: Vec<OfficerPrefix>,
	suffixes: Vec<OfficerSuffix>,
}

fn mod_string(name: &str, base: f32, new: f32) -> Option<String>
{
	let change = (100. * (new - base) / base) as i32;
	if change == 0
	{
		None
	}
	else
	{
		Some(format!("{name}{change:+}%"))
	}
}

#[derive(Clone, Debug)]
pub enum ItemKind
{
	Weapon(Weapon),
	Goods(i32),
	Cotton(i32),
	Tobacco(i32),
	Officer(Officer),
}

impl ItemKind
{
	pub fn name(&self) -> &str
	{
		match self
		{
			ItemKind::Weapon(weapon) => &weapon.name,
			ItemKind::Goods(_) => "Goods",
			ItemKind::Cotton(_) => "Cotton",
			ItemKind::Tobacco(_) => "Tobacco",
			ItemKind::Officer(officer) => &officer.name,
		}
	}

	pub fn color(&self) -> Color
	{
		match self
		{
			ItemKind::Weapon(weapon) => match weapon.rarity
			{
				Rarity::Normal => Color::from_rgb_f(1., 1., 1.),
				Rarity::Magic => Color::from_rgb_f(0.2, 0.2, 1.),
				Rarity::Rare => Color::from_rgb_f(1., 1., 0.2),
			},
			ItemKind::Goods(_) => Color::from_rgb_f(0.2, 1., 0.2),
			ItemKind::Cotton(_) => Color::from_rgb_f(0.2, 1., 0.2),
			ItemKind::Tobacco(_) => Color::from_rgb_f(0.2, 1., 0.2),
			ItemKind::Officer(_) => Color::from_rgb_f(1., 0.2, 0.2),
		}
	}

	pub fn description(&self) -> String
	{
		match self
		{
			ItemKind::Weapon(weapon) =>
			{
				let stats = weapon.stats();

				let fire_interval = stats.fire_interval;
				let arc = (stats.arc / PI * 180.) as i32;
				let damage = stats.damage as i32;
				let level = weapon.level;
				let mut desc = vec![
					"".into(),
					format!("Level: {level}"),
					format!("Damage: {damage}"),
					format!("Reload Time: {fire_interval:.1} sec"),
					format!("Arc: {arc}°"),
					"".into(),
				];

				let base_stats = default_weapon_stats(level);

				if let Some(mod_string) = mod_string(
					"Fire Interval: ",
					base_stats.fire_interval,
					stats.fire_interval,
				)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) = mod_string("Speed: ", base_stats.speed, stats.speed)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) = mod_string("Arc: ", base_stats.arc, stats.arc)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) = mod_string("Spread: ", base_stats.spread, stats.spread)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) = mod_string("Damage: ", base_stats.damage, stats.damage)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) = mod_string(
					"Crit Chance: ",
					base_stats.critical_chance,
					stats.critical_chance,
				)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) = mod_string(
					"Crit Multiplier: ",
					base_stats.critical_multiplier,
					stats.critical_multiplier,
				)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) = mod_string(
					"Armor Damage: ",
					base_stats.armor_damage,
					stats.armor_damage,
				)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) =
					mod_string("Sail Damage: ", base_stats.sail_damage, stats.sail_damage)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) =
					mod_string("Crew Damage: ", base_stats.crew_damage, stats.crew_damage)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) =
					mod_string("Item Destroy: ", base_stats.item_chance, stats.item_chance)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) =
					mod_string("Target Hull: ", base_stats.hull_weight, stats.hull_weight)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) =
					mod_string("Target Sail: ", base_stats.sail_weight, stats.sail_weight)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) =
					mod_string("Target Crew: ", base_stats.crew_weight, stats.crew_weight)
				{
					desc.push(mod_string)
				}
				if let Some(mod_string) = mod_string(
					"Target Infirmary: ",
					base_stats.infirmary_weight,
					stats.infirmary_weight,
				)
				{
					desc.push(mod_string)
				}

				desc.join("\n")
			}
			ItemKind::Goods(level) =>
			{
				let desc = ["".into(), format!("Level: {level}")];
				desc.join("\n")
			}
			ItemKind::Cotton(level) =>
			{
				let desc = ["".into(), format!("Level: {level}")];
				desc.join("\n")
			}
			ItemKind::Tobacco(level) =>
			{
				let desc = ["".into(), format!("Level: {level}")];
				desc.join("\n")
			}
			ItemKind::Officer(officer) =>
			{
				let level = officer.level;
				let mut desc = vec!["".into(), format!("Level: {level}"), "".into()];
				let mut stats = DerivedShipStats::new();
				for prefix in &officer.prefixes
				{
					prefix.apply(&mut stats);
				}
				for suffix in &officer.suffixes
				{
					suffix.apply(&mut stats);
				}

				if stats.reload_speed != 0.0
				{
					desc.push(format!(
						"Fire rate: {:+}%",
						(stats.reload_speed * 100.) as i32
					));
				}
				if stats.speed != 0.0
				{
					desc.push(format!("Speed: {:+}%", (stats.speed * 100.) as i32));
				}
				if stats.accuracy != 0.0
				{
					desc.push(format!("Accuracy: {:+}%", (stats.accuracy * 100.) as i32));
				}
				if stats.critical_chance != 0.0
				{
					desc.push(format!(
						"Critical chance: {:+}%",
						(stats.critical_chance * 100.) as i32
					));
				}
				if stats.armor_repair != 0.0
				{
					desc.push(format!(
						"Armour repair: {:+}%",
						(stats.armor_repair * 100.) as i32
					));
				}
				if stats.hull_repair != 0.0
				{
					desc.push(format!(
						"Hull repair: {:+}%",
						(stats.hull_repair * 100.) as i32
					));
				}
				if stats.infirmary_repair != 0.0
				{
					desc.push(format!(
						"Infirmary repair: {:+}%",
						(stats.infirmary_repair * 100.) as i32
					));
				}
				if stats.sail_repair != 0.0
				{
					desc.push(format!(
						"Sail repair: {:+}%",
						(stats.sail_repair * 100.) as i32
					));
				}
				if stats.item_protect != 0.0
				{
					desc.push(format!(
						"Item protection: {:+}%",
						(stats.item_protect * 100.) as i32
					));
				}
				if stats.medic != 0.0
				{
					desc.push(format!("Healing: {:+}%", (stats.medic * 100.) as i32));
				}

				desc.join("\n")
			}
		}
	}

	pub fn draw(&self, pos: Point2<f32>, state: &game_state::GameState)
	{
		match self
		{
			ItemKind::Weapon(weapon) =>
			{
				let sprite = match weapon.rarity
				{
					Rarity::Normal => "data/cannon_normal.cfg",
					Rarity::Magic => "data/cannon_magic.cfg",
					Rarity::Rare => "data/cannon_rare.cfg",
				};
				state.get_sprite(sprite).unwrap().draw(
					pos,
					0,
					Color::from_rgb_f(1., 1., 1.),
					state,
				);
			}
			ItemKind::Goods(_) =>
			{
				state.get_sprite("data/goods.cfg").unwrap().draw(
					pos,
					0,
					Color::from_rgb_f(1., 1., 1.),
					state,
				);
			}
			ItemKind::Cotton(_) =>
			{
				state.get_sprite("data/cotton.cfg").unwrap().draw(
					pos,
					0,
					Color::from_rgb_f(1., 1., 1.),
					state,
				);
			}
			ItemKind::Tobacco(_) =>
			{
				state.get_sprite("data/tobacco.cfg").unwrap().draw(
					pos,
					0,
					Color::from_rgb_f(1., 1., 1.),
					state,
				);
			}
			ItemKind::Officer(_) =>
			{
				state.get_sprite("data/officer.cfg").unwrap().draw(
					pos,
					0,
					Color::from_rgb_f(1., 1., 1.),
					state,
				);
			}
		}
	}
}

pub fn generate_weapon(level: i32, rng: &mut impl Rng) -> Item
{
	let num_prefixes = *[0, 1, 2, 3]
		.choose_weighted(rng, |idx| [25., 10., 2., 1.][*idx])
		.unwrap();
	let num_suffixes = *[0, 1, 2, 3]
		.choose_weighted(rng, |idx| [25., 10., 2., 1.][*idx])
		.unwrap();

	let rarity = if num_prefixes == 0 && num_suffixes == 0
	{
		Rarity::Normal
	}
	else if num_prefixes <= 1 && num_suffixes <= 1
	{
		Rarity::Magic
	}
	else
	{
		Rarity::Rare
	};
	let max_tier = if level < 5
	{
		1
	}
	else if level < 10
	{
		2
	}
	else
	{
		3
	};

	let mut prefixes = vec![];
	for _ in 0..num_prefixes
	{
		let prefix_idx = rand_distr::WeightedIndex::new(WEAPON_PREFIX_WEIGHTS)
			.unwrap()
			.sample(rng);
		let tier = rng.gen_range(0..max_tier);
		let f = rng.gen_range(0.0..1.0);
		let prefix = match prefix_idx
		{
			0 => WeaponPrefix::Rapid(tier, f),
			1 => WeaponPrefix::Swivel(tier, f),
			2 => WeaponPrefix::Fast(tier, f),
			3 => WeaponPrefix::Accurate(tier, f),
			4 => WeaponPrefix::CrewSelective(tier, f),
			5 => WeaponPrefix::SailSelective(tier, f),
			6 => WeaponPrefix::InfirmarySelective(tier, f),
			7 => WeaponPrefix::HullSelective(tier, f),
			8 => WeaponPrefix::Critical(tier, f),
			_ => unreachable!(),
		};
		prefixes.push(prefix);
	}
	let mut suffixes = vec![];
	for _ in 0..num_suffixes
	{
		let suffix_idx = rand_distr::WeightedIndex::new(WEAPON_SUFFIX_WEIGHTS)
			.unwrap()
			.sample(rng);
		let tier = rng.gen_range(0..max_tier);
		let f = rng.gen_range(0.0..1.0);
		let suffix = match suffix_idx
		{
			0 => WeaponSuffix::OfDamage(tier, f),
			1 => WeaponSuffix::OfCritMulti(tier, f),
			2 => WeaponSuffix::OfCrewSlaying(tier, f),
			3 => WeaponSuffix::OfSailSlaying(tier, f),
			4 => WeaponSuffix::OfItemSlaying(tier, f),
			5 => WeaponSuffix::OfArmorSlaying(tier, f),
			_ => unreachable!(),
		};
		suffixes.push(suffix);
	}

	let name = match rarity
	{
		Rarity::Normal => "Cannon".into(),
		Rarity::Magic => format!(
			"{}{}{}",
			prefixes.first().map(|a| a.name()).unwrap_or(""),
			"Cannon",
			suffixes.first().map(|a| a.name()).unwrap_or("")
		),
		Rarity::Rare => generate_weapon_name(rng),
	};

	Item {
		kind: ItemKind::Weapon(Weapon {
			name: name,
			rarity: rarity,
			prefixes: prefixes,
			suffixes: suffixes,
			readiness: 0.,
			time_to_fire: None,
			level: level,
		}),
		price: 10,
	}
}

pub fn generate_officer(level: i32, rng: &mut impl Rng) -> Item
{
	let mut num_prefixes = *[0, 1].choose_weighted(rng, |idx| [10., 1.][*idx]).unwrap();
	let mut num_suffixes = *[0, 1].choose_weighted(rng, |idx| [10., 1.][*idx]).unwrap();
	if num_prefixes == 0 && num_suffixes == 0
	{
		if rng.gen_bool(0.5)
		{
			num_prefixes = 1;
		}
		else
		{
			num_suffixes = 1;
		}
	}

	let max_tier = if level < 5
	{
		1
	}
	else if level < 10
	{
		2
	}
	else
	{
		3
	};

	let mut prefixes = vec![];
	for _ in 0..num_prefixes
	{
		let prefix_idx = rand_distr::WeightedIndex::new(OFFICER_PREFIX_WEIGHTS)
			.unwrap()
			.sample(rng);
		let tier = rng.gen_range(0..max_tier);
		let f = rng.gen_range(0.0..1.0);
		let prefix = match prefix_idx
		{
			0 => OfficerPrefix::Rapid(tier, f),
			1 => OfficerPrefix::Speed(tier, f),
			2 => OfficerPrefix::Accurate(tier, f),
			3 => OfficerPrefix::Critical(tier, f),
			_ => unreachable!(),
		};
		prefixes.push(prefix);
	}
	let mut suffixes = vec![];
	for _ in 0..num_suffixes
	{
		let suffix_idx = rand_distr::WeightedIndex::new(OFFICER_SUFFIX_WEIGHTS)
			.unwrap()
			.sample(rng);
		let tier = rng.gen_range(0..max_tier);
		let f = rng.gen_range(0.0..1.0);
		let suffix = match suffix_idx
		{
			0 => OfficerSuffix::ArmorRepair(tier, f),
			1 => OfficerSuffix::HullRepair(tier, f),
			2 => OfficerSuffix::InfirmaryRepair(tier, f),
			3 => OfficerSuffix::SailRepair(tier, f),
			4 => OfficerSuffix::ItemProtect(tier, f),
			5 => OfficerSuffix::Medic(tier, f),
			_ => unreachable!(),
		};
		suffixes.push(suffix);
	}

	let name = format!(
		"{}{}{}",
		prefixes.first().map(|a| a.name()).unwrap_or(""),
		"Officer",
		suffixes.first().map(|a| a.name()).unwrap_or("")
	);

	Item {
		kind: ItemKind::Officer(Officer {
			name: name,
			prefixes: prefixes,
			suffixes: suffixes,
			level: level,
		}),
		price: 10,
	}
}

pub fn generate_item(level: i32, rng: &mut impl Rng) -> Item
{
	let idx = rand_distr::WeightedIndex::new([1., 1., 1., 1., 1.])
		.unwrap()
		.sample(rng);
	match idx
	{
		0 => generate_weapon(level, rng),
		1 => Item {
			kind: ItemKind::Goods(level),
			price: 10,
		},
		2 => Item {
			kind: ItemKind::Cotton(level),
			price: 10,
		},
		3 => Item {
			kind: ItemKind::Tobacco(level),
			price: 10,
		},
		4 => generate_officer(level, rng),
		_ => unreachable!(),
	}
}

#[derive(Clone, Debug)]
pub struct Item
{
	pub kind: ItemKind,
	pub price: i32,
}

impl Item
{
	pub fn reset_cooldowns(&mut self)
	{
		match &mut self.kind
		{
			ItemKind::Weapon(weapon) =>
			{
				weapon.readiness = 0.;
			}
			_ => (),
		}
	}
}

#[derive(Clone, Debug)]
pub struct ItemSlot
{
	pub item: Option<Item>,
	pub pos: Point2<f32>,
	pub dir: Option<f32>,
	pub is_inventory: bool,
	pub weapons_allowed: bool,
}

#[derive(Clone, Debug)]
pub struct DerivedShipStats
{
	pub reload_speed: f32,
	pub speed: f32,
	pub accuracy: f32,
	pub critical_chance: f32,
	pub armor_repair: f32,
	pub hull_repair: f32,
	pub infirmary_repair: f32,
	pub sail_repair: f32,
	pub item_protect: f32,
	pub medic: f32,
}

impl DerivedShipStats
{
	pub fn new() -> Self
	{
		Self {
			reload_speed: 0.,
			speed: 0.,
			accuracy: 0.,
			critical_chance: 0.,
			armor_repair: 0.,
			hull_repair: 0.,
			infirmary_repair: 0.,
			sail_repair: 0.,
			item_protect: 0.,
			medic: 0.,
		}
	}
}

#[derive(Clone, Debug)]
pub struct Equipment
{
	pub slots: Vec<ItemSlot>,
	pub want_attack: bool,
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
			let x = (i as i32 % 8) as f32 - 3.5;
			let y = (i as i32 / 8) as f32 + 4.;
			slots.push(ItemSlot {
				item: None,
				pos: Point2::new(-2. * y, -2. * x),
				dir: None,
				is_inventory: true,
				weapons_allowed: true,
			})
		}
		Self {
			slots: slots,
			want_attack: false,
			target_pos: Point3::origin(),
			allow_out_of_arc_shots: allow_out_of_arc_shots,
		}
	}

	pub fn derived_stats(&self) -> DerivedShipStats
	{
		let mut stats = DerivedShipStats::new();
		for item_slot in &self.slots
		{
			if item_slot.is_inventory
			{
				continue;
			}
			if let Some(ItemKind::Officer(officer)) = item_slot.item.as_ref().map(|a| &a.kind)
			{
				for prefix in &officer.prefixes
				{
					prefix.apply(&mut stats);
				}
				for suffix in &officer.suffixes
				{
					suffix.apply(&mut stats);
				}
			}
		}
		stats
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
	pub weapon_stats: WeaponStats,
	pub team: Team,
}

#[derive(Copy, Clone, Debug)]
pub struct DamageReport
{
	pub damaged: bool,
	pub item_destroy_chance: f32,
	pub crit: bool,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShipStats
{
	pub hull: f32,
	pub crew: i32,
	pub sails: f32,
	pub infirmary: f32,
	pub armor: [f32; 4], // front, right, back, left
	pub speed: f32,
	pub dir_speed: f32,
}

#[derive(Clone, Debug)]
pub struct ShipState
{
	pub hull: f32,
	pub crew: i32,
	pub wounded: i32,
	pub experience: f32,
	pub level: i32,
	pub team: Team,
	pub sails: f32,
	pub infirmary: f32,
	pub armor: [f32; 4], // front, right, back, left

	pub repair_boost: Vec<usize>,
	pub time_to_board: f64,
}

impl ShipState
{
	pub fn new(stats: &ShipStats, team: Team, level: i32) -> Self
	{
		Self {
			hull: stats.hull,
			crew: stats.crew,
			wounded: 0,
			team: team,
			experience: level_experience(level),
			level: level,
			sails: stats.sails,
			infirmary: stats.infirmary,
			armor: stats.armor,
			repair_boost: vec![],
			time_to_board: 0.,
		}
	}

	pub fn damage(&mut self, damage: &Damage, dir: Vector3<f32>, rng: &mut impl Rng)
		-> DamageReport
	{
		let dir = dir.zx().normalize();
		let mut crit = false;
		let mut item_destroy_chance = 0.;
		if damage.team.can_damage(&self.team)
		{
			let weapon_stats = &damage.weapon_stats;
			let mut base_damage = weapon_stats.damage;
			if rng.gen_bool(weapon_stats.critical_chance as f64)
			{
				crit = true;
				base_damage *= 1. + weapon_stats.critical_multiplier;
			}

			if rng.gen_bool(
				(weapon_stats.sail_weight / (weapon_stats.sail_weight + weapon_stats.hull_weight))
					as f64,
			)
			{
				self.sails = (self.sails - weapon_stats.sail_damage * base_damage / 2.).max(0.);
			}
			else
			{
				let armor_segment =
					((4. * (PI + dir.y.atan2(-dir.x)) / (2. * PI)) - 0.5).round() as usize;
				self.armor[armor_segment] =
					(self.armor[armor_segment] - weapon_stats.armor_damage * base_damage).max(0.);
				let bleed_through_frac =
					1. - (0.1 * self.armor[armor_segment] / base_damage).min(1.);
				item_destroy_chance = 0.01 * bleed_through_frac * weapon_stats.item_chance;
				let bleed_through = base_damage * bleed_through_frac;

				self.hull = (self.hull - bleed_through).max(0.);

				let weights = [2., weapon_stats.crew_weight, weapon_stats.infirmary_weight];
				match rand_distr::WeightedIndex::new(&weights)
					.unwrap()
					.sample(rng)
				{
					0 => (), // Missed internal systems.
					1 =>
					{
						// Hit crew.
						let crew_damage =
							(weapon_stats.crew_damage * bleed_through / 2.).ceil() as i32;
						let old_crew = self.crew;
						self.crew = (old_crew - crew_damage).max(0);
						for _ in 0..(old_crew - self.crew)
						{
							if rng.gen_bool(0.9)
							{
								self.wounded += 1;
							}
						}
					}
					2 =>
					{
						// Hit infirmary.
						self.infirmary = (self.infirmary - bleed_through).max(0.);
					}
					_ => unreachable!(),
				}
			}
			DamageReport {
				damaged: true,
				item_destroy_chance: item_destroy_chance,
				crit: crit,
			}
		}
		else
		{
			DamageReport {
				damaged: false,
				item_destroy_chance: 0.,
				crit: false,
			}
		}
	}

	pub fn compute_level(&mut self)
	{
		let mut level = 1;
		while level_experience(level + 1) <= self.experience
		{
			level += 1;
		}
		self.level = level;
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
	Pirate,
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

#[derive(Clone, Debug)]
pub struct Light
{
	pub pos: Point3<f32>,
	pub color: Color,
	pub intensity: f32,
}

#[derive(Clone, Debug)]
pub struct Lights
{
	pub lights: Vec<Light>,
}

#[derive(Clone, Debug)]
pub struct Sinking;

pub fn generate_weapon_name(rng: &mut impl Rng) -> String
{
	let prefix = [
		"Empyrean ",
		"Crazed ",
		"Oiled ",
		"Foul ",
		"Miasmatic ",
		"Colossal ",
		"Monstrous ",
		"Phallic ",
		"Behemothic ",
		"Thin ",
		"Steel ",
		"Adamnantine ",
		"Golden ",
		"Shadow ",
		"Night ",
		"Sun ",
		"Indominable ",
		"",
		"",
		"",
		"",
	]
	.choose(rng)
	.unwrap();
	let noun = [
		"Blunderbuss",
		"Ballista",
		"Mortar",
		"Firecracker",
		"Catastrophe",
		"Rome",
		"Void",
		"Hole",
		"Howitzer",
		"Ordnance",
		"Trunk",
		"Gun",
		"Trebuchet",
		"Pistol",
		"Rifle",
	]
	.choose(rng)
	.unwrap();
	let suffix = [
		" of War",
		", the King-killer",
		", Sea-Scourge",
		" of the Moon",
		" of the Sun",
		" of the Artifact",
		" of the Parliament",
		" of the King",
		" of the Queen",
		" of the Consort",
		" of the Betrayer",
		" the Betrayer",
		" of the Devil",
		" of Steel",
		"",
		"",
		"",
		"",
	]
	.choose(rng)
	.unwrap();
	format!("{prefix}{noun}{suffix}")
}

pub fn generate_captain_name(team: Team, rng: &mut impl Rng) -> String
{
	match team
	{
		Team::English => [
			"Aldington",
			"Aldridge",
			"Alford",
			"Allbrook",
			"Allday",
			"Allerton",
			"Allingham",
			"Allington",
			"Allnutt",
			"Allport",
			"Allsebrook",
			"Alston",
			"Altham",
			"Alton",
			"Anderton",
			"Ansley",
			"Anstey",
			"Appleton",
			"Appley",
			"Bassford",
			"Batley",
			"Baverstock",
			"Baxenden",
			"Bayford",
			"Beanland",
			"Beardall",
			"Beardwood",
			"Becker",
			"Beckford",
			"Beckwith",
			"Nettleton",
			"Newnham",
			"Nibley",
			"Norbury",
			"Norgrove",
			"Norland",
			"Norrington",
			"Norsworthy",
			"Northall",
			"Thelwall",
			"Thistleton",
			"Thorington",
			"Thorley",
			"Thornbury",
			"Thorndike",
			"Thornley",
			"Thornton",
			"Thorpe",
			"Threapland",
			"Throckmorton",
			"Whiteway",
			"Whitfield",
			"Whitford",
			"Whitgift",
			"Whitley",
			"Whitstone",
			"Whitter",
			"Whittingham",
			"Whittlesey",
			"Whittock",
			"Whitton",
			"Whitwell",
			"Whitworth",
		]
		.choose(rng)
		.unwrap()
		.to_string(),
		Team::French => [
			"Labille",
			"Labouchère",
			"Lacamp",
			"LaClaire",
			"Lacresse",
			"Lafaille",
			"Lafitte",
			"Lafontaine",
			"Laforest",
			"Lafourcade",
			"Lalonde",
			"Lambert",
			"Lamirault",
			"Lancret",
			"Lanvin",
			"Lataste",
			"Latendresse",
			"Lauzon",
			"Le Fur",
			"Le Goff",
			"Le Lievre",
			"Le Maçon",
			"Le Patourel",
			"Le Tissier",
			"Leale",
			"Pételle",
			"Peyron",
			"Picart",
			"Pichette",
			"Pickavance",
			"Pienaar",
			"Pierre-Paul",
			"Piffard",
			"Pineault",
			"Pinochet",
			"Pittet",
			"Plessis",
			"Poilievre",
			"Poiret",
			"Polnareff",
			"Valin",
			"Vanhoutte",
			"Vareilles",
			"Vatin",
			"Vaugrenard",
			"Vaurie",
			"Vautrin",
			"Venteau",
			"Vergès",
			"Vernet",
			"Verzelen",
			"Veuillot",
			"Veyrat",
			"Mallarmé",
			"Marais",
			"Marchive",
			"Mascaron",
			"Mathiasin",
			"Maudet",
			"Mauger",
			"Mazière",
			"Mendy",
			"Bataillon",
			"Baudrier",
			"Baudu",
			"Bazin",
			"Beaudreau",
			"Bédard",
			"Béliveau",
			"Bellecourt",
			"Belshaw",
			"Benassaya",
			"Benezet",
			"Bertrand",
			"Bethancourt",
			"Beves",
			"Fertet",
			"Feydeau",
			"Fillon",
			"Firmin",
			"Fletcher",
			"Florent",
			"Foster",
			"Fouché",
			"Fourcade",
			"Fourie",
			"Fourier",
			"Fovargue",
		]
		.choose(rng)
		.unwrap()
		.to_string(),
		Team::Pirate => [
			"Beugel",
			"Beukers",
			"Beumer",
			"Biemans",
			"Biersteker",
			"Biesheuvel",
			"Bijl",
			"Bijlsma",
			"Bikker",
			"Bisschop",
			"Blaauw",
			"Blanke",
			"Bleecker",
			"Bleekemolen",
			"Bleeker",
			"Blind",
			"Block",
			"Bloem",
			"Bloembergen",
			"Bloemen",
			"Blokland",
			"Blom",
			"Boekbinder",
			"Boeken",
			"Boekhorst",
			"Boer",
			"Boeve",
			"Borstlap",
			"Bos",
			"Bosch",
			"Boschman",
			"Bosman",
			"Bosmans",
			"Bosz",
			"Bot",
			"Bouman",
			"De Wilde",
			"De Winter",
			"De Wit",
			"De Witt",
			"De Witte",
			"De Wolf",
			"De Zeeuw",
			"De Zwart",
			"Declercq",
			"Deconinck",
			"Deelstra",
			"Dejagere",
			"Dekker",
			"Dekkers",
			"Demol",
			"Den Boer",
			"Den Dekker",
			"Den Hartog",
			"Glas",
			"Goes",
			"Goethals",
			"Goff",
			"Goll",
			"Van Gool",
			"Goos",
			"Goossen",
			"Goossens",
			"Goovaerts",
			"Goris",
			"Gorter",
			"Graaf",
			"Graafland",
			"Hendrikse",
			"Hendriksen",
			"Hendrikx",
			"Hendrix",
			"Hennies",
			"Hennis",
			"Herkenhoff",
			"Hermans",
			"Hermsen",
			"Herrema",
			"Hert",
			"Hertog",
			"Heuvelmans",
			"Heybroek",
			"Nieman",
			"Nienhuis",
			"Nienhuys",
			"Nieuwenhuis",
			"Nieuwenhuizen",
			"Nieuwenhuyzen",
			"Nieuwenkamp",
			"Nieuwland",
			"Nijboer",
			"Nijdam",
			"Nijenhuis",
			"Nijhof",
			"Nijhuis",
			"Nijland",
			"Nijman",
			"Nijpels",
			"Maertens",
			"Maes",
			"Maessen",
			"Magel",
			"Magerman",
			"Maij",
			"Majoor",
			"Makkink",
			"Mandel",
			"Manders",
			"Mangels",
			"Mansveld",
			"Scholte",
			"Scholten",
			"Schoonmaker",
			"Schout",
			"Schouten",
			"Schreuder",
			"Schreuders",
			"Schreurs",
			"Schrijver",
			"Schuller",
			"Schulting",
			"Schure",
			"Schut",
			"Schutte",
			"Schuurman",
			"Van de Kamp",
			"Van de Sande",
			"Van de Stadt",
			"Van de Velde",
			"Van de Ven",
			"Van de Vendel",
			"Van de Vijver",
			"Van de Walle",
			"Van de Water",
			"Van de Werve",
			"Van de Wetering",
			"Van de Wiele",
			"Van den Abeele",
			"Van den Akker",
			"Van den Berg",
			"Van den Bergh",
			"Van Den Berghe",
			"Van den Boogaard",
			"Van den Bos",
			"Van den Bosch",
			"Van Paassen",
			"Van Pelt",
			"Van Poortvliet",
			"Van Poppel",
			"Van Praag",
			"Van Putten",
			"Van Raalte",
			"Van Reenen",
			"Van Riel",
			"Van Rijn",
			"Van Rijswijk",
			"Van Roekel",
			"Van Rooy",
			"Van Rooyen",
			"Van Rossum",
		]
		.choose(rng)
		.unwrap()
		.to_string(),
		Team::Neutral => unreachable!(),
	}
}
