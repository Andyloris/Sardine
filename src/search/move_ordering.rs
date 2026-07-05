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

// ToDo: Staged move generation with pseudo-legal checking for the hash move

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveListStages {
	HashMove = 0,
	WinningCaptures = 1,
	Quiets = 2,
	LosingCaptures = 3,
	Finished = 4,
}

impl MoveListStages {
	#[inline(always)]
	pub const fn next(self) -> Self {
		match self {
			Self::HashMove => Self::WinningCaptures,
			Self::WinningCaptures => Self::Quiets,
			Self::Quiets => Self::LosingCaptures,
			Self::LosingCaptures => Self::Finished,
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

	fn score_move<const C: u8>(board: &Board, m: &Move) -> i32 {
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

	pub fn pick_move<const C: u8>(&mut self, board: &Board) -> Option<&Move> {
		if self.cur_stage == MoveListStages::Finished {
			return None;
		}

		if self.current_move_idx >= self.cur_move_list.len() {
			self.cur_stage = self.cur_stage.next();
			if self.only_captures && self.cur_stage == MoveListStages::Quiets {
				self.cur_stage = self.cur_stage.next();
			}

			self.cur_stage
				.gen_moves::<C>(board, &mut self.cur_move_list);
			for i in self.current_move_idx..self.cur_move_list.len() {
				self.scores[i] = match board.get_turn() {
					Color::White => {
						Self::score_move::<WHITE>(board, &self.cur_move_list.as_slice()[i])
					}
					Color::Black => {
						Self::score_move::<BLACK>(board, &self.cur_move_list.as_slice()[i])
					}
				};
			}

			return self.pick_move::<C>(board);
		}

		let mut best_move_idx = self.current_move_idx;
		let mut best_score = i32::MIN + 1;

		for i in self.current_move_idx..self.cur_move_list.len() {
			if self.scores[i] > best_score {
				best_score = self.scores[i];
				best_move_idx = i;
			}
		}

		// Sort quiet moves before losing captures
		if (self.cur_stage == MoveListStages::WinningCaptures) && (best_score < 0) {
			self.cur_stage = self.cur_stage.next();
			if self.only_captures && self.cur_stage == MoveListStages::Quiets {
				self.cur_stage = self.cur_stage.next();
				return self.pick_move::<C>(board);
			}

			// Only rescore quiets
			let old_list_len = self.cur_move_list.len();
			self.cur_stage
				.gen_moves::<C>(board, &mut self.cur_move_list);

			for i in old_list_len..self.cur_move_list.len() {
				self.scores[i] = match board.get_turn() {
					Color::White => {
						Self::score_move::<WHITE>(board, &self.cur_move_list.as_slice()[i])
					}
					Color::Black => {
						Self::score_move::<BLACK>(board, &self.cur_move_list.as_slice()[i])
					}
				};
			}

			return self.pick_move::<C>(board);
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
			return self.pick_move::<C>(board);
		}

		Some(&self.cur_move_list.as_slice()[self.current_move_idx - 1])
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
