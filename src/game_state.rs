use crate::error::Result;
use crate::{atlas, controls, mesh, sfx, sprite, utils};
use allegro::*;
use allegro_font::*;
use allegro_image::*;
use allegro_primitives::*;
use allegro_ttf::*;
use nalgebra::Point2;
use serde_derive::{Deserialize, Serialize};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::{fmt, path, sync};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Options
{
	pub fullscreen: bool,
	pub width: i32,
	pub height: i32,
	pub play_music: bool,
	pub vsync_method: i32,
	pub sfx_volume: f32,
	pub music_volume: f32,

	pub controls: controls::Controls,
}

impl Default for Options
{
	fn default() -> Self
	{
		Self {
			fullscreen: false,
			width: 960,
			height: 864,
			play_music: true,
			vsync_method: 2,
			sfx_volume: 1.,
			music_volume: 1.,
			controls: controls::Controls::new(),
		}
	}
}

#[derive(Debug)]
pub enum NextScreen
{
	Game,
	Menu,
	InGameMenu,
	Quit,
}

fn make_shader(
	disp: &mut Display, vertex_path: &str, pixel_path: &str,
) -> Result<sync::Weak<Shader>>
{
	let shader = disp.create_shader(ShaderPlatform::GLSL).unwrap();

	shader
		.upgrade()
		.unwrap()
		.attach_shader_source(
			ShaderType::Vertex,
			Some(&utils::read_to_string(vertex_path)?),
		)
		.unwrap();

	shader
		.upgrade()
		.unwrap()
		.attach_shader_source(ShaderType::Pixel, Some(&utils::read_to_string(pixel_path)?))
		.unwrap();
	shader.upgrade().unwrap().build().unwrap();
	Ok(shader)
}

fn make_default_shader(core: &Core, disp: &mut Display) -> Result<sync::Weak<Shader>>
{
	let shader = disp.create_shader(ShaderPlatform::GLSL).unwrap();
	shader
		.upgrade()
		.unwrap()
		.attach_shader_source(
			ShaderType::Vertex,
			core.get_default_shader_source(ShaderPlatform::GLSL, ShaderType::Vertex)
				.as_ref()
				.map(|s| s.as_str()),
		)
		.unwrap();

	shader
		.upgrade()
		.unwrap()
		.attach_shader_source(
			ShaderType::Pixel,
			core.get_default_shader_source(ShaderPlatform::GLSL, ShaderType::Pixel)
				.as_ref()
				.map(|s| s.as_str()),
		)
		.unwrap();
	shader.upgrade().unwrap().build().unwrap();
	Ok(shader)
}

pub fn load_options(core: &Core) -> Result<Options>
{
	let mut path_buf = path::PathBuf::new();
	if cfg!(feature = "use_user_settings")
	{
		path_buf.push(
			core.get_standard_path(StandardPath::UserSettings)
				.map_err(|_| "Couldn't get standard path".to_string())?,
		);
	}
	path_buf.push("options.cfg");
	if path_buf.exists()
	{
		utils::load_config(path_buf.to_str().unwrap())
	}
	else
	{
		Ok(Default::default())
	}
}

pub fn save_options(core: &Core, options: &Options) -> Result<()>
{
	let mut path_buf = path::PathBuf::new();
	if cfg!(feature = "use_user_settings")
	{
		path_buf.push(
			core.get_standard_path(StandardPath::UserSettings)
				.map_err(|_| "Couldn't get standard path".to_string())?,
		);
	}
	std::fs::create_dir_all(&path_buf).map_err(|_| "Couldn't create directory".to_string())?;
	path_buf.push("options.cfg");
	utils::save_config(path_buf.to_str().unwrap(), &options)
}

pub struct GameState
{
	pub core: Core,
	pub prim: PrimitivesAddon,
	pub image: ImageAddon,
	pub font: FontAddon,
	pub ttf: TtfAddon,
	pub tick: i64,
	pub paused: bool,

	pub sfx: sfx::Sfx,
	pub atlas: atlas::Atlas,
	pub ui_font: Font,
	//pub number_font: Font,
	pub options: Options,
	pub draw_scale: f32,
	pub display_width: f32,
	pub display_height: f32,
	bitmaps: HashMap<String, Bitmap>,
	sprites: HashMap<String, sprite::Sprite>,
	meshes: HashMap<String, mesh::MultiMesh>,
	pub controls: controls::ControlsHandler,
	pub track_mouse: bool,
	pub mouse_pos: Point2<i32>,

