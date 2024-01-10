use crate::error::Result;
use crate::utils::ColorExt;
use crate::{
	astar, components as comps, controls, game_state, mesh, spatial_grid, sprite, ui, utils,
};
use allegro::*;
use allegro_font::*;
use allegro_primitives::*;
use gl::CULL_FACE;
use gltf::accessor;
use gltf::accessor::util::SparseIndicesIter;
use na::{
	Isometry3, Matrix4, Perspective3, Point2, Point3, Quaternion, RealField, Rotation2, Rotation3,
	Similarity3, Translation3, Unit, Vector2, Vector3, Vector4,
};
use nalgebra as na;
use rand::prelude::*;
use serde_derive::{Deserialize, Serialize};

use std::collections::HashMap;

use std::f32::consts::PI;

const CELL_SIZE: i32 = 128;
const CELL_RADIUS: i32 = 2;
const SLOT_WIDTH: f32 = 64.;
const CREW_COST: i32 = 20;
const MESSAGE_DURATION: f32 = 10.;
const EQUIPMENT_FRAC: f32 = 0.6;
const ECONOMY_INTERVAL: f64 = 30.;

#[derive(Clone, Debug)]
#[repr(usize)]
pub enum Price
{
	Weapon,
	Goods,
	Cotton,
	Tobacco,
	Officer,
}

struct Timer
{
	name: &'static str,
	start: f64,
	end: f64,
	tick: i64,
}

impl Timer
{
	fn new(name: &'static str, state: &game_state::GameState) -> Self
	{
		Self {
			name: name,
			start: state.core.get_time(),
			end: 0.,
			tick: state.tick,
		}
	}

	fn record(&mut self, core: &Core)
	{
		self.end = core.get_time();
	}
}

impl Drop for Timer
{
	fn drop(&mut self)
	{
		let dur = self.end - self.start;
		if dur > 1e-3 && self.tick % 64 == 0
		{
			println!("{}: {:.4}", self.name, dur);
		}
	}
}

fn frac_to_color(f: f32) -> Color
{
	if f == 1.
	{
		ui::ui_color()
	}
	else if f < 0.33
	{
		Color::from_rgb_f(0.9, 0.1, 0.1)
	}
	else if f < 0.66
	{
		Color::from_rgb_f(0.9, 0.9, 0.1)
	}
	else
	{
		Color::from_rgb_f(0.1, 0.9, 0.1)
	}
}

#[derive(Clone)]
pub struct Button
{
	loc: Point2<f32>,
	size: Vector2<f32>,
	sprite: String,
	on: bool,
	hover: bool,
	is_toggle: bool,
}

impl Button
{
	fn new(loc: Point2<f32>, size: Vector2<f32>, is_toggle: bool, sprite: String) -> Self
	{
		Self {
			loc: loc,
			size: size,
			sprite: sprite,
			on: false,
			hover: false,
			is_toggle: is_toggle,
		}
	}

	fn logic(&mut self) -> bool
	{
		let old_on = self.on;
		if !self.is_toggle
		{
			self.on = false;
		}
		old_on
	}

	fn draw(&self, state: &game_state::GameState)
	{
		let variant = if self.on
		{
			2
		}
		else if self.hover
		{
			1
		}
		else
		{
			0
		};
		state.get_sprite(&self.sprite).unwrap().draw(
			self.loc,
			variant,
			Color::from_rgb_f(1., 1., 1.),
			state,
		);
	}

	fn input(&mut self, event: &Event) -> bool
	{
		let start = self.loc - self.size / 2.;
		let end = self.loc + self.size / 2.;
		let mut handled = false;
		match event
		{
			Event::MouseButtonDown {
				button: 1, x, y, ..
			} =>
			{
				let (x, y) = (*x as f32, *y as f32);
				if x > start.x && x < end.x && y < end.y && y > start.y
				{
					if self.is_toggle
					{
						self.on = !self.on;
					}
					else
					{
						self.on = true;
					}
					handled = true;
				}
			}
			Event::MouseAxes { x, y, .. } =>
			{
				let (x, y) = (*x as f32, *y as f32);
				self.hover = x > start.x && x < end.x && y < end.y && y > start.y;
			}
			_ => (),
		}
		handled
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
		center: Point2<i32>, level: i32, rng: &mut R, world: &mut hecs::World,
		state: &mut game_state::GameState,
	) -> Result<Self>
	{
		let world_center = Self::cell_to_world(center);

		//dbg!(world_center);

		let w = CELL_SIZE as f32 / 2. - 10.;

		let num_enemies = if center == Point2::origin() { 0 } else { 1 };

		for _ in 0..num_enemies
		{
			let dx = world_center.x + rng.gen_range(-w..w);
			let dy = world_center.z + rng.gen_range(-w..w);

			let idx = rand_distr::WeightedIndex::new([3., 3., 1.])
				.unwrap()
				.sample(rng);
			let team = [
				comps::Team::English,
				comps::Team::French,
				comps::Team::Pirate,
			][idx];
			//let team = comps::Team::French;
			//let team = comps::Team::English;
			//let team = comps::Team::Pirate;

			let idx = rand_distr::WeightedIndex::new([10., 5., 1.])
				.unwrap()
				.sample(rng);
			let ship_pos = Point3::new(dx, 0., dy);
			if center != Cell::world_to_cell(&ship_pos)
			{
				println!(
					"BAD {:?} {:?} {:?} {:?} {} {}",
					center,
					Cell::world_to_cell(&ship_pos),
					world_center,
					ship_pos,
					dx,
					dy
				);
			}
			let ship = make_ship(
				ship_pos,
				[
					"data/small_ship.cfg",
					"data/medium_ship.cfg",
					"data/big_ship.cfg",
				][idx],
				team,
				if team == comps::Team::Pirate
				{
					level + 3
				}
				else
				{
					level
				},
				rng,
				world,
				state,
			)?;

			world.insert_one(
				ship,
				comps::AI {
					state: comps::AIState::Idle,
					name: comps::generate_captain_name(team, rng),
				},
			)?;
			//world.get::<&mut comps::ShipState>(ship).unwrap().crew = 0;
			//world.get::<&mut comps::ShipState>(ship).unwrap().hull = 1.;
		}

		//for _ in 0..2
		//{
		//	let dx = world_center.x + rng.gen_range(-w..w);
		//	let dy = world_center.z + rng.gen_range(-w..w);

		//	let dir = rng.gen_range(0.0..PI * 2.0);
		//	let vel = Vector3::new(dir.cos(), 0., dir.sin()) * 5.;

		//	make_wisp(Point3::new(dx, 0., dy), vel, world, state)?;
		//}

		Ok(Self { center: center })
	}

	pub fn world_center(&self) -> Point3<f32>
	{
		Self::cell_to_world(self.center)
	}

