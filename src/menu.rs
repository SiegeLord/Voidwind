use crate::error::Result;
use crate::{components, controls, game_state, ui, utils};

use allegro::*;
use allegro_font::*;
use allegro_sys::*;
use nalgebra::{Matrix4, Point2};
use rand::prelude::*;

pub struct Menu
{
	subscreens: Vec<ui::SubScreen>,
}

fn to_f32(pos: Point2<i32>) -> Point2<f32>
{
	Point2::new(pos.x as f32, pos.y as f32)
}

impl Menu
{
	pub fn new(state: &mut game_state::GameState) -> Result<Self>
	{
		state.paused = false;

		state.cache_bitmap("data/title.png")?;

		Ok(Self {
			subscreens: vec![ui::SubScreen::MainMenu(ui::MainMenu::new(state))],
		})
	}

	pub fn input(
		&mut self, event: &Event, state: &mut game_state::GameState,
	) -> Result<Option<game_state::NextScreen>>
	{
		match *event
		{
			Event::MouseAxes { x, y, .. } =>
			{
				if state.track_mouse
				{
					state.mouse_pos = Point2::new(x as i32, y as i32);
				}
			}
			Event::KeyDown {
				keycode: KeyCode::Escape,
				..
			} =>
			{
				if self.subscreens.len() > 1
				{
					state.sfx.play_sound("data/ui2.ogg").unwrap();
					self.subscreens.pop().unwrap();
					return Ok(None);
				}
			}
			_ => (),
		}
		if let Some(action) = self.subscreens.last_mut().unwrap().input(state, event)
		{
			match action
			{
				ui::Action::Forward(subscreen_fn) =>
				{
					self.subscreens.push(subscreen_fn(state));
				}
				ui::Action::Start => return Ok(Some(game_state::NextScreen::Game)),
				ui::Action::Quit => return Ok(Some(game_state::NextScreen::Quit)),
				ui::Action::Back =>
				{
					self.subscreens.pop().unwrap();
				}
				_ => (),
			}
		}
		Ok(None)
	}

	pub fn draw(&mut self, state: &game_state::GameState) -> Result<()>
	{
		state.core.clear_to_color(Color::from_rgb_f(0., 0., 0.));
		let bmp = state.get_bitmap("data/title.png")?;
		let dw = state.display_width;
		let dh = state.display_height;
		state.core.draw_bitmap(
			bmp,
			dw / 2. - bmp.get_width() as f32 / 2.,
			dh / 2. - bmp.get_height() as f32 / 2.,
			Flag::zero(),
		);
		state.core.draw_text(
			&state.title_font,
			Color::from_rgb_f(0.4, 0.1, 0.5),
			dw / 2.,
			dh / 4.,
			FontAlign::Centre,
			"Voidwind",
		);
		state.core.draw_text(
			&state.ui_font,
			ui::ui_color(),
			state.m,
			dh - 2. * state.m,
			FontAlign::Left,
			&format!("Version: {}", game_state::VERSION),
		);
		self.subscreens.last().unwrap().draw(state);
		Ok(())
	}

	pub fn change_buffers(&mut self, state: &mut game_state::GameState) -> Result<()>
	{
		self.subscreens
			.push(ui::SubScreen::MainMenu(ui::MainMenu::new(state)));
		self.subscreens
			.push(ui::SubScreen::OptionsMenu(ui::OptionsMenu::new(state)));
		Ok(())
	}
}
