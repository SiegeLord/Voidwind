use crate::sprite;
use allegro::*;
use na::Point3;
use nalgebra as na;
use rand::prelude::*;

#[derive(Debug, Copy, Clone)]
pub struct Position
{
	pub pos: Point3<i32>,
	pub dir: i32,
}
