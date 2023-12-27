use crate::error::Result;
use crate::{
	astar, components as comps, controls, game_state, mesh, spatial_grid, sprite, ui, utils,
};
use allegro::*;
use allegro_font::*;
use allegro_primitives::*;
use na::{
	Isometry3, Matrix4, Perspective3, Point2, Point3, Quaternion, RealField, Rotation2, Rotation3,
	Translation3, Unit, Vector2, Vector3, Vector4,
};
use nalgebra as na;
use rand::prelude::*;

use std::collections::HashMap;

use std::f32::consts::PI;

static CELL_SIZE: i32 = 64;
static CELL_RADIUS: i32 = 2;
const SLOT_WIDTH: f32 = 32.;

#[derive(Clone)]
pub struct Button
{
	loc: Point2<f32>,
	size: Vector2<f32>,
	text: String,
	on: bool,
	hover: bool,
}

impl Button
{
	fn new(loc: Point2<f32>, size: Vector2<f32>, text: String) -> Self
	{
		Self {
			loc: loc,
			size: size,
			text: text,
			on: false,
			hover: false,
		}
	}

	fn logic(&mut self) -> bool
	{
		let old_on = self.on;
		self.on = false;
		old_on
	}

	fn draw(&self, state: &game_state::GameState)
	{
		let color = if self.hover
		{
			Color::from_rgba_f(1.0, 1.0, 1.0, 1.0)
		}
		else
		{
			Color::from_rgba_f(0.5, 0.5, 0.5, 1.0)
		};
		state.core.draw_text(
			&state.ui_font,
			color,
			self.loc.x,
			self.loc.y,
			FontAlign::Centre,
			&self.text,
		);
	}

	fn input(&mut self, event: &Event)
	{
		let start = self.loc - self.size / 2.;
		let end = self.loc + self.size / 2.;
		match event
		{
			Event::MouseButtonDown {
				button: 1, x, y, ..
			} =>
			{
				let (x, y) = (*x as f32, *y as f32);
				if x > start.x && x < end.x && y < end.y && y > start.y
				{
					self.on = true;
				}
			}
			Event::MouseAxes { x, y, .. } =>
			{
				let (x, y) = (*x as f32, *y as f32);
				self.hover = x > start.x && x < end.x && y < end.y && y > start.y;
			}
			_ => (),
		}
	}
}

#[derive(Clone)]
pub struct Cell
{
	center: Point2<i32>,
}

impl Cell
{
	fn new<R: Rng>(
		center: Point2<i32>, rng: &mut R, world: &mut hecs::World,
		state: &mut game_state::GameState,
	) -> Result<Self>
	{
		let world_center = Point3::new(
			(center.x * CELL_SIZE) as f32,
			0.,
			(center.y * CELL_SIZE) as f32,
		);

		dbg!(world_center);

		let ship = make_ship(
			world_center,
			*[comps::Team::English, comps::Team::French]
				.choose(rng)
				.unwrap(),
			world,
			state,
		)?;

		world.insert_one(
			ship,
			comps::AI {
				state: comps::AIState::Idle,
			},
		)?;
		world.get::<&mut comps::ShipState>(ship).unwrap().crew = 0;

		Ok(Self { center: center })
	}

	pub fn world_center(&self) -> Point3<f32>
	{
		Point3::new(
			(self.center.x * CELL_SIZE) as f32,
			0.,
			(self.center.y * CELL_SIZE) as f32,
		)
	}

	pub fn world_to_cell(pos: &Point3<f32>) -> Point2<i32>
	{
		let sz = CELL_SIZE as f32;
		let x = pos.x + sz / 2.;
		let y = pos.z + sz / 2.;
		Point2::new((x / sz).floor() as i32, (y / sz).floor() as i32)
	}

	pub fn contains(&self, pos: &Point3<f32>) -> bool
	{
		self.center == Cell::world_to_cell(pos)
	}
}

pub struct Game
{
	map: Map,
	equipment_screen: Option<EquipmentScreen>,
	subscreens: Vec<ui::SubScreen>,
}

impl Game
{
	pub fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		Ok(Self {
			map: Map::new(state)?,
			subscreens: vec![],
			equipment_screen: None,
		})
	}

	pub fn logic(
		&mut self, state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		if self.subscreens.is_empty()
		{
			let want_inventory = state.controls.get_action_state(controls::Action::Inventory) > 0.5;
			state
				.controls
				.clear_action_state(controls::Action::Inventory);

			if want_inventory
			{
				if self.equipment_screen.is_some()
				{
					self.equipment_screen
						.as_mut()
						.unwrap()
						.finish_trade(&mut self.map);
					self.equipment_screen = None;
					self.map.dock_entity = None;
				}
				else
				{
					self.equipment_screen = Some(EquipmentScreen::new(state));
				}
			}
			if self.map.dock_entity.is_some() && self.equipment_screen.is_none()
			{
				self.equipment_screen = Some(EquipmentScreen::new(state));
			}

			if let Some(equipment_screen) = self.equipment_screen.as_mut()
			{
				self.map.mouse_in_buffer = equipment_screen.logic(&mut self.map, state);
			}
			else
			{
				self.map.mouse_in_buffer = true;
			}
			self.map.logic(state)
		}
		else
		{
			Ok(None)
		}
	}

	pub fn input(
		&mut self, event: &Event, state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		let handled;
		if let Some(equipment_screen) = self.equipment_screen.as_mut()
		{
			handled = equipment_screen.input(event, &mut self.map, state);
		}
		else
		{
			handled = false;
		}
		if !handled
		{
			state.controls.decode_event(event);
		}
		match *event
		{
			Event::MouseAxes { x, y, .. } =>
			{
				if state.track_mouse
				{
					state.mouse_pos = Point2::new(x as i32, y as i32);
				}
			}
			_ => (),
		}
		if self.subscreens.is_empty()
		{
			let mut in_game_menu = false;
			match *event
			{
				Event::KeyDown { keycode, .. } => match keycode
				{
					KeyCode::Escape =>
					{
						if self.equipment_screen.is_some()
						{
							self.equipment_screen
								.as_mut()
								.unwrap()
								.finish_trade(&mut self.map);
							self.equipment_screen = None;
							self.map.dock_entity = None;
						}
						else
						{
							in_game_menu = true;
						}
					}
					_ => (),
				},
				_ =>
				{
					let res = self.map.input(event, state);
					if let Ok(Some(game_state::NextScreen::InGameMenu)) = res
					{
						in_game_menu = true;
					}
					else
					{
						return res;
					}
				}
			}
			if in_game_menu
			{
				self.subscreens
					.push(ui::SubScreen::InGameMenu(ui::InGameMenu::new(
						state.display_width,
						state.display_height,
					)));
				state.paused = true;
			}
		}
		else
		{
			if let Some(action) = self
				.subscreens
				.last_mut()
				.and_then(|s| s.input(state, event))
			{
				match action
				{
					ui::Action::Forward(subscreen_fn) =>
					{
						self.subscreens.push(subscreen_fn(
							state,
							state.display_width,
							state.display_height,
						));
					}
					ui::Action::MainMenu => return Ok(Some(game_state::NextScreen::Menu)),
					ui::Action::Back =>
					{
						self.subscreens.pop().unwrap();
					}
					_ => (),
				}
			}
			if self.subscreens.is_empty()
			{
				state.paused = false;
			}
		}
		Ok(None)
	}

	pub fn draw(&mut self, state: &game_state::GameState) -> Result<()>
	{
		state.core.clear_to_color(Color::from_rgb_f(0.5, 0.5, 1.));
		self.map.draw(state, self.subscreens.is_empty())?;
		if let Some(equipment_screen) = self.equipment_screen.as_ref()
		{
			equipment_screen.draw(&self.map, state);
		}
		if let Some(subscreen) = self.subscreens.last_mut()
		{
			state.prim.draw_filled_rectangle(
				0.,
				0.,
				state.display_width,
				state.display_height,
				Color::from_rgba_f(0., 0., 0., 0.5),
			);
			subscreen.draw(state);

			// // This is dumb.
			// let sprite = state.get_sprite("data/cursor.cfg").unwrap();
			// sprite.draw(
			// 	to_f32(state.mouse_pos),
			// 	0,
			// 	Color::from_rgb_f(1., 1., 1.),
			// 	state,
			// );
		}
		Ok(())
	}
}

