use crate::error::Result;
use crate::game_state::GameState;
use crate::{atlas, utils};
use allegro::*;
use na::Point2;
use nalgebra as na;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct SpriteDesc
{
	bitmap: String,
	width: i32,
	height: i32,
	#[serde(default)]
	center_x: i32,
	#[serde(default)]
	center_y: i32,
}

#[derive(Clone, Debug)]
pub struct Sprite
{
	desc: SpriteDesc,
	variants: Vec<atlas::AtlasBitmap>,
}

impl Sprite
{
	pub fn load(sprite: &str, core: &Core, atlas: &mut atlas::Atlas) -> Result<Sprite>
	{
		let desc: SpriteDesc = utils::load_config(sprite)?;

		let bitmap = utils::load_bitmap(&core, &desc.bitmap)?;

		let num_variants_y = bitmap.get_height() / desc.height;
		let num_variants_x = bitmap.get_width() / desc.width;
		let num_variants = num_variants_x * num_variants_y;
		let mut variants = Vec::with_capacity(num_variants as usize);
		for y in 0..num_variants_y
		{
			for x in 0..num_variants_x
			{
				variants.push(
					atlas.insert(
						&core,
						&*bitmap
							.create_sub_bitmap(
								x * desc.width,
								y * desc.height,
								desc.width,
								desc.height,
							)
							.map_err(|_| "Couldn't create sub-bitmap?".to_string())?
							.upgrade()
							.unwrap(),
					)?,
				)
			}
		}
		Ok(Sprite {
			desc: desc,
			variants: variants,
		})
	}

	pub fn num_variants(&self) -> i32
	{
		self.variants.len() as i32
	}

	pub fn draw(&self, pos: Point2<f32>, variant: i32, tint: Color, state: &GameState)
	{
		let w = self.desc.width as f32;
		let h = self.desc.height as f32;
		let atlas_bmp = &self.variants[variant as usize];

		state.core.draw_tinted_bitmap_region(
			&state.atlas.pages[atlas_bmp.page].bitmap,
			tint,
			atlas_bmp.start.x,
			atlas_bmp.start.y,
			w,
			h,
			pos.x - self.desc.center_x as f32,
			pos.y - self.desc.center_y as f32,
			Flag::zero(),
		);
	}
}