	pub fn cell_to_world(pos: Point2<i32>) -> Point3<f32>
	{
		Point3::new((pos.x * CELL_SIZE) as f32, 0., (pos.y * CELL_SIZE) as f32)
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

struct HUD
{
	buffer_height: f32,
	buffer_width: f32,
	buttons: Vec<Button>,
	toggled: Vec<usize>,
}

impl HUD
{
	fn new(state: &mut game_state::GameState) -> Self
	{
		let mut buttons = vec![];
		// Sigh... why does it always end up like this? Despicable, regrettable garbage...
		let (dw, dh) = (state.display_width, state.display_height);
		let m = state.m;
		let (x, mut y) = (dw - m * 6., dh - m * 9.);

		let size = Vector2::new(m * 2., m);

		let sprite_name = "data/repair.cfg".to_string();

		for i in 0..4
		{
			let theta = -PI / 2. + i as f32 * PI / 2.;

			let r = m * 1.5;
			let cx = x;

			let lx = cx + (m * 1.5 + r) * theta.cos();
			let ly = y + (m + r) * theta.sin();

			let offt = if i == 3 { -m * 2. } else { m * 2. };
			buttons.push(Button::new(
				Point2::new(lx + offt, ly),
				size,
				true,
				sprite_name.clone(),
			));
		}

		y += m * 4.;

		let h = m;
		let x = x - m * 5.;

		buttons.push(Button::new(
			Point2::new(x, y),
			size,
			true,
			sprite_name.clone(),
		));
		y += h;
		y += h;
		buttons.push(Button::new(
			Point2::new(x, y),
			size,
			true,
			sprite_name.clone(),
		));
		y += h;
		buttons.push(Button::new(
			Point2::new(x, y),
			size,
			true,
			sprite_name.clone(),
		));

		Self {
			buffer_width: dw,
			buffer_height: dh,
			buttons: buttons,
			toggled: vec![],
		}
	}

	fn status_pos(&self, idx: i32, m: f32) -> Point2<f32>
	{
		let (dw, dh) = (self.buffer_width, self.buffer_height);
		if idx == 0
		{
			Point2::new(m * 6., dh - m * 9.)
		}
		else
		{
			Point2::new(dw - m * 6., dh - m * 9.)
		}
	}

	fn over_ui(&self, state: &game_state::GameState) -> bool
	{
		let mouse_pos = Point2::new(state.mouse_pos.x as f32, state.mouse_pos.y as f32);
		let status_pos_0 = self.status_pos(0, state.m);
		let status_pos_1 = self.status_pos(1, state.m);
		let m = state.m;
		let h = m * 7.;
		let w = m * 9.;
		(mouse_pos.x < status_pos_0.x + w && mouse_pos.y > status_pos_0.y - h)
			|| (mouse_pos.x > status_pos_1.x - w && mouse_pos.y > status_pos_1.y - h)
	}

	fn input(&mut self, event: &Event, _map: &mut Map, state: &mut game_state::GameState) -> bool
	{
		let mut over_ui = false;
		for (i, button) in &mut self.buttons.iter_mut().enumerate()
		{
			let old_on = button.on;
			over_ui |= button.input(event);
			if !old_on && button.on
			{
				self.toggled.push(i);
			}
		}
		self.toggled.retain(|i| self.buttons[*i].on);
		//self.over_ui(state)
		over_ui
	}

	fn logic(&mut self, map: &mut Map, _state: &game_state::GameState)
	{
		if let Ok(mut ship_state) = map.world.get::<&mut comps::ShipState>(map.player)
		{
			let max_repair = 2;
			if self.toggled.len() > max_repair as usize
			{
				for i in &self.toggled[..(self.toggled.len() - max_repair as usize)]
				{
					self.buttons[*i].on = false;
				}
				self.toggled
					.drain(..(self.toggled.len() - max_repair as usize));
			}
			ship_state.repair_boost.clone_from(&self.toggled);
		}
	}

	fn draw(&self, map: &Map, state: &game_state::GameState)
	{
		let ui_color = ui::ui_color();
		let (dw, dh) = (self.buffer_width, self.buffer_height);
		let m = state.m;

		let mut weapon_slots = vec![];
		if let (Ok(pos), Ok(equipment)) = (
			map.world.get::<&comps::Position>(map.player),
			map.world.get::<&comps::Equipment>(map.player),
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
								weapon.readiness,
								slot.pos,
								slot.dir.unwrap_or(0.),
								weapon.stats().arc,
								item.kind.clone(),
							));
						}
						_ => (),
					}
				}
			}
		}
		let w = m * 3.;
		let total = weapon_slots.len() as f32 * w;
		let offt = total / 2.;
		let mouse_ground_pos = map.get_mouse_ground_pos(state);

		for (i, (pos, dir, fire_readiness, slot_pos, slot_dir, arc, kind)) in
			weapon_slots.iter().enumerate()
		{
			let x = i as f32 * w - offt + dw / 2.;
			let y = dh - 2. * w;
			let f = *fire_readiness;

			let rot = Rotation2::new(*dir);
			let rot_slot = Rotation2::new(*slot_dir);
			let slot_pos = pos.zx() + rot * slot_pos.coords;
			let slot_vec_dir = rot_slot * rot * Vector2::new(1., 0.);
			let target_dir = (mouse_ground_pos.zx() - slot_pos).normalize();
			let min_dot = (arc / 2.).cos();

			draw_item(x + w / 2., y + 64. + w / 2., &kind, state);
			if slot_vec_dir.dot(&target_dir) > min_dot
			{
				state.prim.draw_filled_pieslice(
					x + w / 2.,
					y + w / 2.,
					w / 3.,
					-slot_dir - arc / 2. + PI * 3. / 2.,
					*arc,
					frac_to_color(f),
				);
			}
			else
			{
				state.prim.draw_pieslice(
					x + w / 2.,
					y + w / 2.,
					w / 3.,
					-slot_dir - arc / 2. + PI * 3. / 2.,
					*arc,
					frac_to_color(f),
					3.,
				);
			}
		}

		if let (Ok(ship_state), Ok(stats)) = (
			map.world.get::<&comps::ShipState>(map.player),
			map.world.get::<&comps::ShipStats>(map.player),
		)
		{
			let status_pos = self.status_pos(1, m);
			draw_ship_state(&*ship_state, &*stats, status_pos.x, status_pos.y, state);

			let f = (ship_state.experience - comps::level_experience(ship_state.level))
				/ (comps::level_experience(ship_state.level + 1)
					- comps::level_experience(ship_state.level));
			state.prim.draw_filled_rectangle(
				dw / 3.,
				dh - 16.,
				dw / 3. + (dw / 3.) * f,
				dh,
				Color::from_rgb_f(0.7, 0.7, 0.2),
			);
		}

		if let Some(target_entity) = map.target_entity
		{
			let status_pos = self.status_pos(0, m);
			if let Ok(ai) = map.world.get::<&comps::AI>(target_entity)
			{
				state.core.draw_text(
					&state.ui_font,
					Color::from_rgb_f(1., 1., 1.),
					status_pos.x,
					status_pos.y - m * 7.,
					FontAlign::Centre,
					&ai.name,
				);
			}

			if let (Ok(ship_state), Ok(stats)) = (
				map.world.get::<&comps::ShipState>(target_entity),
				map.world.get::<&comps::ShipStats>(target_entity),
			)
			{
				draw_ship_state(&*ship_state, &*stats, status_pos.x, status_pos.y, state);
			}
		}
		state.core.draw_text(
			&state.ui_font,
			ui_color,
			dw / 2.0,
			16.,
			FontAlign::Centre,
			&format!("Money: £{}", map.money),
		);

		let lh = state.ui_font.get_line_height() as f32;

		let num_messages = map.messages.len();
		for (i, (message, time)) in map.messages.iter().enumerate()
		{
			let f = 1. - (state.time() - time) as f32 / MESSAGE_DURATION;
			state.core.draw_text(
				&state.ui_font,
				ui_color.interpolate(Color::from_rgba(0, 0, 0, 0), 1. - f),
				dw / 2.0,
				dh / 4.0 - i as f32 * lh * 1.5 + num_messages as f32 * lh * 1.5,
				FontAlign::Centre,
				&message,
			);
		}

		for toggle in &self.buttons
		{
			toggle.draw(state);
		}
	}
}

pub struct Game
{
	map: Map,
	equipment_screen: Option<EquipmentScreen>,
	subscreens: Vec<ui::SubScreen>,
	hud: HUD,
}

