#![allow(non_snake_case)]
#![allow(unused_imports)]
#![allow(dead_code)]

mod astar;
mod atlas;
mod components;
mod controls;
mod deferred;
mod error;
mod game;
mod game_state;
mod menu;
mod mesh;
mod sfx;
mod spatial_grid;
mod sprite;
mod ui;
mod utils;

use crate::error::Result;
use allegro::*;
use allegro_dialog::*;
use allegro_sys::*;
use rand::prelude::*;
use serde_derive::{Deserialize, Serialize};
use std::rc::Rc;

enum Screen
{
	Game(game::Game),
	Menu(menu::Menu),
}

fn real_main() -> Result<()>
{
	println!("Version: {}", game_state::VERSION);

	let mut state = game_state::GameState::new()?;

	let mut flags = OPENGL | OPENGL_3_0 | PROGRAMMABLE_PIPELINE;

	if state.options.fullscreen
	{
		flags = flags | FULLSCREEN_WINDOW;
	}
	state.core.set_new_display_flags(flags);

	if state.options.vsync_method == 1
	{
		state.core.set_new_display_option(
			DisplayOption::Vsync,
			1,
			DisplayOptionImportance::Suggest,
		);
	}
	state.core.set_new_display_option(
		DisplayOption::DepthSize,
		16,
		DisplayOptionImportance::Suggest,
	);
	let mut display = Display::new(&state.core, state.options.width, state.options.height)
		.map_err(|_| "Couldn't create display".to_string())?;

	gl_loader::init_gl();
	gl::load_with(|symbol| gl_loader::get_proc_address(symbol) as *const _);

	state.post_init(&mut display)?;

	let timer = Timer::new(&state.core, utils::DT as f64)
		.map_err(|_| "Couldn't create timer".to_string())?;

	let queue =
		EventQueue::new(&state.core).map_err(|_| "Couldn't create event queue".to_string())?;
	queue.register_event_source(display.get_event_source());
	queue.register_event_source(
		state
			.core
			.get_keyboard_event_source()
			.expect("Couldn't get keyboard"),
	);
	queue.register_event_source(
		state
			.core
			.get_mouse_event_source()
			.expect("Couldn't get mouse"),
	);
	queue.register_event_source(timer.get_event_source());

	let mut quit = false;
	let mut draw = true;

	let mut cur_screen = Screen::Menu(menu::Menu::new(&mut state)?);
	//let mut cur_screen = Screen::Game(game::Game::new(&mut state)?);

	let mut logics_without_draw = 0;
	let mut old_fullscreen = state.options.fullscreen;

	let mut frame_times = circular_buffer::CircularBuffer::<16, _>::new();
	let mut logic_times = circular_buffer::CircularBuffer::<16, _>::new();
	//state.core.grab_mouse(&display).ok();
	//display.show_cursor(false).ok();

	timer.start();
	while !quit
	{
		if draw && queue.is_empty()
		{
			if state.display_width != display.get_width() as f32
				|| state.display_height != display.get_height() as f32
			{
				state.display_width = display.get_width() as f32;
				state.display_height = display.get_height() as f32;
			}

			let frame_start = state.core.get_time();

			state.core.set_target_bitmap(state.buffer.as_ref());
			unsafe {
				gl::Disable(gl::CULL_FACE);
			}
			state.core.clear_to_color(Color::from_rgb_f(0., 0., 0.5));
			state.core.clear_depth_buffer(1.);
			state.core.set_depth_test(None);

			match &mut cur_screen
			{
				Screen::Game(game) => game.draw(&state)?,
				Screen::Menu(menu) => menu.draw(&state)?,
			}

			if state.options.vsync_method == 2
			{
				state.core.wait_for_vsync().ok();
			}

			state.core.set_target_bitmap(Some(display.get_backbuffer()));
			state
				.core
				.draw_bitmap(state.buffer.as_ref().unwrap(), 0., 0., Flag::zero());
			state.core.flip_display();

			frame_times.push_back(state.core.get_time() - frame_start);
			if (state.tick + 1) % 120 == 0
			{
				let mut sum = 0.;
				for v in &frame_times
				{
					sum += v;
				}
				//~ println!("FPS: {:.2}", 1. / (sum / frame_times.len() as f64));
				let mut sum = 0.;
				for v in &logic_times
				{
					sum += v;
				}
				//~ println!("LPS: {:.2}", 1. / (sum / logic_times.len() as f64));
			}
			logics_without_draw = 0;
			draw = false;
		}

		let event = queue.wait_for_event();
		let mut next_screen = match &mut cur_screen
		{
			Screen::Game(game) => game.input(&event, &mut state)?,
			Screen::Menu(menu) => menu.input(&event, &mut state)?,
		};

		match event
		{
			Event::DisplayClose { .. } => quit = true,
			Event::DisplaySwitchIn { .. } =>
			{
				//state.core.grab_mouse(&display).ok();
				//display.show_cursor(false).ok();
				state.track_mouse = true;
			}
			Event::DisplaySwitchOut { .. } =>
			{
				//state.core.ungrab_mouse().ok();
				//display.show_cursor(true).ok();
				state.track_mouse = false;
			}
			Event::MouseButtonDown { .. } =>
			{
				//state.core.grab_mouse(&display).ok();
				//display.show_cursor(false).ok();
				state.track_mouse = true;
			}
			Event::TimerTick { .. } =>
			{
				if logics_without_draw > 10
				{
					continue;
				}

				let frame_start = state.core.get_time();
				if next_screen.is_none()
				{
					next_screen = match &mut cur_screen
					{
						Screen::Game(game) => game.logic(&mut state)?,
						_ => None,
					}
				}
				logic_times.push_back(state.core.get_time() - frame_start);

				if old_fullscreen != state.options.fullscreen
				{
					display.set_flag(FULLSCREEN_WINDOW, state.options.fullscreen);
					old_fullscreen = state.options.fullscreen;
					state.create_buffers(&mut display)?;

					match &mut cur_screen
					{
						Screen::Game(game) => game.change_buffers(&mut state)?,
						Screen::Menu(menu) => menu.change_buffers(&mut state)?,
					}
				}

				logics_without_draw += 1;
				state.sfx.update_sounds()?;

				if !state.paused
				{
					state.tick += 1;
				}
				draw = true;
			}
			_ => (),
		}

		if let Some(next_screen) = next_screen
		{
			match next_screen
			{
				game_state::NextScreen::Game =>
				{
					cur_screen = Screen::Game(game::Game::new(&mut state)?);
				}
				game_state::NextScreen::Menu =>
				{
					cur_screen = Screen::Menu(menu::Menu::new(&mut state)?);
				}
				game_state::NextScreen::Quit =>
				{
					quit = true;
				}
				_ => panic!("Unknown next screen {:?}", next_screen),
			}
		}
	}

	Ok(())
}

allegro_main! {
	use std::panic::catch_unwind;

	match catch_unwind(|| real_main().unwrap())
	{
		Err(e) =>
		{
			let err: String = e
				.downcast_ref::<&'static str>()
				.map(|&e| e.to_owned())
				.or_else(|| e.downcast_ref::<String>().map(|e| e.clone()))
				.unwrap_or("Unknown error!".to_owned());

			let mut lines = vec![];
			for line in err.lines().take(10)
			{
				lines.push(line.to_string());
			}
			show_native_message_box(
				None,
				"Error!",
				"An error has occurred!",
				&lines.join("\n"),
				Some("You make me sad."),
				MESSAGEBOX_ERROR,
			);
		}
		Ok(_) => (),
	}
}
