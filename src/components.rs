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
pub struct WeaponStats
{
	pub fire_interval: f32,
	pub arc: f32,
}

#[derive(Clone, Debug)]
pub struct Weapon
{
	pub readiness: f32,
	pub time_to_fire: Option<f64>,
	pub stats: WeaponStats,
}

impl Weapon
{
	pub fn new(stats: WeaponStats) -> Self
	{
		Self {
			readiness: 0.,
			time_to_fire: None,
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
	pub fn name(&self) -> String
	{
		match self
		{
			ItemKind::Weapon(_) =>
			{
				format!("Cannon")
			}
		}
	}

	pub fn description(&self) -> String
	{
		match self
		{
			ItemKind::Weapon(weapon) =>
			{
				let fire_interval = weapon.stats.fire_interval;
				let arc = (weapon.stats.arc / PI * 180.) as i32;
				[
					self.name(),
					"".into(),
					format!("Reload Time: {fire_interval:.1} sec"),
					format!("Arc: {arc}°"),
				]
				.join("\n")
			}
		}
	}

	pub fn draw(&self, pos: Point2<f32>, state: &game_state::GameState)
	{
		match self
		{
			ItemKind::Weapon(_) =>
			{
				state.get_sprite("data/cannon_rare.cfg").unwrap().draw(
					pos,
					0,
					Color::from_rgb_f(1., 1., 1.),
					state,
				);
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
			let x = (i as i32 % 8) as f32 - 3.5;
			let y = (i as i32 / 8) as f32 + 4.;
			slots.push(ItemSlot {
				item: None,
				pos: Point2::new(-2. * y, -2. * x),
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
	pub board_entity: Option<hecs::Entity>,
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
			board_entity: None,
			time_to_board: 0.,
		}
	}

	pub fn damage(&mut self, damage: &Damage, dir: Vector3<f32>, rng: &mut impl Rng)
		-> (bool, f32)
	{
		let dir = dir.zx().normalize();
		let mut bleed_through = 0.;
		if damage.team.can_damage(&self.team)
		{
			if rng.gen_bool(0.25)
			{
				self.sails = (self.sails - damage.damage / 2.).max(0.);
			}
			else
			{
				let armor_segment =
					((4. * (PI + dir.y.atan2(-dir.x)) / (2. * PI)) - 0.5).round() as usize;
				self.armor[armor_segment] = (self.armor[armor_segment] - damage.damage).max(0.);
				let bleed_through_frac =
					1. - (0.1 * self.armor[armor_segment] / damage.damage).min(1.);

				bleed_through = damage.damage * bleed_through_frac;

				self.hull = (self.hull - bleed_through).max(0.);

				let weights = [2., 1., 1.];
				match rand_distr::WeightedIndex::new(&weights)
					.unwrap()
					.sample(rng)
				{
					0 => (), // Missed internal systems.
					1 =>
					{
						// Hit crew.
						let crew_damage = (bleed_through / 2.).ceil() as i32;
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
			(true, bleed_through)
		}
		else
		{
			(false, 0.)
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
