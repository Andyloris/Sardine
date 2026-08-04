use crate::board::{
	Board,
	attacks::{get_king_attacks, get_knight_attacks, get_pawn_attacks, get_sliding_attacks},
	utils::{BLACK, Color, NUM_PIECES, PIECES, Piece, PieceColorPair, WHITE, clear_lsb},
};

const MG_MATERIAL_WEIGHTS: [i16; NUM_PIECES] = [82, 337, 365, 477, 1025, 0];
const EG_MATERIAL_WEIGHTS: [i16; NUM_PIECES] = [94, 281, 297, 512, 936, 0];

pub const MATERIAL_WEIGHTS: [i16; NUM_PIECES] = {
	let mut res: [i16; NUM_PIECES] = [0; NUM_PIECES];
	let mut p = 0;
	loop {
		if p >= NUM_PIECES {
			break;
		}

		res[p] = (MG_MATERIAL_WEIGHTS[p] * 4 + EG_MATERIAL_WEIGHTS[p]) / 5;
		p += 1;
	}
	res
};

const MG_PAWN_TABLE: [i16; 64] = [
	0, 0, 0, 0, 0, 0, 0, 0, 98, 134, 61, 95, 68, 126, 34, -11, -6, 7, 26, 31, 65, 56, 25, -20, -14,
	13, 6, 21, 23, 12, 17, -23, -27, -2, -5, 12, 17, 6, 10, -25, -26, -4, -4, -10, 3, 3, 33, -12,
	-35, -1, -20, -23, -15, 24, 38, -22, 0, 0, 0, 0, 0, 0, 0, 0,
];

const EG_PAWN_TABLE: [i16; 64] = [
	0, 0, 0, 0, 0, 0, 0, 0, 178, 173, 158, 134, 147, 132, 165, 187, 94, 100, 85, 67, 56, 53, 82,
	84, 32, 24, 13, 5, -2, 4, 17, 17, 13, 9, -3, -7, -7, -8, 3, -1, 4, 7, -6, 1, 0, -5, -1, -8, 13,
	8, 8, 10, 13, 0, 2, -7, 0, 0, 0, 0, 0, 0, 0, 0,
];

const MG_KNIGHT_TABLE: [i16; 64] = [
	-167, -89, -34, -49, 61, -97, -15, -107, -73, -41, 72, 36, 23, 62, 7, -17, -47, 60, 37, 65, 84,
	129, 73, 44, -9, 17, 19, 53, 37, 69, 18, 22, -13, 4, 16, 13, 28, 19, 21, -8, -23, -9, 12, 10,
	19, 17, 25, -16, -29, -53, -12, -3, -1, 18, -14, -19, -105, -21, -58, -33, -17, -28, -19, -23,
];

const EG_KNIGHT_TABLE: [i16; 64] = [
	-58, -38, -13, -28, -31, -27, -56, -99, -25, -8, -25, -2, -9, -25, -24, -52, -24, -20, 10, 9,
	-1, -9, -19, -41, -17, 3, 22, 22, 22, 11, 8, -18, -18, -6, 16, 25, 16, 17, 4, -18, -23, -3, -1,
	15, 10, -3, -20, -22, -42, -20, -10, -5, -2, -20, -23, -44, -29, -51, -23, -15, -22, -18, -50,
	-64,
];

const MG_BISHOP_TABLE: [i16; 64] = [
	-29, 4, -82, -37, -25, -42, 7, -8, -26, 16, -18, -13, 30, 59, 18, -47, -16, 37, 43, 40, 35, 50,
	37, -2, -4, 5, 19, 50, 37, 37, 7, -2, -6, 13, 13, 26, 34, 12, 10, 4, 0, 15, 15, 15, 14, 27, 18,
	10, 4, 15, 16, 0, 7, 21, 33, 1, -33, -3, -14, -21, -13, -12, -39, -21,
];

const EG_BISHOP_TABLE: [i16; 64] = [
	-14, -21, -11, -8, -7, -9, -17, -24, -8, -4, 7, -12, -3, -13, -4, -14, 2, -8, 0, -1, -2, 6, 0,
	4, -3, 9, 12, 9, 14, 10, 3, 2, -6, 3, 13, 19, 7, 10, -3, -9, -12, -3, 8, 10, 13, 3, -7, -15,
	-14, -18, -7, -1, 4, -9, -15, -27, -23, -9, -23, -5, -9, -16, -5, -17,
];