struct EquipmentScreen
{
	buffer_width: f32,
	buffer_height: f32,
	mouse_button_down: bool,
	ctrl_down: bool,

	// Source slot, equipment_idx
	hover_slot: Option<(usize, i32)>,
	// Source slot, equipment_idx, item
	dragged_item: Option<(usize, i32, comps::Item)>,

	switch_ships: Option<Button>,
}

impl EquipmentScreen
{
	fn new(state: &mut game_state::GameState) -> Self
	{
		Self {
			buffer_width: state.display_width,
			buffer_height: state.display_height,
			hover_slot: None,
			dragged_item: None,
			mouse_button_down: false,
			ctrl_down: false,
			switch_ships: None,
		}
	}

	fn get_slot_pos(&self, equipment_idx: i32, real_pos: Point2<f32>) -> Point2<f32>
	{
		let (bw, bh) = (self.buffer_width, self.buffer_height);
		Point2::new(-real_pos.y, -real_pos.x) * 64.
			+ Vector2::new(bw / 6. + bw * 2. / 3. * equipment_idx as f32, bh / 4.)
	}

	fn over_ui(&self, map: &mut Map, state: &game_state::GameState) -> bool
	{
		let mouse_pos = Point2::new(state.mouse_pos.x as f32, state.mouse_pos.y as f32);
		let in_right =
			mouse_pos.x > self.buffer_width * 2. / 3. && mouse_pos.y < self.buffer_height / 2.;
		let in_left =
			mouse_pos.x < self.buffer_width * 1. / 3. && mouse_pos.y < self.buffer_height / 2.;
		if map.dock_entity.is_some()
		{
			in_left || in_right
		}
		else
		{
			in_right
		}
	}

	fn input(&mut self, event: &Event, map: &mut Map, state: &mut game_state::GameState) -> bool
	{
		if let Some(button) = self.switch_ships.as_mut()
		{
			button.input(event);
		}
		match *event
		{
			Event::MouseButtonDown { button: 1, .. } =>
			{
				if self.over_ui(map, state)
				{
					self.mouse_button_down = true;
					return true;
				}
			}
			Event::MouseButtonUp { button: 1, .. } =>
			{
				self.mouse_button_down = false;
			}
			Event::KeyDown {
				keycode: KeyCode::LCtrl | KeyCode::RCtrl,
				..
			} =>
			{
				if self.over_ui(map, state)
				{
					self.ctrl_down = true;
					return true;
				}
			}
			Event::KeyUp {
				keycode: KeyCode::LCtrl | KeyCode::RCtrl,
				..
			} =>
			{
				self.ctrl_down = false;
			}
			_ => (),
		}
		false
	}

	fn do_trade(&self, map: &Map) -> bool
	{
		let dock_team = map.dock_entity.and_then(|dock_entity| {
			map.world
				.get::<&comps::ShipState>(dock_entity)
				.map(|ss| ss.team)
				.ok()
		});
		let player_team = map
			.world
			.get::<&comps::ShipState>(map.player)
			.map(|ss| ss.team)
			.ok();

		if let (Some(dock_team), Some(player_team)) = (dock_team, player_team)
		{
			dock_team.trade_with(&player_team)
		}
		else
		{
			false
		}
	}

