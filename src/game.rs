use crate::error::Result;
use crate::{
	astar, components as comps, controls, game_state, mesh, spatial_grid, sprite, ui, utils,
};
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

use std::f32::consts::PI;

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

fn make_projectile(
	pos: Point3<f32>, dir: Vector3<f32>, world: &mut hecs::World, state: &mut game_state::GameState,
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
		comps::Mesh { mesh: mesh.into() },
		comps::TimeToDie {
			time_to_die: state.time() + 1.,
		},
		comps::AffectedByGravity,
		comps::CollidesWithWater,
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
		comps::Target { waypoints: vec![] },
		comps::Stats { speed: 10. },
		comps::Solid { size: 2., mass: 1. },
		comps::Equipment {
			slots: vec![
				comps::ItemSlot {
					pos: Point2::new(0.5, 1.0),
					dir: PI / 2.0,

					item: Some(comps::Item {
						kind: comps::ItemKind::Weapon(comps::Weapon::new(comps::WeaponStats {
							fire_interval: 1.,
							arc: PI / 2.0,
						})),
					}),
				},
				comps::ItemSlot {
					pos: Point2::new(-0.5, 1.0),
					dir: PI / 2.0,

					item: Some(comps::Item {
						kind: comps::ItemKind::Weapon(comps::Weapon::new(comps::WeaponStats {
							fire_interval: 1.,
							arc: PI / 2.0,
						})),
					}),
				},
				comps::ItemSlot {
					pos: Point2::new(0.0, -1.0),
					dir: -PI / 2.0,
					item: Some(comps::Item {
						kind: comps::ItemKind::Weapon(comps::Weapon::new(comps::WeaponStats {
							fire_interval: 1.,
							arc: PI / 2.0,
						})),
					}),
				},
			],
			want_action_1: false,
			target_pos: Point3::new(0., 0., 0.),
			allow_out_of_arc_shots: true,
		},
	));
	Ok(res)
}