const MG_ROOK_TABLE: [i16; 64] = [
	32, 42, 32, 51, 56, 9, 31, 43, 27, 32, 58, 62, 80, 67, 26, 44, -5, 19, 26, 36, 17, 45, 61, 16,
	-24, -11, 7, 26, 24, 35, -8, -20, -36, -26, -12, -1, 9, -7, 6, -23, -45, -25, -16, -17, 3, 0,
	-5, -33, -44, -16, -20, -9, -1, 11, -6, -71, -19, -13, 1, 17, 16, 7, -37, -26,
];

const EG_ROOK_TABLE: [i16; 64] = [
	13, 10, 18, 15, 12, 12, 8, 5, 11, 13, 13, 11, -3, 3, 8, 3, 7, 7, 7, 5, 4, -3, -5, -3, 4, 3, 13,
	1, 2, 1, -1, 2, 3, 5, 8, 4, -5, -6, -8, -11, -4, 0, -5, -1, -7, -12, -8, -16, -6, -6, 0, 2, -9,
	-9, -11, -3, -9, 2, 3, -1, -5, -13, 4, -20,
];

const MG_QUEEN_TABLE: [i16; 64] = [
	-28, 0, 29, 12, 59, 44, 43, 45, -24, -39, -5, 1, -16, 57, 28, 54, -13, -17, 7, 8, 29, 56, 47,
	57, -27, -27, -16, -16, -1, 17, -2, 1, -9, -26, -9, -10, -2, -4, 3, -3, -14, 2, -11, -2, -5, 2,
	14, 5, -35, -8, 11, 2, 8, 15, -3, 1, -1, -18, -9, 10, -15, -25, -31, -50,
];

const EG_QUEEN_TABLE: [i16; 64] = [
	-9, 22, 22, 27, 27, 19, 10, 20, -17, 20, 32, 41, 58, 25, 30, 0, -20, 6, 9, 49, 47, 35, 19, 9,
	3, 22, 24, 45, 57, 40, 57, 36, -18, 28, 19, 47, 31, 34, 39, 23, -16, -27, 15, 6, 9, 17, 10, 5,
	-22, -23, -30, -16, -16, -23, -36, -32, -33, -28, -22, -43, -5, -32, -20, -41,
];

const MG_KING_TABLE: [i16; 64] = [
	-65, 23, 16, -15, -56, -34, 2, 13, 29, -1, -20, -7, -8, -4, -38, -29, -9, 24, 2, -16, -20, 6,
	22, -22, -17, -20, -12, -27, -30, -25, -14, -36, -49, -1, -27, -39, -46, -44, -33, -51, -14,
	-14, -22, -46, -44, -30, -15, -27, 1, 7, -8, -64, -43, -16, 9, 8, -15, 36, 12, -54, 8, -28, 24,
	14,
];

const EG_KING_TABLE: [i16; 64] = [
	-74, -35, -18, -18, -11, 15, 4, -17, -12, 17, 14, 17, 17, 38, 23, 11, 10, 17, 23, 15, 20, 45,
	44, 13, -8, 22, 24, 27, 26, 33, 26, 3, -18, -4, 21, 24, 27, 23, 9, -11, -19, -3, 11, 21, 23,
	16, 7, -9, -27, -11, 4, 13, 14, 4, -5, -17, -53, -34, -21, -11, -28, -14, -24, -43,
];

static MG_PSTS: [[i16; 64]; 6] = [
	MG_PAWN_TABLE,
	MG_KNIGHT_TABLE,
	MG_BISHOP_TABLE,
	MG_ROOK_TABLE,
	MG_QUEEN_TABLE,
	MG_KING_TABLE,
];

static EG_PSTS: [[i16; 64]; 6] = [
	EG_PAWN_TABLE,
	EG_KNIGHT_TABLE,
	EG_BISHOP_TABLE,
	EG_ROOK_TABLE,
	EG_QUEEN_TABLE,
	EG_KING_TABLE,
];

pub(super) const GAMEPHASE_INCREMENTS: [u8; 6] = [0, 1, 1, 2, 4, 0];

