use crate::board::{Board, movegen::Move};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScoreType {
	#[default]
	Exact = 0,
	Upper = 1,
	Lower = 2,
}

impl From<u8> for ScoreType {
	fn from(value: u8) -> Self {
		match value {
			0 => Self::Exact,
			1 => Self::Upper,
			2 => Self::Lower,
			_ => panic!("Invalid ScoreType determinant {value}"),
		}
	}
}

#[derive(Default, Clone, Debug)]
pub struct TTEntry {
	pub hash: u64,
	pub score: i16,
	pub score_type: ScoreType,
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

	pub fn add_entry(
		&mut self,
		board: &Board,
		best_move: Move,
		depth: i16,
		score: i16,
		score_type: ScoreType,
	) {
		let idx = board.get_hash() % (1 << self.size_exponent as u64);
		self.table[idx as usize] = TTEntry {
			hash: board.get_hash(),
			best_move,
			depth,
			score,
			score_type,
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
