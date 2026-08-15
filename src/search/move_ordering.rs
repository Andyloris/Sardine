use crate::board::{
	Board,
	eval::MATERIAL_WEIGHTS,
	movegen::{MAX_MOVE_LIST_SIZE, Move, MoveFlag, MoveList},
	utils::{BLACK, Color, NUM_PIECES, Piece, WHITE},
};

// King penalty since we are scoring pseudo-legal moves
const MVV_LVA_MATERIAL_VALUES: [i32; NUM_PIECES] = [1, 3, 3, 5, 9, 20];

const fn mvv_lva_score(from_piece: usize, to_victim: usize) -> i32 {
	MVV_LVA_MATERIAL_VALUES[to_victim] * 64 - MVV_LVA_MATERIAL_VALUES[from_piece]
}

static MVV_LVA_LOOKUP: [[i32; NUM_PIECES]; NUM_PIECES] = {
	let mut table = [[0; NUM_PIECES]; NUM_PIECES];
	let mut from = 0;

	loop {
		if from >= 6 {
			break;
		}

		let mut to = 0;
		loop {
			if to >= 6 {
				break;
			}

			table[from][to] = mvv_lva_score(from, to);

			to += 1;
		}

		from += 1;
	}

	table
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveListStages {
	HashMove = 0,
	Captures = 1,
	Quiets = 2,
	Finished = 3,
}

impl MoveListStages {
	pub const fn next(self) -> Self {
		match self {
			Self::HashMove => Self::Captures,
			Self::Captures => Self::Quiets,
			Self::Quiets => Self::Finished,
			Self::Finished => Self::Finished,
		}
	}

	pub fn gen_moves<const C: u8, const OPP: u8>(
		self,
		board: &Board,
		buf: &mut MoveList,
		checkers_mask: u64,
	) {
		match self {
			Self::Captures => {
				board.gen_pseudo_legal_captures_in_check::<C, OPP>(buf, checkers_mask);
			}
			Self::Quiets => {
				board.gen_pseudo_legal_quiets_in_check::<C, OPP>(buf, checkers_mask);
			}
			_ => {}
		};
	}
}

pub struct StagedMoveList {
	cur_stage: MoveListStages,
	cur_move_list: MoveList,
	scores: [i32; MAX_MOVE_LIST_SIZE],
	current_move_idx: usize,
	hash_move: Option<Move>,
	only_captures: bool,
	checkers_mask: u64,
}

impl StagedMoveList {
	pub fn new<const C: u8, const OPP: u8>(
		hash_move: Option<Move>,
		board: &Board,
		only_captures: bool,
		checkers_mask: u64,
	) -> Self {
		let mut res = Self {
			cur_stage: MoveListStages::HashMove,
			cur_move_list: MoveList::default(),
			scores: [0; MAX_MOVE_LIST_SIZE],
			current_move_idx: 0,
			hash_move,
			only_captures,
			checkers_mask,
		};

		if let Some(hash_move) = hash_move
			&& !(only_captures && hash_move.is_quiet())
			&& board.is_pseudo_legal::<C, OPP>(&hash_move)
		{
			res.cur_move_list.push(hash_move);
		}
		res
	}

	fn score_move<const C: u8, const OPP: u8>(
		&self,
		board: &Board,
		m: &Move,
		history: Option<&[[[i16; 64]; 64]; 2]>,
		capture_history: &[[[i16; 64]; 6]; 6],
		killers: Option<&[Move; 2]>,
	) -> i32 {
		let (from, to, flags) = m.unpack();
		if m.is_promotion() {
			return 10000000;
		}

		match self.cur_stage {
			MoveListStages::HashMove => i32::MAX - 1,
			MoveListStages::Captures => {
				let from = board.get_piece_at_square::<C>(from).unwrap();
				let victim = board.get_piece_at_square::<OPP>(to).unwrap_or(Piece::Pawn);
				MVV_LVA_LOOKUP[from as usize][victim as usize] * 9
					+ capture_history[from as usize][victim as usize][to as usize] as i32
			}

			MoveListStages::Quiets => {
				if let Some(killers) = killers
					&& (*m == killers[0] || *m == killers[1])
				{
					return 1000000;
				}

				if let Some(history) = history {
					history[C as usize][from as usize][to as usize] as i32
				} else {
					0
				}
			}

			MoveListStages::Finished => 0,
		}
	}

	pub fn pick_move<const C: u8, const OPP: u8>(
		&mut self,
		board: &Board,
		history: Option<&[[[i16; 64]; 64]; 2]>,
		capture_history: &[[[i16; 64]; 6]; 6],
		killers: Option<&[Move; 2]>,
	) -> Option<&Move> {
		if self.cur_stage == MoveListStages::Finished {
			return None;
		}

		if self.current_move_idx >= self.cur_move_list.len() {
			self.cur_stage = self.cur_stage.next();
			if self.only_captures && self.cur_stage == MoveListStages::Quiets {
				self.cur_stage = self.cur_stage.next();
				return self.pick_move::<C, OPP>(board, history, capture_history, killers);
			}

			let old_list_len = self.cur_move_list.len();
			self.cur_stage
				.gen_moves::<C, OPP>(board, &mut self.cur_move_list, self.checkers_mask);

			for i in old_list_len..self.cur_move_list.len() {
				self.scores[i] = self.score_move::<C, OPP>(
					board,
					&self.cur_move_list.as_slice()[i],
					history,
					capture_history,
					killers,
				);
			}

			return self.pick_move::<C, OPP>(board, history, capture_history, killers);
		}

		let mut best_move_idx = self.current_move_idx;
		let mut best_score = i32::MIN + 1;

		for i in self.current_move_idx..self.cur_move_list.len() {
			if self.scores[i] > best_score {
				best_score = self.scores[i];
				best_move_idx = i;
			}
		}

		self.cur_move_list
			.as_mut_slice()
			.swap(self.current_move_idx, best_move_idx);
		self.scores.swap(self.current_move_idx, best_move_idx);
		self.current_move_idx += 1;
		let m = self.cur_move_list.as_slice()[self.current_move_idx - 1];
		if self.cur_stage != MoveListStages::HashMove
			&& let Some(hash_move) = self.hash_move
			&& hash_move == m
		{
			return self.pick_move::<C, OPP>(board, history, capture_history, killers);
		}

		Some(&self.cur_move_list.as_slice()[self.current_move_idx - 1])
	}

	pub fn stage(&self) -> MoveListStages {
		self.cur_stage
	}
}

const MAX_HISTORY: i16 = 16384;

pub fn update_history<const C: u8>(
	history: &mut [[[i16; 64]; 64]; 2],
	from: u8,
	to: u8,
	bonus: i16,
) {
	let clamped_bonus = bonus.clamp(-MAX_HISTORY, MAX_HISTORY);
	let history_val = &mut history[C as usize][from as usize][to as usize];
	// We need the casts to i32s to avoid overflowing
	*history_val += clamped_bonus
		- ((*history_val as i32 * clamped_bonus.abs() as i32) / MAX_HISTORY as i32) as i16;
}

pub fn update_capture_history(
	capture_history: &mut [[[i16; 64]; 6]; 6],
	from_piece: Piece,
	victim: Piece,
	to_square: u8,
	bonus: i16,
) {
	let clamped_bonus = bonus.clamp(-MAX_HISTORY, MAX_HISTORY);
	let history_val =
		&mut capture_history[from_piece as usize][victim as usize][to_square as usize];
	// We need the casts to i32s to avoid overflowing
	*history_val += clamped_bonus
		- ((*history_val as i32 * clamped_bonus.abs() as i32) / MAX_HISTORY as i32) as i16;
}
