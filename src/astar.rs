use na::Point2;
use nalgebra as na;

#[derive(Copy, Clone, Debug)]
pub struct NodeAndScore
{
	pos: Point2<i32>,
	f_score: f32,
}

impl NodeAndScore
{
	pub fn new(pos: Point2<i32>, f_score: f32) -> NodeAndScore
	{
		NodeAndScore {
			pos: pos,
			f_score: f_score,
		}
	}
}

impl Ord for NodeAndScore
{
	fn cmp(&self, other: &Self) -> std::cmp::Ordering
	{
		// Reverse to make the heap a minheap
		self.f_score.partial_cmp(&other.f_score).unwrap().reverse()
	}
}

impl PartialOrd for NodeAndScore
{
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering>
	{
		Some(self.cmp(other))
	}
}

impl PartialEq for NodeAndScore
{
	fn eq(&self, other: &Self) -> bool
	{
		self == other
	}
}

impl Eq for NodeAndScore {}

pub struct AStarContext
{
	open_set: std::collections::BinaryHeap<NodeAndScore>,
	came_from: Vec<isize>,
	cost: Vec<f32>,
	size: usize,
}

impl AStarContext
{
	pub fn new(size: usize) -> AStarContext
	{
		let len = size * size;
		AStarContext {
			open_set: std::collections::BinaryHeap::new(),
			came_from: vec![-1; len],
			cost: vec![0.; len],
			size: size,
		}
	}

	fn heuristic(&self, from: Point2<i32>, to: Point2<i32>) -> f32
	{
		let dx = (from.x - to.x) as f32;
		let dy = (from.y - to.y) as f32;
		(dx * dx + dy * dy).sqrt()
	}

	fn map_to_idx(&self, pos: Point2<i32>) -> Option<usize>
	{
		if pos.x < 0 || pos.y < 0 || pos.x >= self.size as i32 || pos.y >= self.size as i32
		{
			None
		}
		else
		{
			Some((pos.y * self.size as i32 + pos.x) as usize)
		}
	}

	fn idx_to_map(&self, idx: usize) -> Point2<i32>
	{
		Point2::new((idx % self.size) as i32, (idx / self.size) as i32)
	}

	/// N.B. this returns the path in reverse order.
	pub fn solve<S: Fn(Point2<i32>) -> bool, C: Fn(Point2<i32>) -> f32>(
		&mut self, from: Point2<i32>, to: Point2<i32>, is_solid: S, cost_fn: C,
	) -> Vec<Point2<i32>>
	{
		self.open_set.clear();
		for i in 0..self.came_from.len()
		{
			self.came_from[i] = -1;
			self.cost[i] = 1e6;
		}

		let from_idx = self.map_to_idx(from).unwrap();
		self.cost[from_idx] = self.heuristic(from, from);
		self.came_from[from_idx] = from_idx as isize;
		self.open_set
			.push(NodeAndScore::new(from, self.heuristic(from, from)));

		let mut best_score_so_far = self.heuristic(from, to);
		let mut best_idx_so_far = -1;

		let to_idx = self.map_to_idx(to).unwrap();
		while !self.open_set.is_empty()
		{
			let cur = self.open_set.pop().unwrap();
			//~ println!("Trying {:?}", cur);
			let cur_idx = self.map_to_idx(cur.pos).unwrap();
			if cur_idx == to_idx
			{
				let mut cur_idx = to_idx;
				let mut path = vec![to];
				//~ println!("Start {:?} {:?}", from, to);
				loop
				{
					//~ println!("Reconstructing: {}", cur_idx);
					cur_idx = self.came_from[cur_idx] as usize;
					path.push(self.idx_to_map(cur_idx));
					if cur_idx == from_idx
					{
						//~ path.reverse();
						//~ println!("Path len {}", path.len());
						//~ println!("Done");
						return path;
					}
				}
			}

			for (dx, dy, cost) in &[(-1, 0, 1.), (1, 0, 1.), (0, -1, 1.), (0, 1, 1.)]
			{
				let next = Point2::new(cur.pos.x + dx, cur.pos.y + dy);
				if let Some(next_idx) = self.map_to_idx(next)
				{
					if is_solid(next)
					{
						continue;
					}

					let new_cost = self.cost[cur_idx] + cost + cost_fn(next);
					if new_cost < self.cost[next_idx]
					{
						let new_heuristic = self.heuristic(next, to);
						if new_heuristic < best_score_so_far
						{
							best_score_so_far = new_heuristic;
							best_idx_so_far = next_idx as isize;
						}

						self.came_from[next_idx] = cur_idx as isize;
						self.cost[next_idx] = new_cost;
						self.open_set
							.push(NodeAndScore::new(next, new_cost + new_heuristic));
					}
				}
			}
		}
		if best_idx_so_far > -1
		{
			let mut cur_idx = best_idx_so_far as usize;
			let mut path = vec![self.idx_to_map(cur_idx)];
			//~ println!("Start {:?} {:?}", from, to);
			loop
			{
				//~ println!("Reconstructing: {}", cur_idx);
				cur_idx = self.came_from[cur_idx] as usize;
				path.push(self.idx_to_map(cur_idx));
				if cur_idx == from_idx
				{
					//~ path.reverse();
					//~ println!("Path len {}", path.len());
					//~ println!("Done");
					return path;
				}
			}
		}
		else
		{
			vec![]
		}
	}
}