impl Game
{
	pub fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		Ok(Self {
			map: Map::new(state)?,
			subscreens: vec![],
			equipment_screen: None,
			hud: HUD::new(state),
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
			self.hud.logic(&mut self.map, state);
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
		let mut handled = false;
		if let Some(equipment_screen) = self.equipment_screen.as_mut()
		{
			handled |= equipment_screen.input(event, &mut self.map, state);
		}
		handled |= self.hud.input(event, &mut self.map, state);
		if !handled
		{
			state.controls.decode_event(event);
			let want_move = state.controls.get_action_state(controls::Action::Move) > 0.5;
			if self.map.dock_entity.is_some() && want_move && self.equipment_screen.is_some()
			{
				self.equipment_screen
					.as_mut()
					.unwrap()
					.finish_trade(&mut self.map);
				self.equipment_screen = None;
				self.map.dock_entity = None;
			}
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
					.push(ui::SubScreen::InGameMenu(ui::InGameMenu::new(state)));
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
						self.subscreens.push(subscreen_fn(state));
					}
					ui::Action::Start =>
					{
						state.paused = false;
						return Ok(Some(game_state::NextScreen::Game));
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
		self.map.draw(state)?;

		let (dw, dh) = (state.display_width, state.display_height);
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
		state
			.core
			.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::InverseAlpha);

		if self.subscreens.is_empty()
		{
			self.hud.draw(&self.map, state);
		}
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

	pub fn change_buffers(&mut self, state: &mut game_state::GameState) -> Result<()>
	{
		self.hud = HUD::new(state);
		self.map.buffer_width = state.display_width;
		self.map.buffer_height = state.display_height;
		self.subscreens.clear();
		self.subscreens
			.push(ui::SubScreen::InGameMenu(ui::InGameMenu::new(state)));
		self.subscreens
			.push(ui::SubScreen::OptionsMenu(ui::OptionsMenu::new(state)));

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
	recruit: Option<Button>,

	grab_attempted: bool,
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
			recruit: None,
			grab_attempted: false,
		}
	}

	fn get_slot_pos(&self, equipment_idx: i32, real_pos: Point2<f32>) -> Point2<f32>
	{
		let (bw, bh) = (self.buffer_width, self.buffer_height);
		Point2::new(-real_pos.y, -real_pos.x) * 32.
			+ Vector2::new(bw / 6. + bw * 2. / 3. * equipment_idx as f32, bh / 4.)
	}

	fn over_ui(&self, map: &mut Map, state: &game_state::GameState) -> bool
	{
		let mouse_pos = Point2::new(state.mouse_pos.x as f32, state.mouse_pos.y as f32);
		let in_right = mouse_pos.x > self.buffer_width * 2. / 3.
			&& mouse_pos.y < self.buffer_height * EQUIPMENT_FRAC;
		let in_left = mouse_pos.x < self.buffer_width * 1. / 3.
			&& mouse_pos.y < self.buffer_height * EQUIPMENT_FRAC;
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
		if let Some(button) = self.recruit.as_mut()
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
				self.grab_attempted = false;
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

	fn logic(&mut self, map: &mut Map, state: &mut game_state::GameState) -> bool
	{
		if map.dock_entity.is_some() && (self.switch_ships.is_none() && self.recruit.is_none())
		{
			if let (Ok(dock_state), Ok(player_state)) = (
				map.world.get::<&comps::ShipState>(map.dock_entity.unwrap()),
				map.world.get::<&comps::ShipState>(map.player),
			)
			{
				if dock_state.hull > 0.
					&& dock_state.team != player_state.team
					&& !player_state.is_boss
				{
					self.switch_ships = Some(Button::new(
						Point2::new(state.display_width / 3. - 64., 32.),
						Vector2::new(64., 32.),
						false,
						"data/switch.cfg".into(),
					));
				}
				if dock_state.team == player_state.team
				{
					self.recruit = Some(Button::new(
						Point2::new(state.display_width / 3. - 64., 32.),
						Vector2::new(64., 32.),
						false,
						"data/recruit.cfg".into(),
					));
				}
			}
		}
		else if map.dock_entity.is_none()
		{
			self.switch_ships = None;
			self.recruit = None;
		}
		let do_switch = if let Some(button) = self.switch_ships.as_mut()
		{
			button.logic()
		}
		else
		{
			false
		};
		let do_recruit = if let Some(button) = self.recruit.as_mut()
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
										if !self.grab_attempted
										{
											map.messages.push((
												"Not enough money!".to_string(),
												state.time(),
											));
										}
										self.grab_attempted = true;
									}
									else
									{
										map.money -= item.price;
									}
								}
							}
							if start_grab
							{
								self.dragged_item =
									slot.item.take().map(|item| (i, equipment_idx, item));
								if self.dragged_item.is_some()
								{
									state.sfx.play_sound("data/equipment.ogg").unwrap();
								}
								if self.ctrl_down
								{
									fast_move = true;
								}
							}
						}
						else if !self.mouse_button_down && self.dragged_item.is_some()
						{
							state.sfx.play_sound("data/equipment.ogg").unwrap();
							let is_weapon = if let Some(comps::ItemKind::Weapon(_)) =
								self.dragged_item.as_ref().map(|(_, _, i)| &i.kind)
							{
								true
							}
							else
							{
								false
							};
							if is_weapon && !slot.weapons_allowed
							{
								old_item = self.dragged_item.take();
							}
							else
							{
								let mut do_transaction = true;
								if let Some(item) = slot.item.as_ref()
								{
									if equipment_idx == 0 && do_trade
									{
										if map.money >= item.price
										{
											map.money -= item.price;
										}
										else
										{
											do_transaction = false;
											map.messages.push((
												"Not enough money!".to_string(),
												state.time(),
											));
										}
									}
								}
								if do_transaction
								{
									// Drop item.
									// If dropping into trade partner's window, grab the money.
									let (source_i, source_equipment_idx, mut item) =
										self.dragged_item.take().unwrap();
									if equipment_idx == 0 && do_trade
									{
										map.money += item.price;
									}
									old_item = slot
										.item
										.take()
										.map(|item| (source_i, source_equipment_idx, item));
									item.reset_cooldowns();
									slot.item = Some(item);
								}
								else
								{
									old_item = self.dragged_item.take();
								}
							}
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
						if do_trade
						{
							map.money += item.price;
						}

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
							let item_price = item.price;
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
							// This is in lieu of the logic for dropping.
							if do_trade && moved
							{
								map.money += item_price;
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
		if do_switch || do_recruit
		{
			let mut query = map.world.query::<&mut comps::ShipState>();
			let mut view = query.view();
			let [mut dock_state, mut player_state] =
				view.get_mut_n([map.dock_entity.unwrap(), map.player]);
			if let (Some(dock_state), Some(player_state)) =
				(dock_state.as_mut(), player_state.as_mut())
			{
				if do_switch
				{
					let player_crew = player_state.crew;
					let player_wounded = player_state.wounded;
					let player_experience = player_state.experience;
					let player_team = player_state.team;

					player_state.crew = dock_state.crew;
					player_state.wounded = dock_state.wounded;
					player_state.experience = dock_state.experience;
					player_state.team = dock_state.team;

					dock_state.crew = player_crew;
					dock_state.wounded = player_wounded;
					dock_state.experience = player_experience;
					dock_state.team = player_team;

					if dock_state.is_boss
					{
						map.messages
							.push(("You are now a slave to the Voidwind!".into(), state.time()));
						map.messages.push((
							"Now that you are on board, you can never leave...".into(),
							state.time(),
						));
						dock_state.team = comps::Team::Pirate;
					}
				}
				if do_recruit
				{
					if let (Ok(dock_stats), Ok(player_stats)) = (
						map.world.get::<&comps::ShipStats>(map.dock_entity.unwrap()),
						map.world.get::<&comps::ShipStats>(map.player),
					)
					{
						if dock_state.crew < dock_stats.crew * 2 / 3
						{
							map.messages
								.push(("Not enough crew to recruit!".to_string(), state.time()));
						}
						else if map.money < dock_state.level * CREW_COST
						{
							map.messages
								.push(("Not enough money!".to_string(), state.time()));
						}
						else if player_state.crew >= player_stats.crew
						{
							map.messages
								.push(("No room for more crew!".to_string(), state.time()));
						}
						else
						{
							let player_count = (player_state.crew + player_state.wounded) as f32;
							let new_experience =
								(player_count * player_state.experience + 1.) / (player_count + 1.);
							dock_state.crew -= 1;
							player_state.crew += 1;
							player_state.experience = new_experience;
							player_state.compute_level();
							//dbg!(player_state.experience);
							map.money -= dock_state.level * CREW_COST;
						}
					}
				}
			}

			if do_switch
			{
				let player = map.player;
				map.player = map.dock_entity.unwrap();
				map.dock_entity = Some(player);
				if map.target_entity.is_some()
				{
					map.target_entity = Some(map.dock_entity.unwrap());
				}
			}
		}
		!over_ui
	}

	fn finish_trade(&mut self, map: &mut Map)
	{
		let do_trade = self.do_trade(map);

		{
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
		if let (Ok(mut ship_state), Ok(stats)) = (
			map.world.get::<&mut comps::ShipState>(map.player),
			map.world.get::<&comps::ShipStats>(map.player),
		)
		{
			let overflow = ship_state.crew + ship_state.wounded - stats.crew;
			if overflow > 0
			{
				// ...I guess we dump them overboard? LOL...
				ship_state.wounded -= overflow.min(ship_state.wounded);
			}
			let overflow = ship_state.crew + ship_state.wounded - stats.crew;
			if overflow > 0
			{
				ship_state.crew -= overflow;
			}
		}
	}

	fn draw(&self, map: &Map, state: &game_state::GameState)
	{
		let m = state.m;
		let lh = state.ui_font.get_line_height() as f32;
		let ui_color = ui::ui_color();
		if map.dock_entity.is_some()
		{
			state.prim.draw_filled_rectangle(
				0.,
				0.,
				self.buffer_width * 1. / 3.,
				self.buffer_height * EQUIPMENT_FRAC,
				Color::from_rgb_f(0.1, 0.1, 0.2),
			);
		}
		state.prim.draw_filled_rectangle(
			self.buffer_width * 2. / 3.,
			0.,
			self.buffer_width,
			self.buffer_height * EQUIPMENT_FRAC,
			Color::from_rgb_f(0.1, 0.1, 0.2),
		);
		let do_trade = self.do_trade(map);
		let mouse_pos = Point2::new(state.mouse_pos.x as f32, state.mouse_pos.y as f32);
		let crew_level = map
			.dock_entity
			.and_then(|e| map.world.get::<&comps::ShipState>(e).ok())
			.map(|ss| ss.level)
			.unwrap_or(1);

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
						hover_item = Some((pos, equipment_idx, item.clone()));
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
					ui_color,
					3.,
				);

				if let Some(item) = slot.item.as_ref()
				{
					draw_item(pos.x, pos.y, &item.kind, state);
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
						ui_color,
						4.,
					);
				}
			}

			if let Some((pos, equipment_idx, item)) = hover_item
			{
				let ui_color = ui::ui_color();
				let price_desc = if do_trade
				{
					let price = item.price;
					vec![
						(format!("Price: {price}"), Color::from_rgb_f(1., 0.6, 0.2)),
						("".into(), ui_color),
					]
				}
				else
				{
					vec![]
				};

				let name = vec![(item.kind.name(), item.kind.color())];
				let desc = item.kind.description();

				let lines: Vec<_> = price_desc
					.iter()
					.map(|(s, c)| (s.as_str(), *c))
					.chain(name.iter().map(|(s, c)| (*s, *c)))
					.chain(desc.lines().map(|s| (s, ui_color)))
					.collect();

				state.prim.draw_filled_rectangle(
					pos.x + m * 16. * [1., -1.][equipment_idx as usize],
					pos.y,
					pos.x,
					pos.y + m * (lines.len() as f32 + 2.),
					Color::from_rgba_f(0., 0., 0., 0.75),
				);

				let x = pos.x + m * 8. * [1., -1.][equipment_idx as usize];
				let mut y = pos.y + m * 1.;

				for (line, color) in lines
				{
					state
						.core
						.draw_text(&state.ui_font, color, x, y, FontAlign::Centre, line);
					y += lh;
				}
			}

			if let Some((_, _, ref item)) = self.dragged_item
			{
				draw_item(mouse_pos.x, mouse_pos.y, &item.kind, state);
			}
		}

		if let Some(button) = self.switch_ships.as_ref()
		{
			button.draw(state);
			state.core.draw_text(
				&state.ui_font,
				Color::from_rgb_f(1., 1., 1.),
				button.loc.x - button.size.x,
				button.loc.y - lh / 2.,
				FontAlign::Right,
				"Switch Ships",
			);
		}
		if let Some(button) = self.recruit.as_ref()
		{
			button.draw(state);
			state.core.draw_text(
				&state.ui_font,
				Color::from_rgb_f(1., 1., 1.),
				button.loc.x - button.size.x,
				button.loc.y - lh / 2.,
				FontAlign::Right,
				&format!("Recruit Crew £{}", crew_level * CREW_COST),
			);
		}
	}
}

fn draw_item(x: f32, y: f32, item_kind: &comps::ItemKind, state: &game_state::GameState)
{
	item_kind.draw(Point2::new(x, y), state);
}

fn draw_ship_state(
	ship_state: &comps::ShipState, stats: &comps::ShipStats, x: f32, y: f32,
	state: &game_state::GameState,
)
{
	let mut y = y;
	let ui_color = ui::ui_color();

	let m = state.m;
	let lh = state.ui_font.get_line_height() as f32;

	state.core.draw_text(
		&state.ui_font,
		ui_color,
		x,
		y - lh / 2. - m * 5.,
		FontAlign::Centre,
		&format!("Crew Level: {}", ship_state.level),
	);

	state.core.draw_text(
		&state.ui_font,
		ui_color,
		x - m * 4.,
		y - lh / 2. - m * 3.5,
		FontAlign::Left,
		"Armor:",
	);
	for (i, (armor, armor_max)) in ship_state.armor.iter().zip(stats.armor.iter()).enumerate()
	{
		let theta = -PI / 2. + i as f32 * PI / 2.;

		let r = m * 1.5;
		let cx = x;

		let lx = cx + (m * 1.5 + r) * theta.cos();
		let ly = y + (m + r) * theta.sin();

		let f = armor / armor_max;
		state.prim.draw_arc(
			cx,
			y,
			r,
			theta - PI / 4. + 0.1,
			PI / 2. - 0.2,
			frac_to_color(f),
			(f * 10.).ceil(),
		);

		state.core.draw_text(
			&state.ui_font,
			frac_to_color(f),
			lx,
			ly - lh / 2.,
			FontAlign::Centre,
			&format!("{}", *armor as i32),
		);
	}

	y += m * 4.;

	let h = m;
	state.core.draw_text(
		&state.ui_font,
		frac_to_color(ship_state.hull / stats.hull),
		x - m * 4.,
		y - lh / 2.,
		FontAlign::Left,
		&format!("Hull: {}", ship_state.hull as i32),
	);
	y += h;
	state.core.draw_text(
		&state.ui_font,
		frac_to_color(ship_state.crew as f32 / stats.crew as f32),
		x - m * 4.,
		y - lh / 2.,
		FontAlign::Left,
		&format!("Crew: {} H / {} W", ship_state.crew, ship_state.wounded),
	);
	y += h;
	state.core.draw_text(
		&state.ui_font,
		frac_to_color(ship_state.infirmary / stats.infirmary),
		x - m * 4.,
		y - lh / 2.,
		FontAlign::Left,
		&format!("Infirmary: {}", ship_state.infirmary as i32),
	);
	y += h;
	state.core.draw_text(
		&state.ui_font,
		frac_to_color(ship_state.sails / stats.sails),
		x - m * 4.,
		y - lh / 2.,
		FontAlign::Left,
		&format!("Sails: {}", ship_state.sails as i32),
	);
}