impl Board {
	pub(super) fn init_evaluation(&mut self) {
		for piece in PIECES {
			let mut w_bb = self.pieces[WHITE as usize][piece as usize];
			let mut b_bb = self.pieces[BLACK as usize][piece as usize];

			while w_bb != 0 {
				let sq = w_bb.trailing_zeros();
				clear_lsb(&mut w_bb);

				self.mg_material_score += MG_MATERIAL_WEIGHTS[piece as usize];
				self.eg_material_score += EG_MATERIAL_WEIGHTS[piece as usize];
				self.mg_pst_values += MG_PSTS[piece as usize][sq as usize ^ 56];
				self.eg_pst_values += EG_PSTS[piece as usize][sq as usize ^ 56];

				self.gamephase += GAMEPHASE_INCREMENTS[piece as usize];
			}

			while b_bb != 0 {
				let sq = b_bb.trailing_zeros();
				clear_lsb(&mut b_bb);

				self.mg_material_score -= MG_MATERIAL_WEIGHTS[piece as usize];
				self.eg_material_score -= EG_MATERIAL_WEIGHTS[piece as usize];
				self.mg_pst_values -= MG_PSTS[piece as usize][sq as usize];
				self.eg_pst_values -= EG_PSTS[piece as usize][sq as usize];

				self.gamephase += GAMEPHASE_INCREMENTS[piece as usize];
			}
		}
	}

	pub(super) fn evaluation_add_piece(&mut self, sq: u8, piece_color: PieceColorPair) {
		let PieceColorPair(piece, color) = piece_color;
		match color {
			Color::White => {
				self.mg_material_score += MG_MATERIAL_WEIGHTS[piece as usize];
				self.eg_material_score += EG_MATERIAL_WEIGHTS[piece as usize];
				self.mg_pst_values += MG_PSTS[piece as usize][sq as usize ^ 56];
				self.eg_pst_values += EG_PSTS[piece as usize][sq as usize ^ 56];
			}

			Color::Black => {
				self.mg_material_score -= MG_MATERIAL_WEIGHTS[piece as usize];
				self.eg_material_score -= EG_MATERIAL_WEIGHTS[piece as usize];
				self.mg_pst_values -= MG_PSTS[piece as usize][sq as usize];
				self.eg_pst_values -= EG_PSTS[piece as usize][sq as usize];
			}
		};

		self.gamephase += GAMEPHASE_INCREMENTS[piece as usize];
	}

	pub(super) fn evaluation_remove_piece(&mut self, sq: u8, piece_color: PieceColorPair) {
		let PieceColorPair(piece, color) = piece_color;
		match color {
			Color::White => {
				self.mg_material_score -= MG_MATERIAL_WEIGHTS[piece as usize];
				self.eg_material_score -= EG_MATERIAL_WEIGHTS[piece as usize];
				self.mg_pst_values -= MG_PSTS[piece as usize][sq as usize ^ 56];
				self.eg_pst_values -= EG_PSTS[piece as usize][sq as usize ^ 56];
			}

			Color::Black => {
				self.mg_material_score += MG_MATERIAL_WEIGHTS[piece as usize];
				self.eg_material_score += EG_MATERIAL_WEIGHTS[piece as usize];
				self.mg_pst_values += MG_PSTS[piece as usize][sq as usize];
				self.eg_pst_values += EG_PSTS[piece as usize][sq as usize];
			}
		};

		self.gamephase -= GAMEPHASE_INCREMENTS[piece as usize];
	}

	pub(super) fn evaluation_move_piece(&mut self, from: u8, to: u8, piece_color: PieceColorPair) {
		let PieceColorPair(piece, color) = piece_color;
		match color {
			Color::White => {
				self.mg_pst_values -= MG_PSTS[piece as usize][from as usize ^ 56];
				self.eg_pst_values -= EG_PSTS[piece as usize][from as usize ^ 56];

				self.mg_pst_values += MG_PSTS[piece as usize][to as usize ^ 56];
				self.eg_pst_values += EG_PSTS[piece as usize][to as usize ^ 56];
			}

			Color::Black => {
				self.mg_pst_values += MG_PSTS[piece as usize][from as usize];
				self.eg_pst_values += EG_PSTS[piece as usize][from as usize];

				self.mg_pst_values -= MG_PSTS[piece as usize][to as usize];
				self.eg_pst_values -= EG_PSTS[piece as usize][to as usize];
			}
		}
	}

