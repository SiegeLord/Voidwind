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

#[derive(Clone)]
pub struct Mesh
{
	vtxs: Vec<NormVertex>,
	idxs: Vec<i32>,
	material: Option<String>,
}

#[derive(Clone, Debug)]
#[repr(C)]
struct NormVertex
{
	x: f32,
	y: f32,
	z: f32,
	u: f32,
	v: f32,
	nx: f32,
	ny: f32,
	nz: f32,
	color: Color,
}

unsafe impl VertexType for NormVertex
{
	fn get_decl(prim: &PrimitivesAddon) -> VertexDecl
	{
		fn make_builder() -> std::result::Result<VertexDeclBuilder, ()>
		{
			VertexDeclBuilder::new(std::mem::size_of::<NormVertex>())
				.pos(
					VertexAttrStorage::F32_3,
					memoffset::offset_of!(NormVertex, x),
				)?
				.uv(
					VertexAttrStorage::F32_2,
					memoffset::offset_of!(NormVertex, u),
				)?
				.color(memoffset::offset_of!(NormVertex, color))?
				.user_attr(
					VertexAttrStorage::F32_3,
					memoffset::offset_of!(NormVertex, nx),
				)
		}

		VertexDecl::from_builder(prim, &make_builder().unwrap())
	}
}

fn load_meshes(gltf_file: &str) -> Vec<Mesh>
{
	let (document, buffers, _) = gltf::import(gltf_file).unwrap();
	let mut meshes = vec![];
	for node in document.nodes()
	{
		if let Some(mesh) = node.mesh()
		{
			for prim in mesh.primitives()
			{
				let mut vtxs = vec![];
				let mut idxs = vec![];
				let reader = prim.reader(|buffer| Some(&buffers[buffer.index()]));
				if let (
					Some(pos_iter),
					Some(gltf::mesh::util::ReadTexCoords::F32(uv_iter)),
					Some(normal_iter),
				) = (
					reader.read_positions(),
					reader.read_tex_coords(0),
					reader.read_normals(),
				)
				{
					for ((pos, uv), normal) in pos_iter.zip(uv_iter).zip(normal_iter)
					{
						vtxs.push(NormVertex {
							x: pos[0],
							y: pos[1],
							z: pos[2],
							u: uv[0],
							v: uv[1],
							nx: normal[0],
							ny: normal[1],
							nz: normal[2],
							color: Color::from_rgb_f(1., 1., 1.),
						});
					}
				}

				if let Some(iter) = reader.read_indices()
				{
					for idx in iter.into_u32()
					{
						idxs.push(idx as i32)
					}
				}
				meshes.push(Mesh {
					vtxs: vtxs,
					idxs: idxs,
					material: prim.material().name().map(|x| x.into()),
				});
			}
		}
	}
	meshes
}

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
	player_pos: Point3<f32>,
	project: Perspective3<f32>,
	meshes: Vec<Mesh>,
}

impl Map
{
	fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		let rng = StdRng::seed_from_u64(thread_rng().gen::<u16>() as u64);
		let world = hecs::World::new();

		let meshes = load_meshes("data/test.glb");

		Ok(Self {
			world: world,
			rng: rng,
			player_pos: Point3::new(0., 0., 0.),
			project: utils::projection_transform(state.display_width, state.display_height),
			meshes: meshes,
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

	fn logic(
		&mut self, _state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
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
		//let camera = make_camera(Point3::new(5., 5., -5.), Point3::new(0., 0., 0.));
		//let height = 30.;
		//let camera = utils::make_camera(
		//    self.player_pos + Vector3::new(0., height, height / 2.),
		//    self.player_pos,
		//);

		state
			.core
			.use_transform(&utils::mat4_to_transform(camera.to_homogeneous()));

		for mesh in self.meshes.iter()
		{
			state.prim.draw_indexed_prim(
				&mesh.vtxs[..],
				Option::<&Bitmap>::None,
				//mesh.material					.as_ref()					.and_then(|m| materials.get(m.as_str())),
				&mesh.idxs[..],
				0,
				mesh.idxs.len() as u32,
				PrimType::TriangleList,
			);
		}

		let shift = Isometry3::new(Vector3::new(0., 0., -10.), Vector3::zeros());
		state.core.use_transform(&utils::mat4_to_transform(
			camera.to_homogeneous() * shift.to_homogeneous(),
		));

		for mesh in self.meshes.iter()
		{
			state.prim.draw_indexed_prim(
				&mesh.vtxs[..],
				Option::<&Bitmap>::None,
				//mesh.material					.as_ref()					.and_then(|m| materials.get(m.as_str())),
				&mesh.idxs[..],
				0,
				mesh.idxs.len() as u32,
				PrimType::TriangleList,
			);
		}
		Ok(())
	}
}