fn make_wisp(
	pos: Point3<f32>, vel: Vector3<f32>, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let mesh = "data/wisp.glb";
	game_state::cache_mesh(state, mesh)?;
	let res = world.spawn((
		comps::Position {
			pos: pos + Vector3::new(0., 2., 0.),
			dir: 0.,
		},
		comps::Velocity {
			vel: vel,
			dir_vel: PI,
		},
		comps::Mesh { mesh: mesh.into() },
		comps::Lights {
			lights: vec![comps::Light {
				pos: Point3::origin(),
				color: Color::from_rgb_f(0.2, 0.9, 0.9),
				intensity: 3.,
			}],
		},
	));
	Ok(res)
}

fn make_target(
	pos: Point3<f32>, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let mesh = "data/target.glb";
	game_state::cache_mesh(state, mesh)?;
	let res = world.spawn((
		comps::Position {
			pos: pos + Vector3::new(0., 1., 0.),
			dir: 0.,
		},
		comps::Velocity {
			vel: Vector3::zeros(),
			dir_vel: PI,
		},
		comps::Mesh { mesh: mesh.into() },
		comps::Lights {
			lights: vec![comps::Light {
				pos: Point3::origin(),
				color: Color::from_rgb_f(0.2, 0.8, 0.2),
				intensity: 4.,
			}],
		},
	));
	Ok(res)
}

fn make_selection(
	pos: Point3<f32>, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let mesh = "data/selection_indicator.glb";
	game_state::cache_mesh(state, mesh)?;
	let res = world.spawn((
		comps::Position { pos: pos, dir: 0. },
		comps::Velocity {
			vel: Vector3::zeros(),
			dir_vel: PI,
		},
		comps::Mesh { mesh: mesh.into() },
	));
	Ok(res)
}

fn make_projectile(
	pos: Point3<f32>, dir: Vector3<f32>, parent: hecs::Entity, team: comps::Team,
	weapon_stats: &comps::WeaponStats, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let mesh = "data/cannon_ball.glb";
	game_state::cache_mesh(state, mesh)?;
	let res = world.spawn((
		comps::Position { pos: pos, dir: 0. },
		comps::Velocity {
			vel: dir * weapon_stats.speed,
			dir_vel: 5. * PI,
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
						weapon_stats: weapon_stats.clone(),
						team: team,
					},
				},
			],
		},
		comps::Lights {
			lights: vec![comps::Light {
				pos: Point3::origin(),
				color: Color::from_rgb_f(1., 0.8, 0.2),
				intensity: 2.,
			}],
		},
	));
	Ok(res)
}

