use crate::board::{
	Board,
	movegen::Move,
	utils::{BLACK, Color, WHITE},
};

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
			0 | 3 => Self::Exact,
			1 => Self::Upper,
			2 => Self::Lower,
			_ => panic!("Invalid ScoreType determinant {value}"),
		}
	}
}

#[derive(Default, Clone, Debug)]
pub struct TTEntry {
	pub hash: u32,
	pub score: i16,
	flags: u8,
	pub best_move: Move,
	pub depth: u8,
}

impl TTEntry {
	#[inline(always)]
	pub fn get_score_type(&self) -> ScoreType {
		ScoreType::from(self.flags & 0x3)
	}

	#[inline(always)]
	pub fn get_generation(&self) -> u8 {
		self.flags >> 2
	}
}

//static_assertions::const_assert!(core::mem::size_of::<TTEntry>() == 16);

// Bit-trick coming from fast-hash
const fn u64_to_u32_fermat_residue(h: u64) -> u32 {
	(h as u32).wrapping_sub((h >> 32) as u32)
}

const fn u32_to_u16_fermat_residue(h: u32) -> u16 {
	(h as u16).wrapping_sub((h >> 16) as u16)
}

const fn u64_to_u16_fermat_residue(h: u64) -> u16 {
	u32_to_u16_fermat_residue(u64_to_u32_fermat_residue(h))
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
		depth: u8,
		score: i16,
		score_type: ScoreType,
	) {
		let idx = board.get_hash() % (1 << self.size_exponent);
		let entry = &mut self.table[idx as usize];
		let generation = (board.get_halfmove_clock() & 0x3F) as u8;
		let hash = u64_to_u32_fermat_residue(board.get_hash() & !((1 << self.size_exponent) - 1));
		if score_type == ScoreType::Exact
			|| depth >= entry.depth
			|| ((generation << 2).wrapping_sub(entry.get_generation() << 2)) != 0
		{
			*entry = TTEntry {
				hash,
				best_move,
				depth,
				score,
				flags: (score_type as u8) | generation << 2,
			};
		} else if entry.depth >= 5 {
			entry.depth -= 1;
		}
	}

	pub fn probe(&self, board: &Board) -> Option<&TTEntry> {
		let idx = board.get_hash() % (1 << self.size_exponent as u64);
		let entry = &self.table[idx as usize];
		if u64_to_u32_fermat_residue(board.get_hash() & !((1 << self.size_exponent) - 1))
			== entry.hash
			&& match board.get_turn() {
				Color::White => board.is_pseudo_legal::<WHITE>(&entry.best_move),
				Color::Black => board.is_pseudo_legal::<BLACK>(&entry.best_move),
			} {
			return Some(entry);
		}

		None
	}
}
