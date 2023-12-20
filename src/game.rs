use crate::error::Result;
use crate::{astar, components as comps, controls, game_state, sprite, ui, utils};
use allegro::*;
use allegro_font::*;
use allegro_primitives::*;
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

fn make_player(
	pos: Point3<f32>, world: &mut hecs::World, state: &mut game_state::GameState,
) -> Result<hecs::Entity>
{
	let mesh = "data/test.glb";
	game_state::cache_mesh(state, mesh)?;
	let res = world.spawn((
		comps::Position { pos: pos, dir: 0. },
		comps::Velocity {
			vel: Vector3::zeros(),
			dir_vel: 0.,
		},
		comps::Mesh { mesh: mesh.into() },
		comps::Target { pos: None },
	));
	Ok(res)
}

struct Map
{
	world: hecs::World,
	rng: StdRng,
	player: hecs::Entity,
	player_pos: Point3<f32>,
	project: Perspective3<f32>,
}

impl Map
{
	fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		let rng = StdRng::seed_from_u64(thread_rng().gen::<u16>() as u64);
		let mut world = hecs::World::new();

		let player = make_player(Point3::new(0., 0., 0.), &mut world, state)?;

		Ok(Self {
			world: world,
			rng: rng,
			player_pos: Point3::new(0., 0., 0.),
			player: player,
			project: utils::projection_transform(state.display_width, state.display_height),
		})
	}

	fn make_camera(&self) -> Isometry3<f32>
	{
		let height = 30.;
		utils::make_camera(
			self.player_pos + Vector3::new(0., height, height / 2.),
			self.player_pos,
		)
	}

	fn logic(&mut self, state: &mut game_state::GameState)
		-> Result<Option<game_state::NextScreen>>
	{
		let mut to_die = vec![];
		let dt = utils::DT as f32;

		// Physics
		for (_, (pos, vel)) in self
			.world
			.query::<(&mut comps::Position, &comps::Velocity)>()
			.iter()
		{
			pos.pos += dt * vel.vel;
			pos.dir += dt * vel.dir_vel;
		}

		// Add target
		let want_move = state.controls.get_action_state(controls::Action::Move) > 0.5;
		if want_move
		{
			state.controls.clear_action_state(controls::Action::Move);

			let (x, y) = (state.mouse_pos.x, state.mouse_pos.y);
			let fx = -1. + 2. * x as f32 / state.display_width;
			let fy = -1. + 2. * y as f32 / state.display_height;
			let camera = self.make_camera();
			let ground_pos = utils::get_ground_from_screen(fx, -fy, self.project, camera);

			let mut add_target = false;
			if let Ok(mut target) = self.world.get::<&mut comps::Target>(self.player)
			{
				target.pos = Some(ground_pos);
				add_target = true;
			}

			if add_target
			{
				make_target(ground_pos, &mut self.world, state)?;
			}
		}

		// Target movement.
		for (_, (target, pos, vel)) in self
			.world
			.query::<(&mut comps::Target, &comps::Position, &mut comps::Velocity)>()
			.iter()
		{
			if target.pos.is_none()
			{
				continue;
			}
			let target_pos = target.pos.unwrap();
			let diff = target_pos - pos.pos;
			if diff.magnitude() < 0.1
			{
				vel.vel = Vector3::zeros();
				vel.dir_vel = 0.;
				continue;
			}

			let diff = diff.xz().normalize();
			let rot = Rotation2::new(-pos.dir);
			let forward = rot * Vector2::new(0., -1.);
			let left = rot * Vector2::new(1., 0.);
			if diff.dot(&left) > 0.
			{
				vel.dir_vel = -4.0;
			}
			else
			{
				vel.dir_vel = 4.0;
			}
			if forward.dot(&diff) > 0.5
			{
				vel.vel = 10. * Vector3::new(forward.x, 0., forward.y);
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

	fn draw(&mut self, state: &game_state::GameState) -> Result<()>
	{
		state.core.set_depth_test(Some(DepthFunction::Less));

		state
			.core
			.use_shader(Some(&*state.basic_shader.upgrade().unwrap()))
			.unwrap();

		state
			.core
			.use_projection_transform(&utils::mat4_to_transform(self.project.into_inner()));

		let camera = self.make_camera();

		for (_, (pos, mesh)) in self
			.world
			.query::<(&comps::Position, &comps::Mesh)>()
			.iter()
		{
			let shift = Isometry3::new(pos.pos.coords, pos.dir * Vector3::y());
			state.core.use_transform(&utils::mat4_to_transform(
				camera.to_homogeneous() * shift.to_homogeneous(),
			));

			state
				.get_mesh(&mesh.mesh)
				.unwrap()
				.draw(&state.prim, |s| state.get_bitmap(s));
		}

		Ok(())
	}
}