fn make_muzzle_flash(
	pos: Point3<f32>, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let res = world.spawn((
		comps::Position { pos: pos, dir: 0. },
		comps::TimeToDie {
			time_to_die: state.time() + 0.2,
		},
		comps::Lights {
			lights: vec![comps::Light {
				pos: Point3::origin(),
				color: Color::from_rgb_f(1., 0.8, 0.2),
				intensity: 4.,
			}],
		},
	));
	Ok(res)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SlotDesc
{
	pos: [f32; 2],
	dir: Option<f32>,
	weapons_allowed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct LightDesc
{
	pos: [f32; 3],
	color: [f32; 3],
	intensity: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ShipDesc
{
	mesh: String,
	slots: Vec<SlotDesc>,
	lights: Vec<LightDesc>,
	stats: comps::ShipStats,
	inventory_size: i32,
	size: f32,
}

fn make_ship(
	pos: Point3<f32>, ship_desc: &str, team: comps::Team, level: i32, rng: &mut impl Rng,
	world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let ship_desc: ShipDesc = utils::load_config(ship_desc)?;
	game_state::cache_mesh(state, &ship_desc.mesh)?;

	let mut stats = ship_desc.stats.clone();
	stats.dir_speed *= PI;
	stats.scale_to_level(level);

	let mut slots = vec![];
	for slot_desc in &ship_desc.slots
	{
		slots.push(comps::ItemSlot {
			pos: Point2::new(slot_desc.pos[0], slot_desc.pos[1]),
			dir: slot_desc.dir.map(|d| d * PI),
			item: if slot_desc.weapons_allowed
			{
				Some(comps::generate_weapon(level, rng).clone())
			}
			else
			{
				None
			},
			is_inventory: false,
			weapons_allowed: slot_desc.weapons_allowed,
		});
	}

	let mut lights = vec![];
	for light_desc in &ship_desc.lights
	{
		lights.push(comps::Light {
			pos: Point3::new(light_desc.pos[0], light_desc.pos[1], light_desc.pos[2]),
			color: Color::from_rgb_f(
				light_desc.color[0],
				light_desc.color[1],
				light_desc.color[2],
			),
			intensity: light_desc.intensity,
		});
	}

	let mut equipment =
		comps::Equipment::new(ship_desc.inventory_size.max(0) as usize, true, slots);

	for slot in &mut equipment.slots
	{
		if !slot.is_inventory
		{
			continue;
		}
		if rng.gen_bool(0.5)
		{
			slot.item = Some(comps::generate_item(level, rng));
		}
	}

	let res = world.spawn((
		comps::Position {
			pos: pos,
			dir: PI / 2.,
		},
		comps::Velocity {
			vel: Vector3::zeros(),
			dir_vel: 0.,
		},
		comps::Mesh {
			mesh: ship_desc.mesh.clone(),
		},
		comps::Target { waypoints: vec![] },
		stats.clone(),
		comps::Solid {
			size: ship_desc.size,
			mass: ship_desc.size.powf(3.),
			kind: comps::CollideKind::Big,
			parent: None,
		},
		equipment,
		comps::ShipState::new(&stats, team, level),
		comps::Tilt {
			tilt: 0.,
			target_tilt: 0.,
		},
		comps::Lights { lights: lights },
	));
	Ok(res)
}

#[derive(Copy, Clone, Debug)]
struct CollisionEntry
{
	entity: hecs::Entity,
	pos: Point3<f32>,
}

fn update_economy(economy: &mut [f32; 5], rng: &mut impl Rng) -> (usize, bool)
{
	let idx = rng.gen_range(0..economy.len());

	let dir = *([-1., 1.].choose(rng).unwrap());
	economy[idx] *= (1.25_f32).powf(dir);
	let mut cur_sum = 0.;
	for e in economy.iter()
	{
		cur_sum += *e;
	}
	for e in economy.iter_mut()
	{
		*e *= 1000. / cur_sum;
	}
	(idx, dir > 0.)
}

fn round_price(price: f32) -> i32
{
	((price / 10.) as i32) * 10
}

struct Map
{
	world: hecs::World,
	rng: StdRng,
	player: hecs::Entity,
	player_pos: Point3<f32>,
	zoom: f32,
	target_entity: Option<hecs::Entity>,
	dock_entity: Option<hecs::Entity>,
	selection_indicator: Option<hecs::Entity>,
	buffer_width: f32,
	buffer_height: f32,
	mouse_in_buffer: bool,
	cells: Vec<Cell>,
	money: i32,
	messages: Vec<(String, f64)>,
	level: i32,
	global_offset: Vector2<i32>,
	economy: [f32; 5],
	time_to_economy: f64,
	boss: Option<hecs::Entity>,
	spawn_boss: bool,
	start_time: f64,
}

impl Map
{
	fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		let mut rng = StdRng::seed_from_u64(thread_rng().gen::<u16>() as u64);
		let mut world = hecs::World::new();

		let player = make_ship(
			Point3::new(0., 0., 0.),
			"data/small_ship.cfg",
			comps::Team::English,
			2,
			&mut rng,
			&mut world,
			state,
		)?;
		{
			//let mut ship_state = world.get::<&mut comps::ShipState>(player).unwrap();
			//ship_state.hull = 10.;
			//ship_state.crew = 1;
			//ship_state.wounded = 0;
			//ship_state.infirmary = 0.;
			//ship_state.sails = 30.;
			//ship_state.armor[0] = 50.;
			//ship_state.armor[1] = 0.;
			//ship_state.experience = comps::level_experience(10);
			//ship_state.compute_level();
		}

		let mut cells = vec![];
		for y in -CELL_RADIUS..=CELL_RADIUS
		{
			for x in -CELL_RADIUS..=CELL_RADIUS
			{
				cells.push(Cell::new(
					Point2::new(x, y),
					1,
					&mut rng,
					&mut world,
					state,
				)?);
			}
		}

		state.cache_bitmap("data/english_flag.png")?;
		state.cache_bitmap("data/pirate_flag.png")?;
		state.cache_bitmap("data/french_flag.png")?;
		state.cache_sprite("data/cannon_normal.cfg")?;
		state.cache_sprite("data/cannon_magic.cfg")?;
		state.cache_sprite("data/goods.cfg")?;
		state.cache_sprite("data/cotton.cfg")?;
		state.cache_sprite("data/tobacco.cfg")?;
		state.cache_sprite("data/officer.cfg")?;
		state.cache_sprite("data/cannon_rare.cfg")?;
		state.cache_sprite("data/repair.cfg")?;
		state.cache_sprite("data/switch.cfg")?;
		state.cache_sprite("data/recruit.cfg")?;
		state.sfx.cache_sample("data/order.ogg")?;
		state.sfx.cache_sample("data/equipment.ogg")?;
		state.sfx.cache_sample("data/cannon_shot.ogg")?;
		state.sfx.cache_sample("data/screams.ogg")?;
		state.sfx.cache_sample("data/sink.ogg")?;
		state.sfx.cache_sample("data/explosion.ogg")?;
		game_state::cache_mesh(state, "data/sphere.glb")?;

		let mut economy = [0.; 5];

		for e in &mut economy
		{
			*e = rng.gen_range(100.0..200.0);
		}
		update_economy(&mut economy, &mut rng);

		Ok(Self {
			world: world,
			rng: rng,
			player_pos: Point3::new(0., 0., 0.),
			player: player,
			target_entity: None,
			selection_indicator: None,
			buffer_width: state.display_width,
			buffer_height: state.display_height,
			mouse_in_buffer: true,
			dock_entity: None,
			cells: cells,
			zoom: 1.,
			money: 500,
			messages: vec![
				("Transcend the Sea".into(), state.time()),
				("Hunt the Voidwind".into(), state.time()),
				("Sail North".into(), state.time()),
			],
			level: 1,
			global_offset: Vector2::new(0, 0),
			economy: economy,
			time_to_economy: state.time() + ECONOMY_INTERVAL,
			boss: None,
			start_time: state.time(),
			spawn_boss: true,
		})
	}

	fn make_project(&self) -> Perspective3<f32>
	{
		utils::projection_transform(self.buffer_width, self.buffer_height, PI / 2.)
	}

	fn camera_pos(&self) -> Point3<f32>
	{
		let height = 30. / self.zoom;
		self.player_pos + Vector3::new(0., height, height / 2.)
	}

	fn make_camera(&self) -> Isometry3<f32>
	{
		utils::make_camera(self.camera_pos(), self.player_pos)
	}

	fn add_message(&mut self, message: String, state: &game_state::GameState)
	{
		self.messages.push((message, state.time()));
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

		// Messages
		self.messages
			.retain(|(_, t)| state.time() - t < MESSAGE_DURATION as f64);

		if state.time() > self.time_to_economy
		{
			let (idx, increased) = update_economy(&mut self.economy, &mut self.rng);

			let name = match idx
			{
				0 => "Weapon",
				1 => "Goods",
				2 => "Cotton",
				3 => "Tobacco",
				4 => "Officer",
				_ => unreachable!(),
			};

			let message = if increased
			{
				format!(
					"{name} markets are going up! Now at £{}",
					round_price(self.economy[idx])
				)
			}
			else
			{
				format!(
					"{name} markets are falling down! Now at £{}",
					round_price(self.economy[idx])
				)
			};

			self.messages.push((message, state.time()));

			self.time_to_economy = state.time() + ECONOMY_INTERVAL;
		}

		let mut timer = Timer::new("cell changes", state);
		// Cell changes
		let mut new_cell_centers = vec![];
		let player_cell = Cell::world_to_cell(&self.player_pos);
		let mut good_cells = vec![];

		for cell in &self.cells
		{
			let disp = cell.center - player_cell;
			if disp.x.abs() <= CELL_RADIUS && disp.y.abs() <= CELL_RADIUS
			{
				good_cells.push(cell.clone());
			}
		}
		self.cells.clear();

		for (id, position) in self.world.query::<&comps::Position>().iter()
		{
			let cell = Cell::world_to_cell(&position.pos);
			let disp = cell - player_cell;
			if disp.x.abs() > CELL_RADIUS || disp.y.abs() > CELL_RADIUS
			{
				to_die.push(id);
			}
		}

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
					//println!("New cell {}", cell_center);
				}
			}
		}

		new_cell_centers.shuffle(&mut self.rng);

		if self.spawn_boss && !new_cell_centers.is_empty()
		{
			if let Some(boss) = self.boss
			{
				if !self.world.contains(boss)
				{
					self.boss = None;
				}
			}
			if self.boss.is_none()
			{
				println!("Spawned boss");
				let boss = make_ship(
					Cell::cell_to_world(new_cell_centers[0]),
					//self.player_pos + Vector3::new(0., 0., 100.),
					"data/boss_ship.cfg",
					comps::Team::Pirate,
					(-self.global_offset.y + 10).max(15),
					&mut self.rng,
					&mut self.world,
					state,
				)?;
				self.world.insert(
					boss,
					(
						comps::AI {
							state: comps::AIState::Idle,
							name: "Voidwind".into(),
						},
						comps::WispSpawner {
							time_to_spawn: state.time(),
						},
					),
				)?;
				{
					let mut ship_state = self.world.get::<&mut comps::ShipState>(boss).unwrap();
					ship_state.is_boss = true;
				}
				self.boss = Some(boss);
			}
		}

		for cell_center in new_cell_centers
		{
			let level = (-(cell_center.y + self.global_offset.y)).max(1);
			//println!("LEVEL {} {:?}", level, self.global_offset);
			self.cells.push(Cell::new(
				cell_center,
				level,
				&mut self.rng,
				&mut self.world,
				state,
			)?);
		}

		// Recenter.
		let cell_offt = player_cell.coords;
		self.global_offset += cell_offt;
		for cell in &mut self.cells
		{
			cell.center -= player_cell.coords;
		}
		let offt = Cell::cell_to_world(player_cell).coords;
		for (_, pos) in self.world.query::<&mut comps::Position>().iter()
		{
			pos.pos -= offt;
		}
		for (_, target) in self.world.query::<&mut comps::Target>().iter()
		{
			for waypoint in &mut target.waypoints
			{
				waypoint.pos -= offt;
			}
		}
		for (_, equipment) in self.world.query::<&mut comps::Equipment>().iter()
		{
			equipment.target_pos -= offt;
		}
		self.player_pos -= offt;
		if offt.magnitude() > 0.0
		{
			dbg!("recentered");
		}
		timer.record(&state.core);

		let mut timer = Timer::new("physics", state);
		// Collision.
		let center = self.player_pos.zx();
		let mut grid = spatial_grid::SpatialGrid::new(128, 128, 8.0, 8.0);
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
		timer.record(&state.core);

		let mut timer = Timer::new("physics", state);
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
		timer.record(&state.core);

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

		let mut timer = Timer::new("ship_state", state);
		// Ship state simulation.
		let mut num_ships = 0;
		for (_, (ship_state, stats, equipment)) in self
			.world
			.query::<(
				&mut comps::ShipState,
				&comps::ShipStats,
				&mut comps::Equipment,
			)>()
			.iter()
		{
			num_ships += 1;
			if !ship_state.is_structurally_sound()
			{
				// Can't fix a broken ship.
				continue;
			}
			let derived_stats = equipment.derived_stats();

			ship_state.compute_level();

			let effective_crew =
				ship_state.crew as f32 * comps::level_effectiveness(ship_state.level);

			// Each crew member can repair 0.1 point per 1 second, probabilistically
			let repair_prob = dt as f64;
			let num_repaired =
				rand_distr::Binomial::new((effective_crew.sqrt() * 0.5).ceil() as u64, repair_prob)
					.unwrap()
					.sample(&mut self.rng);

			let mut parts = [
				stats.armor[0] - ship_state.armor[0],
				stats.armor[1] - ship_state.armor[1],
				stats.armor[2] - ship_state.armor[2],
				stats.armor[3] - ship_state.armor[3],
				10. * (stats.hull - ship_state.hull), // No hull, no ship.
				stats.infirmary - ship_state.infirmary,
				stats.sails - ship_state.sails,
			];
			for i in &ship_state.repair_boost
			{
				parts[*i] *= 5.;
			}
			if let Ok(dist) = rand_distr::WeightedIndex::new(&parts)
			{
				let to_repair = dist.sample(&mut self.rng);
				let num_repaired = num_repaired as f32;
				match to_repair
				{
					0 =>
					{
						ship_state.armor[0] = (ship_state.armor[0]
							+ num_repaired * (1. + derived_stats.armor_repair))
							.min(stats.armor[0])
					}
					1 =>
					{
						ship_state.armor[1] = (ship_state.armor[1]
							+ num_repaired * (1. + derived_stats.armor_repair))
							.min(stats.armor[1])
					}
					2 =>
					{
						ship_state.armor[2] = (ship_state.armor[2]
							+ num_repaired * (1. + derived_stats.armor_repair))
							.min(stats.armor[2])
					}
					3 =>
					{
						ship_state.armor[3] = (ship_state.armor[3]
							+ num_repaired * (1. + derived_stats.armor_repair))
							.min(stats.armor[3])
					}
					4 =>
					{
						ship_state.hull = (ship_state.hull
							+ num_repaired * (1. + derived_stats.hull_repair))
							.min(stats.hull)
					}
					5 =>
					{
						ship_state.infirmary = (ship_state.infirmary
							+ num_repaired * (1. + derived_stats.infirmary_repair))
							.min(stats.infirmary)
					}
					6 =>
					{
						ship_state.sails = (ship_state.sails
							+ num_repaired * (1. + derived_stats.sail_repair))
							.min(stats.sails)
					}
					_ => unreachable!(),
				}
			}

			// Each patient has a chance of getting better, weighed by infirmary strength... I
			// guess it has more drugs?
			let heal_prob = (dt as f32 * ship_state.infirmary.sqrt()
				/ 100.0 / ship_state.wounded as f32
				* (1. + derived_stats.medic))
				.min(1.);
			for _ in 0..ship_state.wounded
			{
				if self.rng.gen_bool(heal_prob as f64)
				{
					ship_state.wounded -= 1;
					ship_state.crew += 1;
				}
			}

			// Weapon handling.
			let mut num_weapons = 0;
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
							if weapon.readiness < 1.
							{
								num_weapons += 1;
							}
						}
						_ => (),
					}
				}
			}

			// X crew per weapon to reload it effectively.
			let crew_per_weapon = 10;
			let fire_rate_adjustment = 1. / crew_per_weapon as f32 * effective_crew.sqrt()
				/ num_weapons as f32
				* (1. + derived_stats.reload_speed);
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
							weapon.readiness = (weapon.readiness
								+ dt * (fire_rate_adjustment / weapon.stats().fire_interval))
								.min(1.0);
						}
						_ => (),
					}
				}
			}
		}
		timer.record(&state.core);
		if state.tick % 64 == 0
		{
			//println!("Num ships: {}", num_ships);
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

		let mut timer = Timer::new("collision", state);
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
		if state.tick % 64 == 0
		{
			//println!("Colliding pairs: {}", colliding_pairs.len());
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
					for (id, other_id, pos, other_pos) in
						[(id1, id2, pos1, pos2), (id2, id1, pos2, pos1)]
					{
						if let Ok(on_contact_effect) = self.world.get::<&comps::OnContactEffect>(id)
						{
							on_contact_effects.push((
								id,
								other_id,
								pos,
								other_pos,
								on_contact_effect.effects.clone(),
							));
						}
					}
				}
			}
		}

		// On contact effects.
		for (id, other_id, pos, other_pos, effects) in on_contact_effects
		{
			for effect in effects
			{
				match (effect, other_id)
				{
					(comps::ContactEffect::Die, _) => to_die.push(id),
					(comps::ContactEffect::Hurt { damage }, other_id) =>
					{
						let mut damage_report = None;
						let mut disabled = None;
						let mut destroyed = false;
						if let (Ok(mut ship_state), Ok(ship_stats)) = (
							self.world.get::<&mut comps::ShipState>(other_id),
							self.world.get::<&comps::ShipStats>(other_id),
						)
						{
							let was_active = ship_state.is_active();
							let was_sound = ship_state.is_structurally_sound();
							let had_crew = ship_state.has_crew();
							let report = ship_state.damage(
								&damage,
								(pos - other_pos).normalize(),
								&mut self.rng,
							);
							if report.damaged
							{
								state.sfx.play_positional_sound(
									"data/explosion.ogg",
									pos.xz(),
									self.player_pos.xz(),
									0.5,
								)?;
							}
							if report.damaged && was_active != ship_state.is_active()
							{
								disabled = Some((ship_state.level, ship_stats.exp_bonus));
								destroyed = !ship_state.is_structurally_sound();
							}
							if report.damaged && had_crew != ship_state.has_crew()
							{
								state.sfx.play_positional_sound(
									"data/screams.ogg",
									pos.xz(),
									self.player_pos.xz(),
									0.5,
								)?;
							}
							if report.damaged && was_sound != ship_state.is_structurally_sound()
							{
								state.sfx.play_positional_sound(
									"data/sink.ogg",
									pos.xz(),
									self.player_pos.xz(),
									0.5,
								)?;
							}
							damage_report = Some(report);
						}
						if let Some(report) = damage_report
						{
							if let Ok(mut ai) = self.world.get::<&mut comps::AI>(other_id)
							{
								if let Some(parent_id) = self
									.world
									.get::<&comps::Solid>(id)
									.ok()
									.and_then(|s| s.parent)
								{
									ai.state = comps::AIState::Pursuing(parent_id);
								}
							}

							let destroy_prob = if destroyed
							{
								0.75
							}
							else
							{
								report.item_destroy_chance
							};
							if let Ok(mut equipment) =
								self.world.get::<&mut comps::Equipment>(other_id)
							{
								let derived_stats = equipment.derived_stats();
								for slot in &mut equipment.slots
								{
									if self.rng.gen_bool(
										(destroy_prob / (1. + derived_stats.item_protect)) as f64,
									)
									{
										//println!("Destroyed {:?}", slot.item);
										if !destroyed && other_id == self.player
										{
											if let Some(item) = slot.item.as_ref()
											{
												self.messages.push((
													format!("{} destroyed!", item.kind.name()),
													state.time(),
												));
											}
										}
										slot.item = None;
									}
									if disabled.is_some()
									{
										// Officers die.
										if let Some(item) = slot.item.as_ref()
										{
											if let comps::ItemKind::Officer(_) = item.kind
											{
												slot.item = None;
											}
										}
										if Some(other_id) == self.boss
										{
											self.messages
												.push(("You are victorious!".into(), state.time()));
											self.messages.push((format!("Voidwind has been defeated after {:.1} minutes!", (state.time() - self.start_time) / 60.), state.time()));
											self.spawn_boss = false;
											self.boss = None;
										}
									}
								}
							}
						}
						if let Some((level, exp_bonus)) = disabled
						{
							let parent_id = self
								.world
								.get::<&comps::Solid>(id)
								.ok()
								.and_then(|s| s.parent);
							if let Some(mut ship_state) = parent_id
								.and_then(|id| self.world.get::<&mut comps::ShipState>(id).ok())
							{
								ship_state.experience += exp_bonus * comps::enemy_experience(level);
								//dbg!(ship_state.experience);
								let old_level = ship_state.level;
								ship_state.compute_level();
								if old_level != ship_state.level && parent_id == Some(self.player)
								{
									self.messages
										.push(("Crew got more experienced!".into(), state.time()));
								}
							}
						}
					}
				}
			}
		}
		timer.record(&state.core);

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
		let want_attack = state.controls.get_action_state(controls::Action::Attack) > 0.5;
		let want_zoom_in = state.controls.get_action_state(controls::Action::ZoomIn) > 0.5;
		let want_zoom_out = state.controls.get_action_state(controls::Action::ZoomOut) > 0.5;
		let want_target = state.controls.get_action_state(controls::Action::Target) > 0.5;

		let mouse_in_buffer = self.mouse_in_buffer;
		let mouse_ground_pos = self.get_mouse_ground_pos(state);
		if mouse_in_buffer && (want_dock || want_attack || want_target)
		{
			let d = 1.;
			let mouse_entries = grid.query_rect(
				mouse_ground_pos.zx() - Vector2::new(d, d) - center.coords,
				mouse_ground_pos.zx() + Vector2::new(d, d) - center.coords,
				|_| true,
			);

			if let Some(entry) = mouse_entries.first()
			{
				if entry.inner.entity != self.player
				{
					if let (Ok(pos), Ok(solid), Ok(_)) = (
						self.world.get::<&comps::Position>(entry.inner.entity),
						self.world.get::<&comps::Solid>(entry.inner.entity),
						self.world.get::<&comps::ShipState>(entry.inner.entity),
					)
					{
						if (pos.pos - mouse_ground_pos).magnitude() < 1.5 * solid.size
						{
							self.target_entity = Some(entry.inner.entity);
						}
					}
				}
			}
		}

		if want_move && mouse_in_buffer && player_alive
		{
			//state.sfx.play_sound("data/order.ogg").unwrap();
			state.controls.clear_action_state(controls::Action::Move);
			self.dock_entity = None;
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
				to_die.push(marker);
			}
		}
		if want_stop && player_alive
		{
			state.sfx.play_sound("data/order.ogg").unwrap();
			state.controls.clear_action_state(controls::Action::Stop);
			if let Ok(mut target) = self.world.get::<&mut comps::Target>(self.player)
			{
				target.clear(|m| to_die.push(m));
			}
		}
		if want_attack && mouse_in_buffer && player_alive
		{
			if let Ok(mut equipment) = self.world.get::<&mut comps::Equipment>(self.player)
			{
				equipment.want_attack = true;
				equipment.target_pos = mouse_ground_pos;
			}
		}
		if !want_attack
		{
			if let Ok(mut equipment) = self.world.get::<&mut comps::Equipment>(self.player)
			{
				equipment.want_attack = false;
			}
		}
		if want_dock && player_alive && self.target_entity != Some(self.player)
		{
			state.controls.clear_action_state(controls::Action::Dock);
			self.dock_entity = None;
			if let Some(target_entity) = self.target_entity
			{
				let mut move_to = None;
				let mut do_trade = false;
				if let (
					Ok(player_pos),
					Ok(mut player_target),
					Ok(player_ship_state),
					Ok(player_solid),
					Ok(pos),
					Ok(_),
					Ok(ship_state),
					Ok(solid),
				) = (
					self.world.get::<&comps::Position>(self.player),
					self.world.get::<&mut comps::Target>(self.player),
					self.world.get::<&comps::ShipState>(self.player),
					self.world.get::<&comps::Solid>(self.player),
					self.world.get::<&comps::Position>(target_entity),
					self.world.get::<&comps::Equipment>(target_entity),
					self.world.get::<&comps::ShipState>(target_entity),
					self.world.get::<&comps::Solid>(target_entity),
				)
				{
					if ship_state.team.dock_with(&player_ship_state.team)
					{
						state.sfx.play_sound("data/order.ogg").unwrap();
						if (player_pos.pos.zx() - pos.pos.zx()).magnitude()
							< 2.0 + solid.size + player_solid.size
						{
							player_target.clear(|m| to_die.push(m));
							self.dock_entity = Some(target_entity);
							do_trade = ship_state.team.trade_with(&player_ship_state.team);
						}
						else
						{
							move_to = Some(player_pos.pos);
						}
					}
				}
				if let (Ok(mut ai), Ok(mut target), Some(move_to)) = (
					self.world.get::<&mut comps::AI>(target_entity),
					self.world.get::<&mut comps::Target>(target_entity),
					move_to,
				)
				{
					ai.state = comps::AIState::Pause {
						time_to_unpause: state.time() + 1.,
					};
					target.clear(|m| to_die.push(m));
					target.waypoints.push(comps::Waypoint {
						pos: move_to,
						marker: None,
					});
				}
				if do_trade
				{
					for entity in [self.player, self.target_entity.unwrap()]
					{
						if let Ok(mut equipment) = self.world.get::<&mut comps::Equipment>(entity)
						{
							for slot in &mut equipment.slots
							{
								if let Some(item) = slot.item.as_mut()
								{
									match &mut item.kind
									{
										comps::ItemKind::Weapon(weapon) =>
										{
											item.price = round_price(
												comps::level_effectiveness(weapon.level)
													* (1 + weapon.prefixes.len()
														+ weapon.suffixes.len()) as f32 * self.economy
													[Price::Weapon as usize],
											)
										}
										comps::ItemKind::Officer(officer) =>
										{
											item.price = round_price(
												comps::level_effectiveness(officer.level)
													* (1 + officer.prefixes.len()
														+ officer.suffixes.len()) as f32 * self.economy
													[Price::Officer as usize],
											)
										}
										comps::ItemKind::Goods(level) =>
										{
											item.price = round_price(
												comps::level_effectiveness(*level)
													* self.economy[Price::Goods as usize],
											)
										}
										comps::ItemKind::Tobacco(level) =>
										{
											item.price = round_price(
												comps::level_effectiveness(*level)
													* self.economy[Price::Tobacco as usize],
											)
										}
										comps::ItemKind::Cotton(level) =>
										{
											item.price = round_price(
												comps::level_effectiveness(*level)
													* self.economy[Price::Cotton as usize],
											)
										}
									}
								}
							}
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

		let mut timer = Timer::new("equipment actions", state);
		// Equipment actions
		let mut spawn_projectiles = vec![];
		for (id, (pos, equipment, ship_state)) in self
			.world
			.query::<(&comps::Position, &mut comps::Equipment, &comps::ShipState)>()
			.iter()
		{
			let derived_stats = equipment.derived_stats();
			// No buffering
			let want_attack = equipment.want_attack;
			//equipment.want_attack = false;
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
							if weapon.readiness >= 1.0
								&& want_attack && weapon.time_to_fire.is_none()
							{
								weapon.time_to_fire =
									Some(state.time() + self.rng.gen_range(0.0..0.2));
							}
							if weapon
								.time_to_fire
								.map(|ttf| state.time() > ttf)
								.unwrap_or(false)
							{
								weapon.time_to_fire = None;
								let rot = Rotation2::new(pos.dir);
								let slot_dir = slot.dir.unwrap_or(0.);
								let rot_slot = Rotation2::new(slot_dir);
								let slot_pos = pos.pos.zx() + rot * slot.pos.coords;
								let slot_dir_vec = rot_slot * rot * Vector2::new(1., 0.);
								let target_dir = (equipment.target_pos.zx() - slot_pos).normalize();
								let weapon_stats = weapon.stats();
								let arc = weapon_stats.arc;
								let min_dot = (arc / 2.).cos();
								let min_dot_2 = (2. * arc / 2.).cos();

								let spawn_pos = Point3::new(slot_pos.y, 3., slot_pos.x);
								let mut spawn_dir = None;
								if slot_dir_vec.dot(&target_dir) > min_dot
								{
									spawn_dir = Some(target_dir);
								}
								else if slot_dir_vec.dot(&target_dir) > min_dot_2
									&& equipment.allow_out_of_arc_shots
								{
									let cand_dir1 = Rotation2::new(slot_dir + arc / 2.)
										* rot * Vector2::new(1., 0.);
									let cand_dir2 = Rotation2::new(slot_dir - arc / 2.)
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
									let f = 1. + derived_stats.accuracy;
									let rot = Rotation2::new(self.rng.gen_range(
										-weapon_stats.spread / f..=weapon_stats.spread / f,
									));
									let spawn_dir = rot * spawn_dir;
									let spawn_dir =
										Vector3::new(spawn_dir.y, 0.5, spawn_dir.x).normalize();
									let mut weapon_stats = weapon.stats().clone();
									weapon_stats.critical_chance *=
										1. + derived_stats.critical_chance;
									spawn_projectiles.push((
										spawn_pos,
										spawn_dir,
										id,
										ship_state.team,
										weapon_stats,
									));
									state.sfx.play_positional_sound(
										"data/cannon_shot.ogg",
										spawn_pos.xz(),
										self.player_pos.xz(),
										0.5,
									)?;
									weapon.readiness = 0.;
								}
							}
						}
						_ => (),
					}
				}
			}
		}

		for (spawn_pos, spawn_dir, parent, team, stats) in spawn_projectiles
		{
			make_muzzle_flash(spawn_pos, &mut self.world, state)?;
			make_projectile(
				spawn_pos,
				spawn_dir,
				parent,
				team,
				&stats,
				&mut self.world,
				state,
			)?;
		}
		timer.record(&state.core);

		let mut spawn_wisps = vec![];
		for (_, (pos, wisp_spawner)) in self
			.world
			.query::<(&comps::Position, &mut comps::WispSpawner)>()
			.iter()
		{
			if state.time() > wisp_spawner.time_to_spawn
			{
				spawn_wisps.push(pos.pos);
				wisp_spawner.time_to_spawn = state.time() + 4.;
			}
		}
		for pos in spawn_wisps
		{
			let dir = self.rng.gen_range(0.0..PI * 2.0);
			let vel = Vector3::new(dir.cos(), 0., dir.sin()) * 5.;
			make_wisp(pos, vel, &mut self.world, state)?;
		}

		// Update player pos.
		if let Ok(pos) = self.world.get::<&comps::Position>(self.player)
		{
			self.player_pos = pos.pos;
		}

		// Target movement.
		for (_, (target, pos, vel, ship_state, stats, equipment)) in self
			.world
			.query::<(
				&mut comps::Target,
				&comps::Position,
				&mut comps::Velocity,
				&comps::ShipState,
				&comps::ShipStats,
				&comps::Equipment,
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

			let speed_factor = 0.1
				+ 0.9 * (ship_state.sails / stats.sails) * (1. + equipment.derived_stats().speed);

			let dot = diff.dot(&left);
			if dot > 0.05
			{
				vel.dir_vel =
					speed_factor * stats.dir_speed * comps::level_effectiveness(ship_state.level);
			}
			else if dot < -0.05
			{
				vel.dir_vel = speed_factor * -stats.dir_speed;
			}
			else
			{
				vel.dir_vel = 0.;
			}
			vel.vel = speed_factor * stats.speed * Vector3::new(forward.y, 0., forward.x);
		}

		// AI
		let mut timer = Timer::new("ai", state);
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
			let sense_radius = 40.;
			let attack_radius = 20.;
			if Some(id) == self.dock_entity
			{
				target.clear(|m| to_die.push(m));
				continue;
			}
			match ai.state
			{
				comps::AIState::Pause { time_to_unpause } =>
				{
					if state.time() > time_to_unpause
					{
						ai.state = comps::AIState::Idle;
					}
				}
				comps::AIState::Idle =>
				{
					let mut entries = grid.query_rect(
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
					if ship_state.is_boss
					{
						entries.clear();
					}
					if let Some(entry) = entries.choose(&mut self.rng)
					{
						ai.state = comps::AIState::Pursuing(entry.inner.entity);
					}
					else if target.waypoints.is_empty()
					{
						let cell_id = (0..self.cells.len()).choose(&mut self.rng).unwrap();
						let d = CELL_SIZE as f32 / 2.0;
						let dx = self.rng.gen_range(-d..d);
						let dy = self.rng.gen_range(-d..d);
						target.waypoints.push(comps::Waypoint {
							pos: self.cells[cell_id].world_center() + Vector3::new(dx, 0., dy),
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
							if diff.magnitude() < attack_radius
							{
								target.clear(|m| to_die.push(m));
								ai.state = comps::AIState::Attacking(target_entity);
							}
							else if diff.magnitude() > sense_radius
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
							equipment.want_attack = false;
						}
						else
						{
							let target_pos =
								self.world.get::<&comps::Position>(target_entity).unwrap();
							let diff = target_pos.pos - pos.pos;
							// Too far to shoot.
							if diff.magnitude() > attack_radius
							{
								ai.state = comps::AIState::Pursuing(target_entity);
								equipment.want_attack = false;
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
								equipment.want_attack = true;
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
		timer.record(&state.core);

		// Ship state death
		let mut remove_ai = vec![];
		for (id, (target, ship_state)) in self
			.world
			.query_mut::<(&mut comps::Target, &mut comps::ShipState)>()
		{
			if !ship_state.is_active() && ship_state.team != comps::Team::Neutral
			{
				if id == self.player
				{
					self.messages
						.push(("You've been defeated!".into(), state.time()));
				}
				target.clear(|m| to_die.push(m));
				ship_state.team = comps::Team::Neutral;
				ship_state.crew = 0;
				ship_state.wounded = 0;
				remove_ai.push(id);
			}
		}
		for id in remove_ai
		{
			// Player has no AI.
			self.world.remove_one::<comps::AI>(id).ok();
			if let Ok(mut equipment) = self.world.get::<&mut comps::Equipment>(id)
			{
				equipment.want_attack = false;
			}
		}

		// Selection indicator
		let mut target_pos = None;
		if let Some(pos) = self
			.target_entity
			.and_then(|e| self.world.get::<&comps::Position>(e).ok())
		{
			target_pos = Some(pos.pos);
		}
		else
		{
			if let Some(selection_entity) = self.selection_indicator
			{
				to_die.push(selection_entity);
				self.selection_indicator = None;
			}
		}
		if let Some(target_pos) = target_pos
		{
			let mut make_new = true;
			if let Some(mut pos) = self
				.selection_indicator
				.and_then(|e| self.world.get::<&mut comps::Position>(e).ok())
			{
				pos.pos = target_pos;
				make_new = false;
			}
			if make_new
			{
				self.selection_indicator =
					Some(make_selection(target_pos, &mut self.world, state)?);
			}
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
		if !to_die.is_empty()
		{
			//println!("   Despawned: {}", to_die.len());
		}
		for id in to_die
		{
			//println!("died {id:?}");
			if self.world.contains(id)
			{
				self.world.despawn(id)?;
			}
		}

		Ok(None)
	}

	fn input(
		&mut self, _event: &Event, _state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		Ok(None)
	}

	fn draw(&mut self, state: &game_state::GameState) -> Result<()>
	{
		// Forward pass.

		let project = self.make_project();
		let camera = self.make_camera();
		state
			.core
			.use_projection_transform(&utils::mat4_to_transform(project.to_homogeneous()));
		state
			.core
			.use_transform(&utils::mat4_to_transform(camera.to_homogeneous()));

		state.g_buffer.as_ref().unwrap().bind();
		unsafe {
			gl::Enable(gl::CULL_FACE);
			gl::CullFace(gl::BACK);
		}
		state
			.core
			.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::Zero);
		state.core.set_depth_test(Some(DepthFunction::Less));
		state.core.clear_depth_buffer(1.);
		state.core.clear_to_color(Color::from_rgb_f(0., 0., 0.));

		let shift = Vector3::new(0., -0.01, 0.);
		let tl = utils::get_ground_from_screen(-1.0, 1.0, project, camera) + shift;
		let tr = utils::get_ground_from_screen(1.0, 1.0, project, camera) + shift;
		let bl = utils::get_ground_from_screen(-1.0, -1.0, project, camera) + shift;
		let br = utils::get_ground_from_screen(1.0, -1.0, project, camera) + shift;
		let vtxs = [
			mesh::WaterVertex {
				x: bl.x,
				y: bl.y,
				z: bl.z,
			},
			mesh::WaterVertex {
				x: br.x,
				y: br.y,
				z: br.z,
			},
			mesh::WaterVertex {
				x: tr.x,
				y: tr.y,
				z: tr.z,
			},
			mesh::WaterVertex {
				x: tl.x,
				y: tl.y,
				z: tl.z,
			},
		];
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
			.use_shader(Some(&*state.forward_shader.upgrade().unwrap()))
			.unwrap();

		for (id, (pos, mesh)) in self
			.world
			.query::<(&comps::Position, &comps::Mesh)>()
			.iter()
		{
			let screen_pos =
				(project.to_homogeneous() * camera.to_homogeneous()).transform_point(&pos.pos);
			if screen_pos.x < -1.5
				|| screen_pos.x > 1.5
				|| screen_pos.y < -1.5
				|| screen_pos.y > 1.5
			{
				continue;
			}

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

			let material_mapper =
				|material: &mesh::Material, texture_name: &str| -> Result<&Bitmap> {
					if material.name == "flag_material"
					{
						unsafe {
							gl::Disable(gl::CULL_FACE);
						}
						if let Ok(ship_state) = self.world.get::<&comps::ShipState>(id)
						{
							let texture_name = match ship_state.team
							{
								comps::Team::English => "data/english_flag.png",
								comps::Team::French => "data/french_flag.png",
								comps::Team::Pirate => "data/pirate_flag.png",
								_ => texture_name,
							};
							state.get_bitmap(texture_name)
						}
						else
						{
							state.get_bitmap(texture_name)
						}
					}
					else if material.name == "fire_material"
					{
						unsafe {
							gl::Enable(gl::CULL_FACE);
						}
						state.get_bitmap(texture_name)
					}
					else
					{
						unsafe {
							gl::Disable(gl::CULL_FACE);
						}
						state.get_bitmap(texture_name)
					}
				};

			state
				.get_mesh(&mesh.mesh)
				.unwrap()
				.draw(&state.core, &state.prim, material_mapper) //|s| state.get_bitmap(s));
		}

		// Light pass.
		state.core.set_target_bitmap(state.light_buffer.as_ref());
		state
			.core
			.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::Zero);
		state
			.core
			.clear_to_color(Color::from_rgba_f(0.05, 0.05, 0.05, 0.));
		state
			.core
			.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::One);
		state
			.core
			.use_projection_transform(&utils::mat4_to_transform(project.to_homogeneous()));

		state.core.set_depth_test(None);
		unsafe {
			gl::Enable(gl::CULL_FACE);
			gl::DepthMask(gl::FALSE);
			gl::CullFace(gl::FRONT);
		}

		state
			.core
			.use_shader(Some(&*state.light_shader.upgrade().unwrap()))
			.unwrap();
		state
			.core
			.set_shader_uniform("position_buffer", &[0_i32][..])
			.ok(); //unwrap();
		state
			.core
			.set_shader_uniform("normal_buffer", &[1_i32][..])
			.ok(); //unwrap();
		state
			.core
			.set_shader_uniform(
				"buffer_size",
				&[[self.buffer_width, self.buffer_height]][..],
			)
			.ok(); //.unwrap();
		let camera_pos = self.camera_pos();
		state
			.core
			.set_shader_uniform(
				"camera_pos",
				&[[camera_pos.x, camera_pos.y, camera_pos.z]][..],
			)
			.ok(); //.unwrap();

		let g_buffer = state.g_buffer.as_ref().unwrap();
		unsafe {
			gl::ActiveTexture(gl::TEXTURE0);
			gl::BindTexture(gl::TEXTURE_2D, g_buffer.position_tex);
			gl::ActiveTexture(gl::TEXTURE1);
			gl::BindTexture(gl::TEXTURE_2D, g_buffer.normal_tex);
		}

		for (_, (pos, lights)) in self
			.world
			.query::<(&comps::Position, &comps::Lights)>()
			.iter()
		{
			let common_shift = Isometry3::new(pos.pos.coords, pos.dir * Vector3::y());
			for light in &lights.lights
			{
				let shift = common_shift * Isometry3::new(light.pos.coords, Vector3::zeros());
				let transform = Similarity3::from_isometry(shift, 20. * light.intensity.sqrt());
				let light_pos = transform.transform_point(&Point3::origin());

				let screen_pos = (project.to_homogeneous() * camera.to_homogeneous())
					.transform_point(&light_pos);
				if screen_pos.x < -1.5
					|| screen_pos.x > 1.5
					|| screen_pos.y < -1.5
					|| screen_pos.y > 1.5
				{
					continue;
				}

				let (r, g, b) = light.color.to_rgb_f();

				state
					.core
					.set_shader_uniform("light_color", &[[r, g, b, 1.0]][..])
					.ok(); //.unwrap();
				state
					.core
					.set_shader_uniform("light_pos", &[[light_pos.x, light_pos.y, light_pos.z]][..])
					.ok(); //.unwrap();
				state
					.core
					.set_shader_uniform("light_intensity", &[light.intensity][..])
					.ok(); //.unwrap();

				state.core.use_transform(&utils::mat4_to_transform(
					camera.to_homogeneous() * transform.to_homogeneous(),
				));

				if let Ok(mesh) = state.get_mesh("data/sphere.glb")
				{
					mesh.draw(&state.core, &state.prim, |_, s| state.get_bitmap(s));
				}
			}
		}

		// Final pass.
		let g_buffer = state.g_buffer.as_ref().unwrap();
		state.core.set_target_bitmap(state.buffer.as_ref());
		state.core.clear_to_color(Color::from_rgb_f(0., 0.3, 0.0));
		state.core.set_depth_test(None);
		state
			.core
			.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::Zero);
		// Copy depth buffer.
		unsafe {
			gl::BindFramebuffer(gl::READ_FRAMEBUFFER, g_buffer.frame_buffer);
			gl::BlitFramebuffer(
				0,
				0,
				self.buffer_width as i32,
				self.buffer_height as i32,
				0,
				0,
				self.buffer_width as i32,
				self.buffer_height as i32,
				gl::DEPTH_BUFFER_BIT,
				gl::NEAREST,
			);
		}

		let ortho_mat = Matrix4::new_orthographic(
			0.,
			self.buffer_width as f32,
			self.buffer_height as f32,
			0.,
			-1.,
			1.,
		);
		state
			.core
			.use_projection_transform(&utils::mat4_to_transform(ortho_mat));
		state.core.use_transform(&Transform::identity());

		state
			.core
			.use_shader(Some(&*state.final_shader.upgrade().unwrap()))
			.unwrap();
		state
			.core
			.set_shader_uniform("position_buffer", &[1_i32][..])
			.ok(); //unwrap();
		state
			.core
			.set_shader_uniform("normal_buffer", &[2_i32][..])
			.ok(); //unwrap();
		state
			.core
			.set_shader_uniform("albedo_buffer", &[3_i32][..])
			.ok(); //.unwrap();
	   //state
	   //	.core
	   //	.set_shader_uniform(
	   //		"camera_pos",
	   //		&[[camera_pos[0], camera_pos[1], camera_pos[2]]][..],
	   //	)
	   //	.ok(); //unwrap();
		unsafe {
			gl::Disable(gl::CULL_FACE);
			gl::ActiveTexture(gl::TEXTURE1);
			gl::BindTexture(gl::TEXTURE_2D, g_buffer.position_tex);
			gl::ActiveTexture(gl::TEXTURE2);
			gl::BindTexture(gl::TEXTURE_2D, g_buffer.normal_tex);
			gl::ActiveTexture(gl::TEXTURE3);
			gl::BindTexture(gl::TEXTURE_2D, g_buffer.albedo_tex);
		}
		let vertices = [
			Vertex {
				x: 0.,
				y: 0.,
				z: 0.,
				u: 0.,
				v: 1.,
				color: Color::from_rgb_f(1.0, 1.0, 1.0),
			},
			Vertex {
				x: self.buffer_width,
				y: 0.,
				z: 0.,
				u: 1.,
				v: 1.,
				color: Color::from_rgb_f(1.0, 1.0, 1.0),
			},
			Vertex {
				x: self.buffer_width,
				y: self.buffer_height,
				z: 0.,
				u: 1.,
				v: 0.,
				color: Color::from_rgb_f(1.0, 1.0, 1.0),
			},
			Vertex {
				x: 0.,
				y: self.buffer_height,
				z: 0.,
				u: 0.,
				v: 0.,
				color: Color::from_rgb_f(1.0, 1.0, 1.0),
			},
		];
		state.prim.draw_prim(
			&vertices[..],
			state.light_buffer.as_ref(),
			0,
			4,
			PrimType::TriangleFan,
		);

		Ok(())
	}
}
