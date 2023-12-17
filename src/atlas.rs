use crate::error::Result;
use crate::utils;

use allegro::*;
use allegro_sys::*;
use nalgebra::Point2;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct AtlasBitmap
{
	pub start: Point2<f32>,
	pub end: Point2<f32>,
	pub page: usize,
}

impl AtlasBitmap
{
	pub fn width(&self) -> f32
	{
		self.end.x - self.start.x
	}

	pub fn height(&self) -> f32
	{
		self.end.y - self.start.y
	}
}

pub struct Page
{
	pub bitmap: Bitmap,
	packer: rect_packer::Packer,
}

impl Page
{
	fn new(core: &Core, size: i32) -> Result<Self>
	{
		let config = rect_packer::Config {
			width: size,
			height: size,
			border_padding: 1,
			rectangle_padding: 1,
		};

		let bitmap = Bitmap::new(core, size, size)
			.map_err(|_| format!("Couldn't create page with size {}x{}", size, size))?;
		core.set_target_bitmap(Some(&bitmap));
		core.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::Zero);
		core.clear_to_color(Color::from_rgba_f(0., 0., 0., 0.));
		core.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::InverseAlpha);

		Ok(Page {
			bitmap: bitmap,
			packer: rect_packer::Packer::new(config),
		})
	}

	fn insert<B: BitmapLike>(&mut self, core: &Core, bitmap: &B, page: usize)
		-> Option<AtlasBitmap>
	{
		if let Some(placement) = self
			.packer
			.pack(bitmap.get_width(), bitmap.get_height(), false)
		{
			core.set_target_bitmap(Some(&self.bitmap));
			core.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::Zero);
			core.draw_bitmap(bitmap, placement.x as f32, placement.y as f32, Flag::zero());
			core.set_blender(BlendOperation::Add, BlendMode::One, BlendMode::InverseAlpha);
			Some(AtlasBitmap {
				start: Point2::new(placement.x as f32, placement.y as f32),
				end: Point2::new(
					(placement.x + placement.width) as f32,
					(placement.y + placement.height) as f32,
				),
				page: page,
			})
		}
		else
		{
			None
		}
	}
}

pub struct Atlas
{
	pub pages: Vec<Page>,
	bitmaps: HashMap<String, AtlasBitmap>,
	page_size: i32,
}

impl Atlas
{
	pub fn new(page_size: i32) -> Self
	{
		Self {
			pages: vec![],
			bitmaps: HashMap::new(),
			page_size: page_size,
		}
	}

	pub fn insert<B: BitmapLike>(&mut self, core: &Core, bitmap: &B) -> Result<AtlasBitmap>
	{
		for (id, page) in self.pages.iter_mut().enumerate()
		{
			if let Some(atlas_bitmap) = page.insert(core, bitmap, id)
			{
				return Ok(atlas_bitmap);
			}
		}

		self.pages.push(Page::new(core, self.page_size)?);
		let id = self.pages.len() - 1;
		if let Some(atlas_bitmap) = self.pages.last_mut().unwrap().insert(core, bitmap, id)
		{
			return Ok(atlas_bitmap);
		}
		else
		{
			Err(format!(
				"Couldn't fit bitmap with size {}x{}",
				bitmap.get_width(),
				bitmap.get_height()
			)
			.into())
		}
	}

	pub fn insert_from_file(&mut self, core: &Core, filename: &str) -> Result<AtlasBitmap>
	{
		let old_flags = core.get_new_bitmap_flags();
		core.set_new_bitmap_flags(MEMORY_BITMAP);
		let bitmap = utils::load_bitmap(core, filename)?;
		core.set_new_bitmap_flags(old_flags);
		self.insert(core, &bitmap)
			.map_err(|e| e.context(format!("Couldn't fit bitmap {}", filename)))
	}

	pub fn dump_pages(&self)
	{
		use std::ffi::CString;
		for (id, page) in self.pages.iter().enumerate()
		{
			let filename = format!("page{}.png", id);
			let c_filename = CString::new(filename.as_bytes()).unwrap();
			unsafe {
				al_save_bitmap(c_filename.as_ptr(), page.bitmap.get_allegro_bitmap());
			}
		}
	}
}
