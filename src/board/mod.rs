mod attacks;
pub mod eval;
mod in_between;
pub mod movegen;
mod quiets;
mod sliding_attack_table;
mod state;
pub mod utils;
mod zobrist;

use std::{fmt::Display, num::NonZero};

use crate::board::{
	utils::{
		BLACK_SQUARES, Color, NUM_COLORS, NUM_PIECES, PIECES, Piece, PieceColorPair, Square,
		WHITE_SQUARES,
	},
	zobrist::ZobristDelta,
};

#[derive(Clone, Default)]
pub struct Board {
	pieces: [[u64; NUM_PIECES]; NUM_COLORS],
	pieces_by_color: [u64; NUM_COLORS],
	empty: u64,
	occupied: u64,

	en_passant: Option<NonZero<u8>>,
	king_castle_flags: [bool; 2],
	queen_castle_flags: [bool; 2],
	turn: Color,
	halfmove_clock: usize,

	zobrist: u64,
	hash_history: Vec<u64>,

	mg_material_score: i16,
	eg_material_score: i16,
	mg_pst_values: i16,
	eg_pst_values: i16,
	gamephase: u8,
	num_bishops: [u8; 2],
}

impl Board {
	pub const fn get_turn(&self) -> Color {
		self.turn
	}

	pub const fn get_opponent_bb<const C: u8>(&self) -> u64 {
		self.pieces_by_color[(C ^ 1) as usize]
	}

	pub const fn get_hash(&self) -> u64 {
		self.zobrist
	}

	pub fn has_non_pawn_material<const C: u8>(&self) -> bool {
		self.pieces[C as usize][Piece::Knight as usize] != 0
			|| self.pieces[C as usize][Piece::Bishop as usize] != 0
			|| self.pieces[C as usize][Piece::Rook as usize] != 0
			|| self.pieces[C as usize][Piece::Queen as usize] != 0
	}

	pub const fn get_piece_at_square<const C: u8>(&self, sq: u8) -> Option<Piece> {
		let mask = 1u64 << sq;
		if (self.pieces[C as usize][Piece::Pawn as usize] & mask) != 0 {
			return Some(Piece::Pawn);
		}

		if (self.pieces[C as usize][Piece::Knight as usize] & mask) != 0 {
			return Some(Piece::Knight);
		}

		if (self.pieces[C as usize][Piece::Bishop as usize] & mask) != 0 {
			return Some(Piece::Bishop);
		}

		if (self.pieces[C as usize][Piece::Rook as usize] & mask) != 0 {
			return Some(Piece::Rook);
		}

		if (self.pieces[C as usize][Piece::Queen as usize] & mask) != 0 {
			return Some(Piece::Queen);
		}

		if (self.pieces[C as usize][Piece::King as usize] & mask) != 0 {
			return Some(Piece::King);
		}

		None
	}

