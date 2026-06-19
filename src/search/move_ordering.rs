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

pub struct OrderedMoveList<'a> {
	moves: &'a mut [Move],
	scores: [i32; MAX_MOVE_LIST_SIZE],
	current_move_idx: usize,
}

impl<'a> OrderedMoveList<'a> {
	pub fn from_move_list(board: &Board, move_list: &'a mut MoveList) -> OrderedMoveList<'a> {
		let mut res = Self {
			moves: move_list.as_mut_slice(),
			scores: [0; MAX_MOVE_LIST_SIZE],
			current_move_idx: 0,
		};

		for i in 0..res.moves.len() {
			res.scores[i] = match board.get_turn() {
				Color::White => Self::score_move::<WHITE>(board, &res.moves[i]),
				Color::Black => Self::score_move::<BLACK>(board, &res.moves[i]),
			};
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
