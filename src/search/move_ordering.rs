use crate::board::{
	Board,
	movegen::{MAX_MOVE_LIST_SIZE, Move, MoveFlag, MoveList},
	utils::{BLACK, NUM_PIECES, Piece, WHITE},
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
	scores: [i32; MAX_MOVE_LIST_SIZE], // ToDo: use i16 scores in move ordering with properly
	// defined ranges
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
		history: &History,
		killers: Option<&[Move; 2]>,
	) -> i32 {
		if m.is_promotion() {
			return 10000000;
		}
		let (from, to, _) = m.unpack();

		let mut score = history.get::<C, OPP>(board, m) as i32;

		score += match self.cur_stage {
			MoveListStages::HashMove => i32::MAX - 1,
			MoveListStages::Captures => {
				let from = board.get_piece_at_square::<C>(from).unwrap();
				let victim = board.get_piece_at_square::<OPP>(to).unwrap_or(Piece::Pawn);
				MVV_LVA_LOOKUP[from as usize][victim as usize] * 9
			}

			MoveListStages::Quiets => {
				if let Some(killers) = killers
					&& (*m == killers[0] || *m == killers[1])
				{
					1000000
				} else {
					0
				}
			}

			MoveListStages::Finished => 0,
		};

		score
	}

	pub fn pick_move<const C: u8, const OPP: u8>(
		&mut self,
		board: &Board,
		history: &History,
		killers: Option<&[Move; 2]>,
	) -> Option<&Move> {
		if self.cur_stage == MoveListStages::Finished {
			return None;
		}

		if self.current_move_idx >= self.cur_move_list.len() {
			self.cur_stage = self.cur_stage.next();
			if self.only_captures && self.cur_stage == MoveListStages::Quiets {
				self.cur_stage = self.cur_stage.next();
				return self.pick_move::<C, OPP>(board, history, killers);
			}

			let old_list_len = self.cur_move_list.len();
			self.cur_stage
				.gen_moves::<C, OPP>(board, &mut self.cur_move_list, self.checkers_mask);

			for i in old_list_len..self.cur_move_list.len() {
				self.scores[i] = self.score_move::<C, OPP>(
					board,
					&self.cur_move_list.as_slice()[i],
					history,
					killers,
				);
			}

			return self.pick_move::<C, OPP>(board, history, killers);
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
			return self.pick_move::<C, OPP>(board, history, killers);
		}

		Some(&self.cur_move_list.as_slice()[self.current_move_idx - 1])
	}

	pub fn stage(&self) -> MoveListStages {
		self.cur_stage
	}
}

const MAX_HISTORY: i16 = 16384;

#[derive(Debug, Clone)]
pub struct History {
	quiet_history: [[[i16; 64]; 64]; 2],
	capture_history: [[[i16; 64]; 6]; 6],
}

impl Default for History {
	fn default() -> Self {
		Self {
			quiet_history: [[[0; 64]; 64]; 2],
			capture_history: [[[0; 64]; 6]; 6],
		}
	}
}

impl History {
	pub const MAX_HISTORY: i16 = 16384;

	pub fn update<const C: u8, const OPP: u8>(&mut self, board: &Board, m: &Move, bonus: i16) {
		if m.is_promotion() {
			return;
		}

		let (from_sq, to_sq, flags) = m.unpack();

		if m.is_quiet() {
			self.update_quiet_history::<C>(from_sq, to_sq, bonus);
		} else {
			let from_piece = board.get_piece_at_square::<C>(from_sq).unwrap();
			let victim = if flags == MoveFlag::EpCaptures {
				Piece::Pawn
			} else {
				board.get_piece_at_square::<OPP>(to_sq).unwrap()
			};

			self.update_capt_history(from_piece, victim, to_sq, bonus);
		}
	}

	pub fn age(&mut self) {
		for i in 0..64 {
			for j in 0..64 {
				self.quiet_history[WHITE as usize][i][j] =
					(self.quiet_history[WHITE as usize][i][j] as i32 * 3 / 4) as i16;
				self.quiet_history[BLACK as usize][i][j] =
					(self.quiet_history[BLACK as usize][i][j] as i32 * 3 / 4) as i16;
			}

			for j in 0..6 {
				for k in 0..6 {
					self.capture_history[j][k][i] =
						(self.capture_history[j][k][i] as i32 * 3 / 4) as i16;
				}
			}
		}
	}

	fn update_capt_history(&mut self, from_piece: Piece, victim: Piece, to_sq: u8, bonus: i16) {
		let bonus = bonus.clamp(-MAX_HISTORY, MAX_HISTORY);
		let entry = &mut self.capture_history[from_piece as usize][victim as usize][to_sq as usize];
		// We need the casts to i32s to avoid overflowing
		*entry += bonus - ((*entry as i32 * bonus.abs() as i32) / Self::MAX_HISTORY as i32) as i16;
	}

	fn update_quiet_history<const C: u8>(&mut self, from_sq: u8, to_sq: u8, bonus: i16) {
		let bonus = bonus.clamp(-MAX_HISTORY, MAX_HISTORY);
		let entry = &mut self.quiet_history[C as usize][from_sq as usize][to_sq as usize];
		// We need the casts to i32s to avoid overflowing
		*entry += bonus - ((*entry as i32 * bonus.abs() as i32) / Self::MAX_HISTORY as i32) as i16;
	}

	pub fn get<const C: u8, const OPP: u8>(&self, board: &Board, m: &Move) -> i16 {
		if m.is_promotion() {
			return Self::MAX_HISTORY;
		}

		let (from_sq, to_sq, flags) = m.unpack();

		if m.is_quiet() {
			self.quiet_history[C as usize][from_sq as usize][to_sq as usize]
		} else {
			let from_piece = board.get_piece_at_square::<C>(from_sq).unwrap();
			let victim = if flags == MoveFlag::EpCaptures {
				Piece::Pawn
			} else {
				board.get_piece_at_square::<OPP>(to_sq).unwrap()
			};

			self.capture_history[from_piece as usize][victim as usize][to_sq as usize]
		}
	}
}