	pub basic_shader: sync::Weak<Shader>,
	pub water_shader: sync::Weak<Shader>,
	pub default_shader: sync::Weak<Shader>,
}

impl GameState
{
	pub fn new() -> Result<Self>
	{
		let core = Core::init()?;
		core.set_app_name("SH2023");
		core.set_org_name("SiegeLord");

		let options = load_options(&core)?;
		let prim = PrimitivesAddon::init(&core)?;
		let image = ImageAddon::init(&core)?;
		let font = FontAddon::init(&core)?;
		let ttf = TtfAddon::init(&font)?;
		core.install_keyboard()
			.map_err(|_| "Couldn't install keyboard".to_string())?;
		core.install_mouse()
			.map_err(|_| "Couldn't install mouse".to_string())?;

		let sfx = sfx::Sfx::new(options.sfx_volume, options.music_volume, &core)?;

		let ui_font =
			Font::new_builtin(&font).map_err(|_| "Could't create builtin font.".to_string())?;

		let controls = controls::ControlsHandler::new(options.controls.clone());
		Ok(Self {
			options: options,
			core: core,
			prim: prim,
			image: image,
			tick: 0,
			bitmaps: HashMap::new(),
			sprites: HashMap::new(),
			meshes: HashMap::new(),
			font: font,
			ttf: ttf,
			sfx: sfx,
			paused: false,
			atlas: atlas::Atlas::new(512),
			ui_font: ui_font,
			draw_scale: 1.,
			display_width: 0.,
			display_height: 0.,
			controls: controls,
			track_mouse: true,
			mouse_pos: Point2::new(0, 0),
			basic_shader: sync::Weak::new(),
			water_shader: sync::Weak::new(),
			default_shader: sync::Weak::new(),
		})
	}

	pub fn post_init(&mut self, display: &mut Display) -> Result<()>
	{
		self.display_width = display.get_width() as f32;
		self.display_height = display.get_height() as f32;
		self.basic_shader =
			make_shader(display, "data/basic_vertex.glsl", "data/basic_pixel.glsl")?;
		self.water_shader =
			make_shader(display, "data/water_vertex.glsl", "data/water_pixel.glsl")?;
		self.default_shader = make_default_shader(&self.core, display)?;
		Ok(())
	}

	pub fn cache_bitmap<'l>(&'l mut self, name: &str) -> Result<&'l Bitmap>
	{
		Ok(match self.bitmaps.entry(name.to_string())
		{
			Entry::Occupied(o) => o.into_mut(),
			Entry::Vacant(v) => v.insert(utils::load_bitmap(&self.core, name)?),
		})
	}

	pub fn cache_sprite<'l>(&'l mut self, name: &str) -> Result<&'l sprite::Sprite>
	{
		Ok(match self.sprites.entry(name.to_string())
		{
			Entry::Occupied(o) => o.into_mut(),
			Entry::Vacant(v) => v.insert(sprite::Sprite::load(name, &self.core, &mut self.atlas)?),
		})
	}

	fn cache_mesh<'l>(&'l mut self, name: &str) -> Result<&'l mesh::MultiMesh>
	{
		let mesh = match self.meshes.entry(name.to_string())
		{
			Entry::Occupied(o) => o.into_mut(),
			Entry::Vacant(v) => v.insert(mesh::MultiMesh::load(name)?),
		};
		Ok(mesh)
	}

	pub fn get_bitmap<'l>(&'l self, name: &str) -> Option<&'l Bitmap>
	{
		self.bitmaps.get(name)
	}

	pub fn get_sprite<'l>(&'l self, name: &str) -> Option<&'l sprite::Sprite>
	{
		self.sprites.get(name)
	}

	pub fn get_mesh<'l>(&'l self, name: &str) -> Option<&'l mesh::MultiMesh>
	{
		self.meshes.get(name)
	}

	pub fn time(&self) -> f64
	{
		self.tick as f64 * utils::DT as f64
	}
}

pub fn cache_mesh(state: &mut GameState, name: &str) -> Result<()>
{
	let mesh = state.cache_mesh(name)?;
	let mut textures = vec![];
	for mesh in &mesh.meshes
	{
		if let Some(material) = mesh.material.as_ref()
		{
			textures.push(material.texture.clone());
		}
	}
	for texture in textures
	{
		state.cache_bitmap(&texture)?;
	}
	Ok(())
}
