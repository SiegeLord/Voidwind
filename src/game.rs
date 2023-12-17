use crate::error::Result;
use crate::{astar, components as comps, controls, game_state, sprite, ui, utils};
use allegro::*;
use allegro_font::*;
use na::{
	Isometry3, Matrix4, Perspective3, Point2, Point3, Quaternion, RealField, Rotation2, Rotation3,
	Unit, Vector2, Vector3, Vector4,
};
use nalgebra as na;
use rand::prelude::*;

use std::collections::HashMap;

pub struct Game
{
	map: Map,
	subscreens: Vec<ui::SubScreen>,
}

impl Game
{
	pub fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		Ok(Self {
			map: Map::new(state)?,
			subscreens: vec![],
		})
	}

	pub fn logic(
		&mut self, state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		if self.subscreens.is_empty()
		{
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
		state.controls.decode_event(event);
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
			let in_game_menu;
			match *event
			{
				Event::KeyDown {
					keycode: KeyCode::Escape,
					..
				} =>
				{
					in_game_menu = true;
				}
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
		if let Some(subscreen) = self.subscreens.last_mut()
		{
			state.core.clear_to_color(Color::from_rgb_f(0.0, 0.0, 0.0));
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
		else
		{
			state.core.clear_to_color(Color::from_rgb_f(0.5, 0.5, 1.));
			self.map.draw(state)?;
		}
		Ok(())
	}
}

struct Map
{
	world: hecs::World,
	rng: StdRng,
}

impl Map
{
	fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		let mut rng = StdRng::seed_from_u64(thread_rng().gen::<u16>() as u64);
		let mut world = hecs::World::new();

		Ok(Self {
			world: world,
			rng: rng,
		})
	}

	fn logic(&mut self, state: &mut game_state::GameState)
		-> Result<Option<game_state::NextScreen>>
	{
		let mut to_die = vec![];

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
		&mut self, event: &Event, state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		Ok(None)
	}

	fn draw(&mut self, state: &game_state::GameState) -> Result<()>
	{
		Ok(())
	}
}
