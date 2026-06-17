mod attacks;
pub mod movegen;
mod quiets;
mod sliding_attack_table;
mod state;
pub mod utils;
mod zobrist;

use std::{fmt::Display, num::NonZero};

use crate::board::{
	utils::{Color, NUM_COLORS, NUM_PIECES, PIECES, Piece, PieceColorPair, Square},
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
	halfmove_clock: u8,

	zobrist: u64,
}

impl Board {
	pub const fn get_turn(&self) -> Color {
		self.turn
	}

	pub const fn get_friendly_bb<const C: u8>(&self) -> u64 {
		self.pieces_by_color[C as usize]
	}

	pub const fn get_opponent_bb<const C: u8>(&self) -> u64 {
		self.pieces_by_color[(C ^ 1) as usize]
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

		let halfmove_clock = parts[4].parse::<u8>().ok()?;

		board.pieces = pieces;
		board.turn = turn;
		board.en_passant = en_passant;
		board.king_castle_flags = king_castle_flags;
		board.queen_castle_flags = queen_castle_flags;
		board.halfmove_clock = halfmove_clock;
		board.gen_redundant_sets();
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