fn make_enemy(
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
		comps::Target { waypoints: vec![] },
		comps::AI {
			state: comps::AIState::Idle,
		},
		comps::Stats { speed: 5. },
		comps::Solid { size: 2., mass: 1. },
		comps::Equipment {
			slots: vec![
				comps::ItemSlot {
					pos: Point2::new(0.5, 1.0),
					dir: PI / 2.0,

					item: Some(comps::Item {
						kind: comps::ItemKind::Weapon(comps::Weapon::new(comps::WeaponStats {
							fire_interval: 1.,
							arc: PI / 2.0,
						})),
					}),
				},
				comps::ItemSlot {
					pos: Point2::new(-0.5, 1.0),
					dir: PI / 2.0,

					item: Some(comps::Item {
						kind: comps::ItemKind::Weapon(comps::Weapon::new(comps::WeaponStats {
							fire_interval: 1.,
							arc: PI / 2.0,
						})),
					}),
				},
				comps::ItemSlot {
					pos: Point2::new(0.0, -1.0),
					dir: -PI / 2.0,
					item: Some(comps::Item {
						kind: comps::ItemKind::Weapon(comps::Weapon::new(comps::WeaponStats {
							fire_interval: 1.,
							arc: PI / 2.0,
						})),
					}),
				},
			],
			want_action_1: false,
			target_pos: Point3::new(0., 0., 0.),
			allow_out_of_arc_shots: false,
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
	project: Perspective3<f32>,
}

impl Map
{
	fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		let rng = StdRng::seed_from_u64(thread_rng().gen::<u16>() as u64);
		let mut world = hecs::World::new();

		let player = make_player(Point3::new(0., 0., 0.), &mut world, state)?;

		make_enemy(Point3::new(30., 0., 0.), &mut world, state)?;
		make_enemy(Point3::new(30., 0., 10.), &mut world, state)?;

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

	fn get_mouse_ground_pos(&self, state: &game_state::GameState) -> Point3<f32>
	{
		let (x, y) = (state.mouse_pos.x, state.mouse_pos.y);
		let fx = -1. + 2. * x as f32 / state.display_width;
		let fy = -1. + 2. * y as f32 / state.display_height;
		let camera = self.make_camera();
		utils::get_ground_from_screen(fx, -fy, self.project, camera)
	}

	fn logic(&mut self, state: &mut game_state::GameState)
		-> Result<Option<game_state::NextScreen>>
	{
		let mut to_die = vec![];
		let dt = utils::DT as f32;

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

		// Collision resolution.
		let mut colliding_pairs = vec![];
		for (a, b) in grid.all_pairs(|a, b| {
			let a_solid = self.world.get::<&comps::Solid>(a.inner.entity).unwrap();
			let b_solid = self.world.get::<&comps::Solid>(b.inner.entity).unwrap();
			a_solid.collides_with(&*b_solid)
		})
		{
			colliding_pairs.push((a.inner, b.inner));
		}

		//let mut on_contact_effects = vec![];
		for _pass in 0..5
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

				// if pass == 0
				// {
				// 	for (id, other_id) in [(id1, Some(id2)), (id2, Some(id1))]
				// 	{
				// 		if let Ok(on_contact_effect) =
				// 			self.world.get::<&components::OnContactEffect>(id)
				// 		{
				// 			on_contact_effects.push((
				// 				id,
				// 				other_id,
				// 				on_contact_effect.effects.clone(),
				// 			));
				// 		}
				// 	}
				// }
			}
		}

		// Player Input
		let want_move = state.controls.get_action_state(controls::Action::Move) > 0.5;
		let want_queue = state.controls.get_action_state(controls::Action::Queue) > 0.5;
		let want_action_1 = state.controls.get_action_state(controls::Action::Action1) > 0.5;
		let mouse_ground_pos = self.get_mouse_ground_pos(state);
		if want_move
		{
			state.controls.clear_action_state(controls::Action::Move);
			let marker = make_target(mouse_ground_pos, &mut self.world, state)?;
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
			}
		}
		if want_action_1
		{
			if let Ok(mut equipment) = self.world.get::<&mut comps::Equipment>(self.player)
			{
				equipment.want_action_1 = true;
				equipment.target_pos = mouse_ground_pos;
			}
		}

		// Equipment actions
		let mut spawn_projectiles = vec![];
		for (_, (pos, equipment)) in self
			.world
			.query::<(&comps::Position, &mut comps::Equipment)>()
			.iter()
		{
			if !equipment.want_action_1
			{
				continue;
			}
			equipment.want_action_1 = false;
			for slot in &mut equipment.slots
			{
				if let Some(item) = slot.item.as_mut()
				{
					match &mut item.kind
					{
						comps::ItemKind::Weapon(weapon) =>
						{
							if state.time() > weapon.time_to_fire
							{
								let rot = Rotation2::new(pos.dir);
								let rot_slot = Rotation2::new(slot.dir);
								let slot_pos = pos.pos.zx() + rot * slot.pos.coords;
								let slot_dir = rot_slot * rot * Vector2::new(1., 0.);
								let target_dir = (equipment.target_pos.zx() - slot_pos).normalize();
								let min_dot = (weapon.stats.arc / 2.).cos();

								let spawn_pos = Point3::new(slot_pos.y, 3., slot_pos.x);
								let mut spawn_dir = None;
								if slot_dir.dot(&target_dir) > min_dot
								{
									spawn_dir = Some(target_dir);
								}
								else if slot_dir.dot(&target_dir) > 0.
									&& equipment.allow_out_of_arc_shots
								{
									let cand_dir1 =
										Rotation2::new(slot.dir + weapon.stats.arc / 2.)
											* rot * Vector2::new(1., 0.);
									let cand_dir2 =
										Rotation2::new(slot.dir - weapon.stats.arc / 2.)
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
									spawn_projectiles.push((spawn_pos, spawn_dir));
									weapon.time_to_fire =
										state.time() + weapon.stats.fire_interval as f64;
								}
							}
						}
					}
				}
			}
		}

		for (spawn_pos, spawn_dir) in spawn_projectiles
		{
			make_projectile(spawn_pos, spawn_dir, &mut self.world, state)?;
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
				vel.dir_vel = stats.speed / 5.;
			}
			else
			{
				vel.dir_vel = -stats.speed / 5.;
			}
			vel.vel = stats.speed * Vector3::new(forward.y, 0., forward.x);
		}

		// AI
		for (_id, (pos, target, ai, equipment)) in self
			.world
			.query::<(
				&comps::Position,
				&mut comps::Target,
				&mut comps::AI,
				&mut comps::Equipment,
			)>()
			.iter()
		{
			match ai.state
			{
				comps::AIState::Idle =>
				{
					let diff = pos.pos - self.player_pos;
					if diff.magnitude() < 30.
					{
						ai.state = comps::AIState::Pursuing(self.player);
					}
				}
				comps::AIState::Pursuing(target_entity) =>
				{
					if !self.world.contains(target_entity)
					{
						ai.state = comps::AIState::Idle;
					}
					else
					{
						let target_pos = self.world.get::<&comps::Position>(target_entity).unwrap();
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
						else
						{
							target.clear(|m| to_die.push(m));
							target.waypoints.push(comps::Waypoint {
								pos: target_pos.pos,
								marker: None,
							})
						}
					}
				}
				comps::AIState::Attacking(target_entity) =>
				{
					if !self.world.contains(target_entity)
					{
						ai.state = comps::AIState::Idle;
						equipment.want_action_1 = false;
					}
					else
					{
						let target_pos = self.world.get::<&comps::Position>(target_entity).unwrap();
						let diff = target_pos.pos - pos.pos;
						if diff.magnitude() > 20.
						{
							ai.state = comps::AIState::Pursuing(target_entity);
							equipment.want_action_1 = false;
						}
						else
						{
							if target.waypoints.is_empty()
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
			.use_projection_transform(&utils::mat4_to_transform(self.project.into_inner()));

		let camera = self.make_camera();

		let tl = utils::get_ground_from_screen(-1.0, 1.0, self.project, camera);
		let tr = utils::get_ground_from_screen(1.0, 1.0, self.project, camera);
		let bl = utils::get_ground_from_screen(-1.0, -1.0, self.project, camera);
		let br = utils::get_ground_from_screen(1.0, -1.0, self.project, camera);
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

		let (dw, dh) = (state.display_width as f32, state.display_height as f32);
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

		let mut weapon_slots = vec![];
		if let (Ok(pos), Ok(equipment)) = (
			self.world.get::<&comps::Position>(self.player),
			self.world.get::<&comps::Equipment>(self.player),
		)
		{
			for slot in &equipment.slots
			{
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
								slot.dir,
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
			let f = 1. - utils::clamp((time_to_fire - state.time()) as f32 / fire_interval, 0., 1.);

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
					slot_dir - arc / 2. + PI / 2.,
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
					slot_dir - arc / 2. + PI / 2.,
					*arc,
					Color::from_rgba_f(f, f, f, f),
					3.,
				);
			}
		}

		Ok(())
	}
}