	fn logic(&mut self, map: &mut Map, state: &game_state::GameState) -> bool
	{
		if map.dock_entity.is_some() && self.switch_ships.is_none()
		{
			self.switch_ships = Some(Button::new(
				Point2::new(state.display_width / 3. - 64., 64.),
				Vector2::new(64., 64.),
				"Switch".into(),
			));
		}
		else if map.dock_entity.is_none()
		{
			self.switch_ships = None;
		}
		let do_switch = if let Some(button) = self.switch_ships.as_mut()
		{
			button.logic()
		}
		else
		{
			false
		};
		let do_trade = self.do_trade(map);
		let mouse_pos = Point2::new(state.mouse_pos.x as f32, state.mouse_pos.y as f32);
		self.hover_slot = None;
		let mut old_item = None;
		let over_ui = self.over_ui(map, state);

		let mut query = map.world.query::<&mut comps::Equipment>();
		let mut view = query.view();
		let [mut dock_equipment, player_equipment] = if let Some(dock_entity) = map.dock_entity
		{
			view.get_mut_n([dock_entity, map.player])
		}
		else
		{
			[None, view.get_mut(map.player)]
		};

		{
			let dock_slots = dock_equipment.iter_mut().flat_map(|eq| eq.slots.iter_mut());
			let mut fast_move = false;
			if let Some(equipment) = player_equipment
			{
				for (i, (equipment_idx, slot)) in
					(equipment.slots.iter_mut().map(|slot| (1, slot)).enumerate())
						.chain(dock_slots.map(|slot| (0, slot)).enumerate())
				{
					if do_trade && equipment_idx == 0 && !slot.is_inventory
					{
						continue;
					}

					let pos = self.get_slot_pos(equipment_idx, slot.pos);
					let w = SLOT_WIDTH;
					if mouse_pos.x > pos.x - w / 2.
						&& mouse_pos.x < pos.x + w / 2.
						&& mouse_pos.y > pos.y - w / 2.
						&& mouse_pos.y < pos.y + w / 2.
					{
						if self.mouse_button_down && self.dragged_item.is_none()
						{
							// Grab item.
							// Don't grab if grabbing from trade partner and not enough money.
							let mut start_grab = true;
							if equipment_idx == 0 && do_trade
							{
								if let Some(item) = slot.item.as_ref()
								{
									if item.price > map.money
									{
										start_grab = false;
									}
									map.money -= item.price;
								}
							}
							if start_grab
							{
								self.dragged_item =
									slot.item.take().map(|item| (i, equipment_idx, item));
								if self.ctrl_down
								{
									fast_move = true;
								}
							}
						}
						else if !self.mouse_button_down && self.dragged_item.is_some()
						{
							// Drop item.
							// If dropping into trade partner's window, grab the money.
							let (source_i, source_equipment_idx, item) =
								self.dragged_item.take().unwrap();
							if equipment_idx == 0 && do_trade
							{
								map.money += item.price;
							}
							old_item = slot
								.item
								.take()
								.map(|item| (source_i, source_equipment_idx, item));
							slot.item = Some(item);
						}
						if !self.mouse_button_down
						{
							self.hover_slot = Some((i, equipment_idx));
						}
					}
				}
				if !self.mouse_button_down && self.dragged_item.is_some()
				{
					old_item = self.dragged_item.take();
				}
				if let Some((i, equipment_idx, item)) = old_item
				{
					if equipment_idx == 1
					{
						equipment.slots[i].item = Some(item);
					}
					else if let Some(dock_equipment) = dock_equipment.as_mut()
					{
						dock_equipment.slots[i].item = Some(item);
					}
				}
				if fast_move
				{
					let mut moved = false;
					if let Some((_, equipment_idx, item)) = self.dragged_item.as_ref()
					{
						if *equipment_idx == 1
						{
							// This is in lieu of the logic for dropping.
							if do_trade
							{
								map.money += item.price;
							}
							if let Some(dock_equipment) = dock_equipment.as_mut()
							{
								for slot in &mut dock_equipment.slots
								{
									if slot.is_inventory && slot.item.is_none()
									{
										slot.item = Some(item.clone());
										moved = true;
										break;
									}
								}
							}
						}
						else
						{
							// We took care of the price when we grabbed it earlier.
							for slot in &mut equipment.slots
							{
								if slot.is_inventory && slot.item.is_none()
								{
									slot.item = Some(item.clone());
									moved = true;
									break;
								}
							}
						}
					}
					if moved
					{
						self.dragged_item = None;
					}
				}
			}
		}
		if do_switch
		{
			let mut query = map.world.query::<&mut comps::ShipState>();
			let mut view = query.view();
			let [mut dock_state, mut player_state] =
				view.get_mut_n([map.dock_entity.unwrap(), map.player]);
			if let (Some(dock_state), Some(player_state)) =
				(dock_state.as_mut(), player_state.as_mut())
			{
				let player_crew = player_state.crew;
				player_state.crew = dock_state.crew;
				dock_state.crew = player_crew;

				let player_team = player_state.team;
				player_state.team = dock_state.team;
				dock_state.team = player_team;
			}

			let player = map.player;
			map.player = map.dock_entity.unwrap();
			map.dock_entity = Some(player);
		}
		!over_ui
	}

	fn finish_trade(&mut self, map: &mut Map)
	{
		let do_trade = self.do_trade(map);

		let mut query = map.world.query::<&mut comps::Equipment>();
		let mut view = query.view();
		let [dock_equipment, player_equipment] = if let Some(dock_entity) = map.dock_entity
		{
			view.get_mut_n([dock_entity, map.player])
		}
		else
		{
			[None, view.get_mut(map.player)]
		};

		if let Some(equipment) = player_equipment
		{
			if let Some((i, equipment_idx, item)) = self.dragged_item.take()
			{
				if equipment_idx == 1
				{
					equipment.slots[i].item = Some(item);
				}
				else if let Some(dock_equipment) = dock_equipment
				{
					// When returning item to the trade partner, refund the price.
					if do_trade
					{
						map.money += item.price;
					}
					dock_equipment.slots[i].item = Some(item);
				}
			}
		}
	}

	fn draw(&self, map: &Map, state: &game_state::GameState)
	{
		if map.dock_entity.is_some()
		{
			state.prim.draw_filled_rectangle(
				0.,
				0.,
				self.buffer_width * 1. / 3.,
				self.buffer_height / 2.,
				Color::from_rgb_f(0.1, 0.1, 0.2),
			);
		}
		state.prim.draw_filled_rectangle(
			self.buffer_width * 2. / 3.,
			0.,
			self.buffer_width,
			self.buffer_height / 2.,
			Color::from_rgb_f(0.1, 0.1, 0.2),
		);
		let do_trade = self.do_trade(map);
		let mouse_pos = Point2::new(state.mouse_pos.x as f32, state.mouse_pos.y as f32);

		let mut query = map.world.query::<&comps::Equipment>();
		let view = query.view();
		let [dock_equipment, player_equipment] = if let Some(dock_entity) = map.dock_entity
		{
			[view.get(dock_entity), view.get(map.player)]
		}
		else
		{
			[None, view.get(map.player)]
		};
		let dock_slots = dock_equipment.iter().flat_map(|eq| eq.slots.iter());
		if let Some(equipment) = player_equipment
		{
			let mut hover_item = None;
			for (i, (equipment_idx, slot)) in
				(equipment.slots.iter().map(|slot| (1, slot)).enumerate())
					.chain(dock_slots.map(|slot| (0, slot)).enumerate())
			{
				if do_trade && equipment_idx == 0 && !slot.is_inventory
				{
					continue;
				}
				let pos = self.get_slot_pos(equipment_idx, slot.pos);
				if let Some(item) = &slot.item
				{
					if Some((i, equipment_idx)) == self.hover_slot
					{
						hover_item = Some((pos, item.clone()));
					}
				}
				let w = SLOT_WIDTH;
				state.prim.draw_rounded_rectangle(
					pos.x - w / 2.,
					pos.y - w / 2.,
					pos.x + w / 2.,
					pos.y + w / 2.,
					8.,
					8.,
					Color::from_rgb_f(1., 1., 1.),
					3.,
				);

				if let Some(item) = slot.item.as_ref()
				{
					draw_item(pos.x, pos.y, item, state);
				}
				if let Some(slot_dir) = slot.dir
				{
					let arc = PI / 4.;
					state.prim.draw_arc(
						pos.x,
						pos.y,
						w,
						-slot_dir - arc / 2. + PI * 3. / 2.,
						arc,
						Color::from_rgba_f(1., 1., 1., 1.),
						4.,
					);
				}
			}

			if let Some((pos, item)) = hover_item
			{
				state.prim.draw_filled_rectangle(
					pos.x,
					pos.y,
					pos.x + 256.,
					pos.y + 128.,
					Color::from_rgba_f(0., 0., 0., 0.75),
				);

				let x = pos.x + 16.;
				let mut y = pos.y + 16.;

				let price_desc = if do_trade
				{
					let price = 10;
					vec!["".into(), format!("Price: {price}")]
				}
				else
				{
					vec![]
				};

				for line in price_desc
					.iter()
					.map(|s| s.as_str())
					.chain(item.kind.description().lines())
				{
					state.core.draw_text(
						&state.ui_font,
						Color::from_rgb_f(1., 1., 1.),
						x,
						y,
						FontAlign::Left,
						line,
					);
					y += 16.;
				}
			}

			if let Some((_, _, ref item)) = self.dragged_item
			{
				draw_item(mouse_pos.x, mouse_pos.y, item, state);
			}
		}

		if let Some(button) = self.switch_ships.as_ref()
		{
			button.draw(state);
		}
	}
}