	const MAX_GAMEPHASE: i32 = 16 * GAMEPHASE_INCREMENTS[Piece::Pawn as usize] as i32
		+ 4 * GAMEPHASE_INCREMENTS[Piece::Knight as usize] as i32
		+ 4 * GAMEPHASE_INCREMENTS[Piece::Bishop as usize] as i32
		+ 4 * GAMEPHASE_INCREMENTS[Piece::Rook as usize] as i32
		+ 2 * GAMEPHASE_INCREMENTS[Piece::Queen as usize] as i32
		+ 2 * GAMEPHASE_INCREMENTS[Piece::King as usize] as i32;

	const TEMPO_BONUS: i32 = 13;
	pub fn eval_objective(&self) -> i16 {
		let mg_weight = self.gamephase.min(Self::MAX_GAMEPHASE as u8) as i32;
		let eg_weight = Self::MAX_GAMEPHASE - mg_weight;

		let mg_material = self.mg_pst_values as i32 + self.mg_material_score as i32;
		let eg_material = self.eg_pst_values as i32 + self.eg_material_score as i32;

		// Tapered eval
		let material = (mg_weight * mg_material + eg_weight * eg_material) / Self::MAX_GAMEPHASE;

		let tempo = match self.turn {
			Color::White => Self::TEMPO_BONUS,
			Color::Black => -Self::TEMPO_BONUS,
		};

		let mut eval = material as i16 + tempo as i16;

		// Scale to zero when close to 50 moves rule
		if self.halfmove_clock >= 80 {
			eval = ((eval as i32 * (100 - self.halfmove_clock.min(100) as i32)) / 20) as i16;
		}

		// Evaluation rounding to improve alpha-beta effectiveness while ignoring subtle and noisy
		// positional differences
		(eval / 4) * 4
	}

	// SEE stuff from https://www.chessprogramming.org/SEE_-_The_Swap_Algorithm
	pub fn attacks_to_square_with_occ<const BY: u8>(&self, sq: u8, occ: &u64) -> u64 {
		let mut attackers = 0;
		let pawns = self.pieces[BY as usize][Piece::Pawn as usize];
		attackers |= match BY ^ 1 {
			WHITE => get_pawn_attacks::<WHITE>(sq) & pawns,
			BLACK => get_pawn_attacks::<BLACK>(sq) & pawns,
			_ => 0,
		};

		let knights = self.pieces[BY as usize][Piece::Knight as usize];
		attackers |= get_knight_attacks(sq) & knights;

		let king = self.pieces[BY as usize][Piece::King as usize];
		attackers |= get_king_attacks(sq) & king;

		let bishops_queens = self.pieces[BY as usize][Piece::Bishop as usize]
			| self.pieces[BY as usize][Piece::Queen as usize];
		attackers |= get_sliding_attacks::<true>(sq, *occ) & bishops_queens;

		let rooks_queens = self.pieces[BY as usize][Piece::Rook as usize]
			| self.pieces[BY as usize][Piece::Queen as usize];
		attackers |= get_sliding_attacks::<false>(sq, *occ) & rooks_queens;

		attackers
	}

	fn get_least_valuable_piece(&self, attadef: u64, by_side: Color, out_piece: &mut Piece) -> u64 {
		for piece in PIECES {
			let subset = attadef & self.pieces[by_side as usize][piece as usize];
			if subset != 0 {
				*out_piece = piece;
				return subset & subset.wrapping_neg();
			}
		}

		0
	}

	pub fn see(&self, from: u8, to: u8, target: Piece, mut a_piece: Piece) -> i16 {
		let mut gain: [i16; 32] = [0; 32];
		let mut depth = 0;
		let mut from_set = 1u64 << from;
		let mut occ = self.occupied;
		gain[0] = MATERIAL_WEIGHTS[target as usize];
		let mut attadef;

		loop {
			depth += 1;
			gain[depth] = MATERIAL_WEIGHTS[a_piece as usize] - gain[depth - 1];
			occ ^= from_set;
			if depth % 2 == 0 {
				attadef = self.attacks_to_square_with_occ::<WHITE>(to, &occ) & occ;
				from_set = self.get_least_valuable_piece(attadef, Color::White, &mut a_piece);
			} else {
				attadef = self.attacks_to_square_with_occ::<BLACK>(to, &occ) & occ;
				from_set = self.get_least_valuable_piece(attadef, Color::Black, &mut a_piece);
			}

			if from_set == 0 {
				break;
			}
		}

		for d in (1..depth).rev() {
			gain[d - 1] = -(gain[d].max(-gain[d - 1]));
		}

		gain[0]
	}
}
