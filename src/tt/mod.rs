use crate::board::{Board, movegen::Move};

#[derive(Clone, Copy, Debug)]
pub enum ScoreType {
	Exact(i32),
	Upper(i32),
	Lower(i32),
}

impl Default for ScoreType {
	fn default() -> Self {
		Self::Exact(0)
	}
}

#[derive(Default, Clone, Debug)]
pub struct TTEntry {
	pub hash: u64,
	pub score: ScoreType,
	pub best_move: Move,
	pub depth: i16,
}

pub struct TT {
	size_exponent: usize,
	table: Box<[TTEntry]>,
}

impl TT {
	pub fn new(size_exponent: usize) -> Self {
		Self {
			size_exponent,
			table: vec![Default::default(); 1 << size_exponent].into_boxed_slice(),
		}
	}

	pub fn add_entry(&mut self, board: &Board, best_move: Move, depth: i16, score: ScoreType) {
		let idx = board.get_hash() % (1 << self.size_exponent as u64);
		self.table[idx as usize] = TTEntry {
			hash: board.get_hash(),
			best_move,
			depth,
			score,
		};
	}

	pub fn probe(&self, board: &Board) -> Option<&TTEntry> {
		let idx = board.get_hash() % (1 << self.size_exponent as u64);
		let entry = &self.table[idx as usize];
		if board.get_hash() == entry.hash {
			return Some(entry);
		}

		None
	}
}