fn draw_item(x: f32, y: f32, _item: &comps::Item, state: &game_state::GameState)
{
	state
		.prim
		.draw_filled_circle(x, y, 8., Color::from_rgba_f(1., 0., 0., 1.));
}

fn draw_ship_state(ship_state: &comps::ShipState, x: f32, y: f32, state: &game_state::GameState)
{
	state.core.draw_text(
		&state.ui_font,
		Color::from_rgb_f(1., 1., 1.),
		x,
		y,
		FontAlign::Left,
		&format!("Hull: {}", ship_state.hull as i32),
	);
	state.core.draw_text(
		&state.ui_font,
		Color::from_rgb_f(1., 1., 1.),
		x,
		y + 16.,
		FontAlign::Left,
		&format!("Crew: {}", ship_state.crew),
	);
}

fn make_target(
	pos: Point3<f32>, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let mesh = "data/target.glb";
	game_state::cache_mesh(state, mesh)?;
	let res = world.spawn((
		comps::Position { pos: pos, dir: 0. },
		comps::Mesh { mesh: mesh.into() },
	));
	Ok(res)
}

fn make_projectile(
	pos: Point3<f32>, dir: Vector3<f32>, parent: hecs::Entity, team: comps::Team,
	world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let mesh = "data/sphere.glb";
	game_state::cache_mesh(state, mesh)?;
	let res = world.spawn((
		comps::Position { pos: pos, dir: 0. },
		comps::Velocity {
			vel: 50. * dir,
			dir_vel: 0.,
		},
		comps::Solid {
			size: 0.5,
			mass: 0.,
			kind: comps::CollideKind::Small,
			parent: Some(parent),
		},
		comps::Mesh { mesh: mesh.into() },
		comps::TimeToDie {
			time_to_die: state.time() + 1.,
		},
		comps::AffectedByGravity,
		comps::CollidesWithWater,
		comps::OnContactEffect {
			effects: vec![
				comps::ContactEffect::Die,
				comps::ContactEffect::Hurt {
					damage: comps::Damage {
						damage: 10.,
						team: team,
					},
				},
			],
		},
	));
	Ok(res)
}

fn make_ship(
	pos: Point3<f32>, team: comps::Team, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let mesh = "data/small_ship.glb";
	game_state::cache_mesh(state, mesh)?;
	let equipment = comps::Equipment::new(
		8,
		true,
		vec![
			comps::ItemSlot {
				pos: Point2::new(0.5, 1.0),
				dir: Some(PI / 2.0),

				item: Some(comps::Item {
					kind: comps::ItemKind::Weapon(comps::Weapon::new(comps::WeaponStats {
						fire_interval: 1.,
						arc: PI / 2.0,
					})),
					price: 10,
				}),
				is_inventory: false,
			},
			comps::ItemSlot {
				pos: Point2::new(-0.5, 1.0),
				dir: Some(PI / 2.0),

				item: Some(comps::Item {
					kind: comps::ItemKind::Weapon(comps::Weapon::new(comps::WeaponStats {
						fire_interval: 1.,
						arc: PI / 2.0,
					})),
					price: 10,
				}),
				is_inventory: false,
			},
			comps::ItemSlot {
				pos: Point2::new(0.0, -1.0),
				dir: Some(-PI / 2.0),
				item: Some(comps::Item {
					kind: comps::ItemKind::Weapon(comps::Weapon::new(comps::WeaponStats {
						fire_interval: 1.,
						arc: PI / 2.0,
					})),
					price: 10,
				}),
				is_inventory: false,
			},
			comps::ItemSlot {
				pos: Point2::new(1.0, 0.0),
				dir: Some(0.),

				item: Some(comps::Item {
					kind: comps::ItemKind::Weapon(comps::Weapon::new(comps::WeaponStats {
						fire_interval: 1.,
						arc: PI / 4.0,
					})),
					price: 10,
				}),
				is_inventory: false,
			},
		],
	);
	let res = world.spawn((
		comps::Position { pos: pos, dir: 0. },
		comps::Velocity {
			vel: Vector3::zeros(),
			dir_vel: 0.,
		},
		comps::Mesh { mesh: mesh.into() },
		comps::Target { waypoints: vec![] },
		comps::Stats { speed: 10. },
		comps::Solid {
			size: 2.,
			mass: 1.,
			kind: comps::CollideKind::Big,
			parent: None,
		},
		equipment,
		comps::ShipState {
			hull: 100.,
			crew: 9,
			team: team,
		},
		comps::Tilt {
			tilt: 0.,
			target_tilt: 0.,
		},
	));
	Ok(res)
}

#[derive(Copy, Clone, Debug)]
struct CollisionEntry
{
	entity: hecs::Entity,
	pos: Point3<f32>,
}

struct Map
{
	world: hecs::World,
	rng: StdRng,
	player: hecs::Entity,
	player_pos: Point3<f32>,
	zoom: f32,
	mouse_entity: Option<hecs::Entity>,
	dock_entity: Option<hecs::Entity>,
	buffer_width: f32,
	buffer_height: f32,
	mouse_in_buffer: bool,
	cells: Vec<Cell>,
	money: i32,
}

impl Map
{
	fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		let mut rng = StdRng::seed_from_u64(thread_rng().gen::<u16>() as u64);
		let mut world = hecs::World::new();

		let player = make_ship(
			Point3::new(30., 0., 0.),
			comps::Team::English,
			&mut world,
			state,
		)?;

		let mut cells = vec![];
		for y in -CELL_RADIUS..=CELL_RADIUS
		{
			for x in -CELL_RADIUS..=CELL_RADIUS
			{
				cells.push(Cell::new(Point2::new(x, y), &mut rng, &mut world, state)?);
			}
		}

		state.cache_bitmap("data/english_flag.png")?;
		state.cache_bitmap("data/french_flag.png")?;