	pub fn from_fen(fen: &str) -> Option<Self> {
		let mut board = Self {
			..Default::default()
		};

		let mut pieces: [[u64; NUM_PIECES]; NUM_COLORS] = [[0; NUM_PIECES]; NUM_COLORS];

		let parts = fen.split_whitespace().collect::<Vec<_>>();

		let mut file = 0;
		let mut rank = 7;

		for c in parts[0].chars() {
			match c {
				'1' => file += 1,
				'2' => file += 2,
				'3' => file += 3,
				'4' => file += 4,
				'5' => file += 5,
				'6' => file += 6,
				'7' => file += 7,
				'8' => file += 8,
				'/' => {
					rank -= 1;
					file = 0;
				}
				_ => {
					let PieceColorPair(piece, color) = PieceColorPair::try_from(c).ok()?;
					let sq = Square::from_rank_file(rank, file).0;
					pieces[color as usize][piece as usize] |= 1u64 << sq;
					board.apply_zobrist_delta(ZobristDelta::PutRemove(
						PieceColorPair(piece, color),
						sq,
					));
					file += 1;
				}
			};

			if file == 8 {
				file = 0;
			}
		}

		let turn = match parts[1] {
			"w" => {
				board.apply_zobrist_delta(ZobristDelta::WhiteTurn);
				Color::White
			}
			"b" => Color::Black,
			_ => return None,
		};

		let mut king_castle_flags: [bool; 2] = [false; 2];
		let mut queen_castle_flags: [bool; 2] = [false; 2];
		for c in parts[2].chars() {
			match c {
				'K' => {
					king_castle_flags[0] = true;
					board.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::White));
				}
				'Q' => {
					queen_castle_flags[0] = true;
					board.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::White));
				}
				'k' => {
					king_castle_flags[1] = true;
					board.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::Black));
				}
				'q' => {
					queen_castle_flags[1] = true;
					board.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::Black));
				}
				_ => {}
			}
		}

		let en_passant: Option<NonZero<u8>> = match parts[3] {
			"-" => {
				board.apply_zobrist_delta(ZobristDelta::EnPassantSquareChange(0));
				None
			}
			_ => {
				let mut chars = parts[3].chars();
				let file = chars.next()?;
				let rank = chars.next()?;
				let Square(sq) = Square::from_rank_file_chars_ascii(rank, file);

				board.apply_zobrist_delta(ZobristDelta::EnPassantSquareChange(sq));
				NonZero::new(sq)
			}
		};

		let halfmove_clock = parts[4].parse::<usize>().ok()?;

		board.pieces = pieces;
		board.turn = turn;
		board.en_passant = en_passant;
		board.king_castle_flags = king_castle_flags;
		board.queen_castle_flags = queen_castle_flags;
		board.halfmove_clock = halfmove_clock;
		board.hash_history = Vec::with_capacity(256);
		board.hash_history.push(board.zobrist);
		board.gen_redundant_sets();
		board.init_evaluation();

		Some(board)
	}

	fn gen_redundant_sets(&mut self) {
		self.pieces_by_color[Color::White as usize] = self.pieces[Color::White as usize]
			.into_iter()
			.fold(0, |acc, e| acc | e);
		self.pieces_by_color[Color::Black as usize] = self.pieces[Color::Black as usize]
			.into_iter()
			.fold(0, |acc, e| acc | e);
		self.occupied = self.pieces_by_color[Color::White as usize]
			| self.pieces_by_color[Color::Black as usize];
		self.empty = !self.occupied;
	}

	pub fn as_piece_list(&self) -> Vec<Option<PieceColorPair>> {
		let mut res = vec![None; 64];
		for (sq, piece_col_pair) in res.iter_mut().enumerate() {
			let mask = 1u64 << sq;
			if (self.empty & mask) != 0 {
				continue;
			}

			if (self.pieces_by_color[Color::White as usize] & mask) != 0 {
				for piece in PIECES {
					if (self.pieces[Color::White as usize][piece as usize] & mask) != 0 {
						*piece_col_pair = Some(PieceColorPair(piece, Color::White));
						break;
					}
				}
			} else if (self.pieces_by_color[Color::Black as usize] & mask) != 0 {
				for piece in PIECES {
					if (self.pieces[Color::Black as usize][piece as usize] & mask) != 0 {
						*piece_col_pair = Some(PieceColorPair(piece, Color::Black));
						break;
					}
				}
			}
		}
		res
	}

	// Only one repetition for draw score
	pub fn detect_repetition(&self) -> bool {
		let mut relevant_hash_history = self
			.hash_history
			.iter()
			.rev()
			.step_by(2)
			.take(self.halfmove_clock.div_ceil(2));
		relevant_hash_history.next();
		for h in relevant_hash_history {
			if *h == self.zobrist {
				return true;
			}
		}

		false
	}

	pub fn fifty_moves_rule(&self) -> bool {
		self.halfmove_clock >= 100
	}

	pub fn get_halfmove_clock(&self) -> usize {
		self.halfmove_clock
	}

	pub fn draw_by_insufficient_material(&self) -> bool {
		/*// Check if no pawns since gamephase ignores pawns
		if (self.pieces[Color::White as usize][Piece::Pawn as usize]
			| self.pieces[Color::Black as usize][Piece::Pawn as usize])
			!= 0
		{
			return false;
		}

		// The position is a draw by insufficient material if
		// 1. Only kings on the board
		if self.gamephase == 0 {
			return true;
		}

		// 2. Only one minor piece on the board
		if (self.gamephase == GAMEPHASE_INCREMENTS[Piece::Bishop as usize])
			|| (self.gamephase == GAMEPHASE_INCREMENTS[Piece::Knight as usize])
		{
			return true;
		}

		// 3. Only two bishops of the same color
		if self.gamephase == 2 * GAMEPHASE_INCREMENTS[Piece::Bishop as usize]
			&& (self.pieces[Color::White as usize][Piece::Rook as usize]
				| self.pieces[Color::Black as usize][Piece::Rook as usize])
				== 0
		{
			let bishops = self.pieces[Color::White as usize][Piece::Bishop as usize]
				| self.pieces[Color::Black as usize][Piece::Bishop as usize];
			if self.pieces[Color::White as usize][Piece::Bishop as usize] == 0
				|| self.pieces[Color::Black as usize][Piece::Bishop as usize] == 0
			{
				return false;
			}

			return ((bishops & WHITE_SQUARES) == bishops)
				|| ((bishops & BLACK_SQUARES) == bishops);
		}*/

		if (self.pieces[Color::White as usize][Piece::Pawn as usize]
			| self.pieces[Color::Black as usize][Piece::Pawn as usize]
			| self.pieces[Color::White as usize][Piece::Rook as usize]
			| self.pieces[Color::Black as usize][Piece::Rook as usize]
			| self.pieces[Color::White as usize][Piece::Queen as usize]
			| self.pieces[Color::Black as usize][Piece::Queen as usize])
			!= 0
		{
			return false;
		}

		let wbishops = self.pieces[Color::White as usize][Piece::Bishop as usize];
		let bbishops = self.pieces[Color::Black as usize][Piece::Bishop as usize];
		let wknights = self.pieces[Color::White as usize][Piece::Knight as usize];
		let bknights = self.pieces[Color::Black as usize][Piece::Knight as usize];

		match (
			wbishops.count_ones(),
			bbishops.count_ones(),
			wknights.count_ones(),
			bknights.count_ones(),
		) {
			(0, 0, 0, 0) => true,
			(1, 0, 0, 0) => true,
			(0, 1, 0, 0) => true,
			(0, 0, 1, 0) => true,
			(0, 0, 0, 1) => true,
			(2, 0, 0, 0) => {
				((wbishops & WHITE_SQUARES) == wbishops) || ((wbishops & BLACK_SQUARES) == wbishops)
			}
			(0, 2, 0, 0) => {
				((bbishops & WHITE_SQUARES) == bbishops) || ((bbishops & BLACK_SQUARES) == bbishops)
			}
			(1, 1, 0, 0) => {
				let bishops = wbishops | bbishops;
				((bishops & WHITE_SQUARES) == bishops) || ((bishops & BLACK_SQUARES) == bishops)
			}
			_ => false,
		}
	}
}

impl Display for Board {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let piece_list = self.as_piece_list();

		writeln!(f, "  A B C D E F G H")?;
		for rank in (0..8).rev() {
			write!(f, "{} ", rank + 1)?;
			for file in 0..8 {
				let Square(sq) = Square::from_rank_file(rank, file);
				match piece_list[sq as usize] {
					None => write!(f, "  ")?,
					Some(pair) => write!(f, "{} ", <PieceColorPair as Into<char>>::into(pair))?,
				};
			}
			writeln!(f, "{}", rank + 1)?;
		}
		writeln!(f, "  A B C D E F G H")?;

		Ok(())
	}
}
