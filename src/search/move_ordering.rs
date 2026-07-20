use crate::board::{
	Board,
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
	WinningCaptures = 1,
	Quiets = 2,
	Finished = 3,
}

impl MoveListStages {
	#[inline(always)]
	pub const fn next(self) -> Self {
		match self {
			Self::HashMove => Self::WinningCaptures,
			Self::WinningCaptures => Self::Quiets,
			Self::Quiets => Self::Finished,
			Self::Finished => Self::Finished,
		}
	}

	#[inline(always)]
	pub fn gen_moves<const C: u8>(self, board: &Board, buf: &mut MoveList) {
		match self {
			Self::WinningCaptures => {
				board.gen_pseudo_legal_captures_in_check::<C>(buf);
			}
			Self::Quiets => {
				board.gen_pseudo_legal_quiets_in_check::<C>(buf);
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
}

impl StagedMoveList {
	pub fn new<const C: u8>(hash_move: Option<Move>, board: &Board, only_captures: bool) -> Self {
		let mut res = Self {
			cur_stage: MoveListStages::HashMove,
			cur_move_list: MoveList::default(),
			scores: [0; MAX_MOVE_LIST_SIZE],
			current_move_idx: 0,
			hash_move,
			only_captures,
		};

		if let Some(hash_move) = hash_move
			&& board.is_pseudo_legal::<C>(&hash_move)
		{
			res.cur_move_list.push(hash_move);
		}

		res
	}

	fn score_move<const C: u8>(
		&self,
		board: &Board,
		m: &Move,
		history: Option<&[[[i16; 64]; 64]; 2]>,
		killers: Option<&[Move; 2]>,
	) -> i32 {
		let (from, to, flags) = m.unpack();
		if flags == MoveFlag::EpCaptures {
			return MVV_LVA_LOOKUP[Piece::Pawn as usize][Piece::Pawn as usize];
		}

		match self.cur_stage {
			MoveListStages::HashMove => i32::MAX - 1,
			MoveListStages::WinningCaptures => {
				let from = board.get_piece_at_square::<C>(from).unwrap();
				// Not E.P. thanks to short-circuit above. So we must have a victim on the to square
				let victim = match C ^ 1 {
					WHITE => board.get_piece_at_square::<WHITE>(to).unwrap(),
					BLACK => board.get_piece_at_square::<BLACK>(to).unwrap(),
					_ => Piece::Pawn,
				};

				MVV_LVA_LOOKUP[from as usize][victim as usize]
			}

			MoveListStages::Quiets => {
				let mut score = 0;
				if m.is_promotion() {
					score += 1000000;
				}

				if let Some(killers) = killers {
					if *m == killers[0] || *m == killers[1] {
						score += 1000000;
					}
				}

				if let Some(history) = history {
					score += history[C as usize][from as usize][to as usize] as i32;
				}

				score
			}

			MoveListStages::Finished => 0,
		}
	}

	pub fn pick_move<const C: u8>(
		&mut self,
		board: &Board,
		history: Option<&[[[i16; 64]; 64]; 2]>,
		killers: Option<&[Move; 2]>,
	) -> Option<&Move> {
		if self.cur_stage == MoveListStages::Finished {
			return None;
		}

		if self.current_move_idx >= self.cur_move_list.len() {
			self.cur_stage = self.cur_stage.next();
			if self.only_captures && self.cur_stage == MoveListStages::Quiets {
				self.cur_stage = self.cur_stage.next();
			}

			let old_list_len = self.cur_move_list.len();
			self.cur_stage
				.gen_moves::<C>(board, &mut self.cur_move_list);

			for i in old_list_len..self.cur_move_list.len() {
				self.scores[i] = match board.get_turn() {
					Color::White => self.score_move::<WHITE>(
						board,
						&self.cur_move_list.as_slice()[i],
						history,
						killers,
					),
					Color::Black => self.score_move::<BLACK>(
						board,
						&self.cur_move_list.as_slice()[i],
						history,
						killers,
					),
				};
			}

			return self.pick_move::<C>(board, history, killers);
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
			return self.pick_move::<C>(board, history, killers);
		}

		Some(&self.cur_move_list.as_slice()[self.current_move_idx - 1])
	}

	pub fn stage(&self) -> MoveListStages {
		self.cur_stage
	}
}

pub struct OrderedMoveList<'a> {
	moves: &'a mut [Move],
	scores: [i32; MAX_MOVE_LIST_SIZE],
	current_move_idx: usize,
}

impl<'a> OrderedMoveList<'a> {
	pub fn from_move_list(
		board: &Board,
		move_list: &'a mut MoveList,
		hash_move: Option<Move>,
	) -> OrderedMoveList<'a> {
		let mut res = Self {
			moves: move_list.as_mut_slice(),
			scores: [0; MAX_MOVE_LIST_SIZE],
			current_move_idx: 0,
		};

		for i in 0..res.moves.len() {
			res.scores[i] = match board.get_turn() {
				Color::White => Self::score_move::<WHITE>(board, &res.moves[i], hash_move),
				Color::Black => Self::score_move::<BLACK>(board, &res.moves[i], hash_move),
			};
		}

		res
	}

	fn score_move<const C: u8>(board: &Board, m: &Move, hash_move: Option<Move>) -> i32 {
		if let Some(hash_move) = hash_move {
			if hash_move == *m {
				return i32::MAX - 1;
			}
		}

		let (from, to, flags) = m.unpack();

		match flags {
			MoveFlag::Captures
			| MoveFlag::KnightPromoCapture
			| MoveFlag::BishopPromoCapture
			| MoveFlag::RookPromoCapture
			| MoveFlag::QueenPromoCapture => {
				let from = board.get_piece_at_square::<C>(from).unwrap();
				let victim = match C ^ 1 {
					WHITE => board.get_piece_at_square::<WHITE>(to).unwrap(),
					BLACK => board.get_piece_at_square::<BLACK>(to).unwrap(),
					_ => Piece::Pawn,
				};

				MVV_LVA_LOOKUP[from as usize][victim as usize]
			}

			_ => 0,
		}
	}

	pub fn pick_move(&mut self, board: &Board) -> Option<&Move> {
		if self.current_move_idx >= self.moves.len() {
			return None;
		}

		let mut best_move_idx = self.current_move_idx;
		let mut best_score = i32::MIN + 1;

		for i in self.current_move_idx..self.moves.len() {
			if self.scores[i] > best_score {
				best_score = self.scores[i];
				best_move_idx = i;
			}
		}

		self.moves.swap(self.current_move_idx, best_move_idx);
		self.scores.swap(self.current_move_idx, best_move_idx);
		self.current_move_idx += 1;
		Some(&self.moves[self.current_move_idx - 1])
	}
}

const MAX_HISTORY: i16 = 24576;

#[inline(always)]
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