		Ok(Self {
			world: world,
			rng: rng,
			player_pos: Point3::new(0., 0., 0.),
			player: player,
			mouse_entity: None,
			buffer_width: state.display_width,
			buffer_height: state.display_height,
			mouse_in_buffer: true,
			dock_entity: None,
			cells: cells,
			zoom: 1.,
			money: 100,
		})
	}

	fn make_project(&self) -> Perspective3<f32>
	{
		utils::projection_transform(self.buffer_width, self.buffer_height, PI / 2.)
	}

	fn make_camera(&self) -> Isometry3<f32>
	{
		let height = 30. / self.zoom;
		utils::make_camera(
			self.player_pos + Vector3::new(0., height, height / 2.),
			self.player_pos,
		)
	}

	fn get_mouse_ground_pos(&self, state: &game_state::GameState) -> Point3<f32>
	{
		let (x, y) = (state.mouse_pos.x, state.mouse_pos.y);
		let fx = -1. + 2. * x as f32 / self.buffer_width;
		let fy = -1. + 2. * y as f32 / self.buffer_height;
		let camera = self.make_camera();
		utils::get_ground_from_screen(fx, -fy, self.make_project(), camera)
	}

	fn logic(&mut self, state: &mut game_state::GameState)
		-> Result<Option<game_state::NextScreen>>
	{
		let mut to_die = vec![];
		let dt = utils::DT as f32;

		// Cell changes
		let mut new_cell_centers = vec![];
		let player_cell = Cell::world_to_cell(&self.player_pos);
		let mut good_cells = vec![];

		for cell in &self.cells
		{
			let disp = cell.center - player_cell;
			if disp.x.abs() > CELL_RADIUS || disp.y.abs() > CELL_RADIUS
			{
				for (id, position) in self.world.query::<&comps::Position>().iter()
				{
					if cell.contains(&position.pos)
					{
						to_die.push(id);
						println!("Killed {:?} {}", id, cell.center);
					}
				}
			}
			else
			{
				good_cells.push(cell.clone())
			}
		}

		self.cells.clear();

		for dy in -CELL_RADIUS..=CELL_RADIUS
		{
			for dx in -CELL_RADIUS..=CELL_RADIUS
			{
				let cell_center = player_cell + Vector2::new(dx, dy);

				let mut found = false;
				for cell in &good_cells
				{
					if cell.center == cell_center
					{
						self.cells.push(cell.clone());
						found = true;
						break;
					}
				}

				if !found
				{
					new_cell_centers.push(cell_center);
					println!("New cell {}", cell_center);
				}
			}
		}

		new_cell_centers.shuffle(&mut self.rng);

		for cell_center in new_cell_centers
		{
			self.cells.push(Cell::new(
				cell_center,
				&mut self.rng,
				&mut self.world,
				state,
			)?);
		}

		// Collision.
		let center = self.player_pos.zx();
		let mut grid = spatial_grid::SpatialGrid::new(64, 64, 16.0, 16.0);
		for (id, (position, solid)) in self
			.world
			.query::<(&comps::Position, &comps::Solid)>()
			.iter()
		{
			let pos = Point2::from(position.pos.zx() - center);
			let disp = Vector2::new(solid.size, solid.size);
			grid.push(spatial_grid::entry(
				pos - disp,
				pos + disp,
				CollisionEntry {
					entity: id,
					pos: position.pos,
				},
			));
		}

		// Physics
		for (_, (_, vel)) in self
			.world
			.query::<(&comps::AffectedByGravity, &mut comps::Velocity)>()
			.iter()
		{
			vel.vel.y -= dt * 100.0;
		}

		for (_, (pos, vel)) in self
			.world
			.query::<(&mut comps::Position, &comps::Velocity)>()
			.iter()
		{
			pos.pos += dt * vel.vel;
			pos.dir += dt * vel.dir_vel;
		}

		// Collides with water.
		for (id, (_, pos)) in self
			.world
			.query::<(&comps::CollidesWithWater, &mut comps::Position)>()
			.iter()
		{
			if pos.pos.y < -0.0
			{
				to_die.push(id);
			}
		}

		// Tilt.
		for (_, (tilt, ship_state)) in self
			.world
			.query::<(&mut comps::Tilt, &comps::ShipState)>()
			.iter()
		{
			tilt.target_tilt = state.time().sin() as f32 * PI / 4.;
			if !ship_state.is_structurally_sound()
			{
				tilt.target_tilt -= PI / 2.;
			}
			tilt.tilt += 0.1 * dt * (tilt.target_tilt - tilt.tilt);
		}

		// Collision resolution.
		let mut colliding_pairs = vec![];
		for (a, b) in grid.all_pairs(|a, b| {
			let a = a.inner.entity;
			let b = b.inner.entity;
			let a_solid = self.world.get::<&comps::Solid>(a).unwrap();
			let b_solid = self.world.get::<&comps::Solid>(b).unwrap();
			a_solid.kind.collides_with(&b_solid.kind)
				&& a_solid.parent != Some(b)
				&& b_solid.parent != Some(a)
		})
		{
			colliding_pairs.push((a.inner, b.inner));
		}

		let mut on_contact_effects = vec![];
		for pass in 0..5
		{
			for &(inner1, inner2) in &colliding_pairs
			{
				let id1 = inner1.entity;
				let id2 = inner2.entity;
				let pos1 = self.world.get::<&comps::Position>(id1)?.pos;
				let pos2 = self.world.get::<&comps::Position>(id2)?.pos;

				let solid1 = *self.world.get::<&comps::Solid>(id1)?;
				let solid2 = *self.world.get::<&comps::Solid>(id2)?;

				let diff = pos2.zx() - pos1.zx();
				let diff_norm = utils::max(0.1, diff.norm());

				if diff_norm > solid1.size + solid2.size
				{
					continue;
				}

				//if solid1.collision_class.interacts() && solid2.collision_class.interacts()
				if true
				{
					let diff = 0.9 * diff * (solid1.size + solid2.size - diff_norm) / diff_norm;
					let diff = Vector3::new(diff.y, 0., diff.x);

					let f1 = 1. - solid1.mass / (solid2.mass + solid1.mass);
					let f2 = 1. - solid2.mass / (solid2.mass + solid1.mass);
					if f32::is_finite(f1)
					{
						self.world.get::<&mut comps::Position>(id1)?.pos -= diff * f1;
					}
					if f32::is_finite(f2)
					{
						self.world.get::<&mut comps::Position>(id2)?.pos += diff * f2;
					}
				}

				if pass == 0
				{
					for (id, other_id) in [(id1, Some(id2)), (id2, Some(id1))]
					{
						if let Ok(on_contact_effect) = self.world.get::<&comps::OnContactEffect>(id)
						{
							on_contact_effects.push((
								id,
								other_id,
								on_contact_effect.effects.clone(),
							));
						}
					}
				}
			}
		}

		// On contact effects.
		for (id, other_id, effects) in on_contact_effects
		{
			for effect in effects
			{
				match (effect, other_id)
				{
					(comps::ContactEffect::Die, _) => to_die.push(id),
					(comps::ContactEffect::Hurt { damage }, Some(other_id)) =>
					{
						let mut damaged = false;
						if let Ok(mut ship_state) =
							self.world.get::<&mut comps::ShipState>(other_id)
						{
							damaged = ship_state.damage(&damage);
						}
						if damaged
						{
							if let Ok(mut ai) = self.world.get::<&mut comps::AI>(other_id)
							{
								ai.state = comps::AIState::Pursuing(id);
							}
						}
					}
					_ => (),
				}
			}
		}

		// Mouse hover.
		let mouse_in_buffer = self.mouse_in_buffer;
		let mouse_ground_pos = self.get_mouse_ground_pos(state);
		if mouse_in_buffer
		{
			let mouse_entries = grid.query_rect(
				mouse_ground_pos.zx() - Vector2::new(0.1, 0.1) - center.coords,
				mouse_ground_pos.zx() + Vector2::new(0.1, 0.1) - center.coords,
				|_| true,
			);

			if let Some(entry) = mouse_entries.first()
			{
				if let (Ok(pos), Ok(solid), Ok(_)) = (
					self.world.get::<&comps::Position>(entry.inner.entity),
					self.world.get::<&comps::Solid>(entry.inner.entity),
					self.world.get::<&comps::ShipState>(entry.inner.entity),
				)
				{
					if (pos.pos - mouse_ground_pos).magnitude() < solid.size
					{
						self.mouse_entity = Some(entry.inner.entity);
					}
				}
			}
		}

		// Player Input
		let player_alive = self
			.world
			.get::<&comps::ShipState>(self.player)
			.map(|s| s.is_active())
			.unwrap_or(false);
		let want_move = state.controls.get_action_state(controls::Action::Move) > 0.5;
		let want_dock = state.controls.get_action_state(controls::Action::Dock) > 0.5;
		let want_stop = state.controls.get_action_state(controls::Action::Stop) > 0.5;
		let want_queue = state.controls.get_action_state(controls::Action::Queue) > 0.5;
		let want_action_1 = state.controls.get_action_state(controls::Action::Action1) > 0.5;
		let want_zoom_in = state.controls.get_action_state(controls::Action::ZoomIn) > 0.5;
		let want_zoom_out = state.controls.get_action_state(controls::Action::ZoomOut) > 0.5;
		if want_move && mouse_in_buffer && player_alive
		{
			self.dock_entity = None;
			state.controls.clear_action_state(controls::Action::Move);
			let marker = make_target(mouse_ground_pos, &mut self.world, state)?;
			let despawn;
			if let Ok(mut target) = self.world.get::<&mut comps::Target>(self.player)
			{
				if !want_queue
				{
					target.clear(|m| to_die.push(m));
				}
				target.waypoints.push(comps::Waypoint {
					pos: mouse_ground_pos,
					marker: Some(marker),
				});
				despawn = false;
			}
			else
			{
				despawn = true;
			}
			if despawn
			{
				self.world.despawn(marker)?;
			}
		}
		if want_stop && player_alive
		{
			if let Ok(mut target) = self.world.get::<&mut comps::Target>(self.player)
			{
				target.clear(|m| to_die.push(m));
			}
		}
		if want_action_1 && mouse_in_buffer && player_alive
		{
			if let Ok(mut equipment) = self.world.get::<&mut comps::Equipment>(self.player)
			{
				equipment.want_action_1 = true;
				equipment.target_pos = mouse_ground_pos;
			}
		}
		if want_dock && player_alive && self.mouse_entity != Some(self.player)
		{
			state.controls.clear_action_state(controls::Action::Dock);
			self.dock_entity = None;
			if let Some(mouse_entity) = self.mouse_entity
			{
				if let (
					Ok(player_pos),
					Ok(mut player_target),
					Ok(player_ship_state),
					Ok(pos),
					Ok(_),
					Ok(ship_state),
					ai,
				) = (
					self.world.get::<&comps::Position>(self.player),
					self.world.get::<&mut comps::Target>(self.player),
					self.world.get::<&comps::ShipState>(self.player),
					self.world.get::<&comps::Position>(mouse_entity),
					self.world.get::<&comps::Equipment>(mouse_entity),
					self.world.get::<&comps::ShipState>(mouse_entity),
					self.world.get::<&mut comps::AI>(mouse_entity),
				)
				{
					if ship_state.team.dock_with(&player_ship_state.team)
					{
						if let Ok(mut ai) = ai
						{
							ai.state = comps::AIState::Pause {
								time_to_unpause: state.time() + 1.,
							};
						}
						if (player_pos.pos.zx() - pos.pos.zx()).magnitude() < 10.0
						{
							player_target.clear(|m| to_die.push(m));
							self.dock_entity = Some(mouse_entity);
						}
					}
				}
			}
		}
		if want_zoom_in
		{
			self.zoom *= 1.25;
		}
		if want_zoom_out
		{
			self.zoom /= 1.25;
		}
		self.zoom = utils::clamp(self.zoom, 1., 4.);

		// Equipment actions
		let mut spawn_projectiles = vec![];
		for (id, (pos, equipment, ship_state)) in self
			.world
			.query::<(&comps::Position, &mut comps::Equipment, &comps::ShipState)>()
			.iter()
		{
			if !equipment.want_action_1
			{
				continue;
			}
			equipment.want_action_1 = false;
			for slot in &mut equipment.slots
			{
				if slot.is_inventory
				{
					continue;
				}
				if let Some(item) = slot.item.as_mut()
				{
					match &mut item.kind
					{
						comps::ItemKind::Weapon(weapon) =>
						{
							if state.time() > weapon.time_to_fire
							{
								let rot = Rotation2::new(pos.dir);
								let slot_dir = slot.dir.unwrap_or(0.);
								let rot_slot = Rotation2::new(slot_dir);
								let slot_pos = pos.pos.zx() + rot * slot.pos.coords;
								let slot_dir_vec = rot_slot * rot * Vector2::new(1., 0.);
								let target_dir = (equipment.target_pos.zx() - slot_pos).normalize();
								let min_dot = (weapon.stats.arc / 2.).cos();
								let min_dot_2 = (2. * weapon.stats.arc / 2.).cos();

								let spawn_pos = Point3::new(slot_pos.y, 3., slot_pos.x);
								let mut spawn_dir = None;
								if slot_dir_vec.dot(&target_dir) > min_dot
								{
									spawn_dir = Some(target_dir);
								}
								else if slot_dir_vec.dot(&target_dir) > min_dot_2
									&& equipment.allow_out_of_arc_shots
								{
									let cand_dir1 =
										Rotation2::new(slot_dir + weapon.stats.arc / 2.)
											* rot * Vector2::new(1., 0.);
									let cand_dir2 =
										Rotation2::new(slot_dir - weapon.stats.arc / 2.)
											* rot * Vector2::new(1., 0.);

									let cand_dir;
									if target_dir.dot(&cand_dir1) > target_dir.dot(&cand_dir2)
									{
										cand_dir = cand_dir1;
									}
									else
									{
										cand_dir = cand_dir2;
									}

									spawn_dir = Some(cand_dir);
								}
								if let Some(spawn_dir) = spawn_dir
								{
									let rot =
										Rotation2::new(self.rng.gen_range(-PI / 12.0..=PI / 12.0));
									let spawn_dir = rot * spawn_dir;
									let spawn_dir =
										Vector3::new(spawn_dir.y, 0.5, spawn_dir.x).normalize();
									spawn_projectiles.push((
										spawn_pos,
										spawn_dir,
										id,
										ship_state.team,
									));
									weapon.time_to_fire =
										state.time() + weapon.stats.fire_interval as f64;
								}
							}
						}
					}
				}
			}
		}

		for (spawn_pos, spawn_dir, parent, team) in spawn_projectiles
		{
			make_projectile(spawn_pos, spawn_dir, parent, team, &mut self.world, state)?;
		}

		// Update player pos.
		if let Ok(pos) = self.world.get::<&comps::Position>(self.player)
		{
			self.player_pos = pos.pos;
		}

		// Target movement.
		for (_, (target, pos, vel, stats)) in self
			.world
			.query::<(
				&mut comps::Target,
				&comps::Position,
				&mut comps::Velocity,
				&comps::Stats,
			)>()
			.iter()
		{
			if target.waypoints.is_empty()
			{
				vel.vel = Vector3::zeros();
				vel.dir_vel = 0.;
				continue;
			}
			let waypoint = target.waypoints.first().unwrap();
			let diff = waypoint.pos - pos.pos;
			if diff.magnitude() < 0.1
			{
				if target.waypoints.len() == 1
				{
					vel.vel = Vector3::zeros();
					vel.dir_vel = 0.;
				}
				if let Some(marker) = waypoint.marker
				{
					to_die.push(marker);
				}
				target.waypoints.remove(0);
				continue;
			}

			let diff = diff.zx().normalize();
			let rot = Rotation2::new(pos.dir);
			let forward = rot * Vector2::new(1., 0.);
			let left = rot * Vector2::new(0., 1.);
			if diff.dot(&left) > 0.
			{
				vel.dir_vel = stats.speed / 10.;
			}
			else
			{
				vel.dir_vel = -stats.speed / 10.;
			}
			vel.vel = stats.speed * Vector3::new(forward.y, 0., forward.x);
		}

		// AI
		for (id, (pos, target, ai, equipment, ship_state)) in self
			.world
			.query::<(
				&comps::Position,
				&mut comps::Target,
				&mut comps::AI,
				&mut comps::Equipment,
				&comps::ShipState,
			)>()
			.iter()
		{
			if Some(id) == self.dock_entity
			{
				target.clear(|m| to_die.push(m));
				continue;
			}
			match ai.state
			{
				comps::AIState::Pause { time_to_unpause } =>
				{
					target.clear(|m| to_die.push(m));
					if state.time() > time_to_unpause
					{
						ai.state = comps::AIState::Idle;
					}
				}
				comps::AIState::Idle =>
				{
					let sense_radius = 30.;
					let entries = grid.query_rect(
						pos.pos.zx() - Vector2::new(sense_radius, sense_radius) - center.coords,
						pos.pos.zx() + Vector2::new(sense_radius, sense_radius) - center.coords,
						|other| {
							if other.inner.entity == id
							{
								return false;
							}
							if let (Ok(other_pos), Ok(other_ship_state)) = (
								self.world.get::<&comps::Position>(other.inner.entity),
								self.world.get::<&comps::ShipState>(other.inner.entity),
							)
							{
								(pos.pos - other_pos.pos).magnitude() < sense_radius
									&& other_ship_state.team.is_enemy(&ship_state.team)
							}
							else
							{
								false
							}
						},
					);
					if let Some(entry) = entries.first()
					{
						ai.state = comps::AIState::Pursuing(entry.inner.entity);
					}
					else if target.waypoints.is_empty()
					{
						let cell_id = (0..self.cells.len()).choose(&mut self.rng).unwrap();
						target.waypoints.push(comps::Waypoint {
							pos: self.cells[cell_id].world_center(),
							marker: None,
						});
					}
				}
				comps::AIState::Pursuing(target_entity) =>
				{
					if self.world.contains(target_entity)
					{
						if self
							.world
							.get::<&comps::ShipState>(target_entity)
							.map(|other_ship_state| {
								!other_ship_state.team.is_enemy(&ship_state.team)
							})
							.unwrap_or(false)
						{
							ai.state = comps::AIState::Idle;
						}
						else
						{
							let target_pos =
								self.world.get::<&comps::Position>(target_entity).unwrap();
							let diff = pos.pos - target_pos.pos;
							if diff.magnitude() < 20.
							{
								target.clear(|m| to_die.push(m));
								ai.state = comps::AIState::Attacking(target_entity);
							}
							else if diff.magnitude() > 30.
							{
								ai.state = comps::AIState::Idle;
							}
							else if Some(id) != self.dock_entity
							{
								target.clear(|m| to_die.push(m));
								target.waypoints.push(comps::Waypoint {
									pos: target_pos.pos,
									marker: None,
								})
							}
						}
					}
					else
					{
						ai.state = comps::AIState::Idle;
					}
				}
				comps::AIState::Attacking(target_entity) =>
				{
					if self.world.contains(target_entity)
					{
						if self
							.world
							.get::<&comps::ShipState>(target_entity)
							.map(|other_ship_state| {
								!other_ship_state.team.is_enemy(&ship_state.team)
							})
							.unwrap_or(false)
						{
							ai.state = comps::AIState::Idle;
							equipment.want_action_1 = false;
						}
						else
						{
							let target_pos =
								self.world.get::<&comps::Position>(target_entity).unwrap();
							let diff = target_pos.pos - pos.pos;
							if diff.magnitude() > 20.
							{
								ai.state = comps::AIState::Pursuing(target_entity);
								equipment.want_action_1 = false;
							}
							else
							{
								if target.waypoints.is_empty() && Some(id) != self.dock_entity
								{
									let theta = [-PI / 3., PI / 3.].choose(&mut self.rng).unwrap();
									let rot = Rotation2::new(*theta);
									let new_disp = rot * diff.zx() * 2.;
									target.waypoints.push(comps::Waypoint {
										pos: pos.pos + Vector3::new(new_disp.y, 0., new_disp.x),
										marker: None,
									});
								}
								equipment.want_action_1 = true;
								equipment.target_pos = target_pos.pos;
							}
						}
					}
					else
					{
						ai.state = comps::AIState::Idle;
					}
				}
			}
		}

		// Ship state death
		let mut remove_ai = vec![];
		for (id, (target, ship_state)) in self
			.world
			.query_mut::<(&mut comps::Target, &mut comps::ShipState)>()
		{
			if !ship_state.is_active() && ship_state.team != comps::Team::Neutral
			{
				target.clear(|m| to_die.push(m));
				ship_state.team = comps::Team::Neutral;
				remove_ai.push(id);
			}
		}
		for id in remove_ai
		{
			// Player has no AI.
			self.world.remove_one::<comps::AI>(id).ok();
		}

		// Time to die
		for (id, time_to_die) in self.world.query_mut::<&comps::TimeToDie>()
		{
			if state.time() > time_to_die.time_to_die
			{
				to_die.push(id);
			}
		}

		// Remove dead entities
		to_die.sort();
		to_die.dedup();
		for id in to_die
		{
			//println!("died {id:?}");
			self.world.despawn(id)?;
		}

		Ok(None)
	}

	fn input(
		&mut self, _event: &Event, _state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		Ok(None)
	}

	fn draw(&mut self, state: &game_state::GameState, draw_ui: bool) -> Result<()>
	{
		state.core.set_depth_test(Some(DepthFunction::Less));

		let project = self.make_project();
		state
			.core
			.use_projection_transform(&utils::mat4_to_transform(project.to_homogeneous()));

		let camera = self.make_camera();

		let tl = utils::get_ground_from_screen(-1.0, 1.0, project, camera);
		let tr = utils::get_ground_from_screen(1.0, 1.0, project, camera);
		let bl = utils::get_ground_from_screen(-1.0, -1.0, project, camera);
		let br = utils::get_ground_from_screen(1.0, -1.0, project, camera);
		let vtxs = [
			mesh::WaterVertex {
				x: tl.x,
				y: tl.y,
				z: tl.z,
			},
			mesh::WaterVertex {
				x: tr.x,
				y: tr.y,
				z: tr.z,
			},
			mesh::WaterVertex {
				x: br.x,
				y: br.y,
				z: br.z,
			},
			mesh::WaterVertex {
				x: bl.x,
				y: bl.y,
				z: bl.z,
			},
		];
		state
			.core
			.use_transform(&utils::mat4_to_transform(camera.to_homogeneous()));
		state
			.core
			.use_shader(Some(&*state.water_shader.upgrade().unwrap()))
			.unwrap();
		state
			.core
			.set_shader_uniform("time", &[state.core.get_time() as f32][..])
			.ok();
		state.prim.draw_prim(
			&vtxs[..],
			Option::<&Bitmap>::None,
			0,
			4,
			PrimType::TriangleFan,
		);

		state
			.core
			.use_shader(Some(&*state.basic_shader.upgrade().unwrap()))
			.unwrap();
		for (id, (pos, mesh)) in self
			.world
			.query::<(&comps::Position, &comps::Mesh)>()
			.iter()
		{
			let mut shift = Isometry3::new(pos.pos.coords, pos.dir * Vector3::y()).to_homogeneous();
			if let Ok(tilt) = self.world.get::<&comps::Tilt>(id)
			{
				shift = shift
					* Rotation3::from_axis_angle(&Vector3::x_axis(), tilt.tilt).to_homogeneous();
			}

			state
				.core
				.use_transform(&utils::mat4_to_transform(camera.to_homogeneous() * shift));
			state
				.core
				.set_shader_transform("model_matrix", &utils::mat4_to_transform(shift))
				.ok();

			let flag_mapper = |material_name: &str, texture_name: &str| -> Option<&Bitmap> {
				if material_name == "flag_material"
				{
					if let Ok(ship_state) = self.world.get::<&comps::ShipState>(id)
					{
						let texture_name = match ship_state.team
						{
							comps::Team::English => "data/english_flag.png",
							comps::Team::French => "data/french_flag.png",
							_ => texture_name,
						};
						state.get_bitmap(texture_name)
					}
					else
					{
						state.get_bitmap(texture_name)
					}
				}
				else
				{
					state.get_bitmap(texture_name)
				}
			};

			state
				.get_mesh(&mesh.mesh)
				.unwrap()
				.draw(&state.prim, flag_mapper) //|s| state.get_bitmap(s));
		}

		let (dw, dh) = (self.buffer_width, self.buffer_height);
		let ortho_mat = Matrix4::new_orthographic(0., dw, dh, 0., -1., 1.);
		state
			.core
			.use_projection_transform(&utils::mat4_to_transform(ortho_mat));
		state.core.use_transform(&Transform::identity());
		state.core.set_depth_test(None);
		state
			.core
			.use_shader(Some(&*state.default_shader.upgrade().unwrap()))
			.unwrap();

		if draw_ui
		{
			let mut weapon_slots = vec![];
			if let (Ok(pos), Ok(equipment)) = (
				self.world.get::<&comps::Position>(self.player),
				self.world.get::<&comps::Equipment>(self.player),
			)
			{
				for slot in &equipment.slots
				{
					if slot.is_inventory
					{
						continue;
					}
					if let Some(item) = slot.item.as_ref()
					{
						match &item.kind
						{
							comps::ItemKind::Weapon(weapon) =>
							{
								weapon_slots.push((
									pos.pos,
									pos.dir,
									weapon.time_to_fire,
									weapon.stats.fire_interval,
									slot.pos,
									slot.dir.unwrap_or(0.),
									weapon.stats.arc,
								));
							}
						}
					}
				}
			}
			let w = 32.;
			let total = weapon_slots.len() as f32 * w;
			let offt = total / 2.;
			let mouse_ground_pos = self.get_mouse_ground_pos(state);

			for (i, (pos, dir, time_to_fire, fire_interval, slot_pos, slot_dir, arc)) in
				weapon_slots.iter().enumerate()
			{
				let x = i as f32 * w - offt + dw / 2.;
				let y = dh - 2. * w;
				let f =
					1. - utils::clamp((time_to_fire - state.time()) as f32 / fire_interval, 0., 1.);

				let rot = Rotation2::new(*dir);
				let rot_slot = Rotation2::new(*slot_dir);
				let slot_pos = pos.zx() + rot * slot_pos.coords;
				let slot_vec_dir = rot_slot * rot * Vector2::new(1., 0.);
				let target_dir = (mouse_ground_pos.zx() - slot_pos).normalize();
				let min_dot = (arc / 2.).cos();

				if slot_vec_dir.dot(&target_dir) > min_dot
				{
					state.prim.draw_filled_pieslice(
						x + w / 2.,
						y + w / 2.,
						w / 2.,
						-slot_dir - arc / 2. + PI * 3. / 2.,
						*arc,
						Color::from_rgba_f(f, f, f, f),
					);
				}
				else
				{
					state.prim.draw_pieslice(
						x + w / 2.,
						y + w / 2.,
						w / 2.,
						-slot_dir - arc / 2. + PI * 3. / 2.,
						*arc,
						Color::from_rgba_f(f, f, f, f),
						3.,
					);
				}
			}

			if let Ok(ship_state) = self.world.get::<&comps::ShipState>(self.player)
			{
				draw_ship_state(&*ship_state, dw - 100., dh - 128., state);
			}

			if let Some(ship_state) = self
				.mouse_entity
				.as_ref()
				.and_then(|e| self.world.get::<&comps::ShipState>(*e).ok())
			{
				if self.mouse_entity != Some(self.player)
				{
					draw_ship_state(&*ship_state, 16., dh - 128., state);
				}
			}
			state.core.draw_text(
				&state.ui_font,
				Color::from_rgb_f(1., 1., 1.),
				dw / 2.0,
				16.,
				FontAlign::Centre,
				&format!("Money: ${}", self.money),
			);
		}

		Ok(())
	}
}
