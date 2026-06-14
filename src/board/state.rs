use crate::board::{
	Board,
	movegen::{Move, MoveFlag},
	utils::{BLACK, Color, Piece, Squares, WHITE, direction},
};

impl Board {
	pub fn do_move<const C: u8>(&mut self, m: &Move) -> Option<()> {
		let (from, to, flags) = m.unpack();
		let mut should_reset_enpassant = true;
		self.halfmove_clock += 1;

		match flags {
			MoveFlag::Quiet => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let piece = self.get_piece_at_square::<C>(from)?;

				if piece == Piece::King {
					self.king_castle_flags[C as usize] = false;
					self.queen_castle_flags[C as usize] = false;
				}

				if (piece == Piece::Rook)
					&& (from
						== match C {
							WHITE => Squares::H1 as u8,
							BLACK => Squares::H8 as u8,
							_ => 0,
						}) {
					self.king_castle_flags[C as usize] = false;
				} else if (piece == Piece::Rook)
					&& (from
						== match C {
							WHITE => Squares::A1 as u8,
							BLACK => Squares::A8 as u8,
							_ => 0,
						}) {
					self.queen_castle_flags[C as usize] = false;
				}

				self.pieces[C as usize][piece as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;

				if piece == Piece::Pawn {
					self.halfmove_clock = 0;
				}
			}

			MoveFlag::DoublePush => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				self.pieces[C as usize][Piece::Pawn as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;
				let en_passant_off = match C {
					WHITE => direction::S,
					BLACK => direction::N,
					_ => 0,
				};

				self.en_passant = Some(to.wrapping_add_signed(en_passant_off));
				should_reset_enpassant = false;
				self.halfmove_clock = 0;
			}

			MoveFlag::KCastle => {
				let king_from_mask = 1u64 << from;
				let king_to_mask = 1u64 << to;
				let king_from_to = king_from_mask | king_to_mask;

				let rook_from_mask = 1u64 << to.wrapping_add_signed(direction::E);
				let rook_to_mask = 1u64 << to.wrapping_add_signed(direction::W);
				let rook_from_to = rook_from_mask | rook_to_mask;

				let from_to = king_from_to | rook_from_to;

				self.pieces[C as usize][Piece::King as usize] ^= king_from_to;
				self.pieces[C as usize][Piece::Rook as usize] ^= rook_from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;

				self.queen_castle_flags[C as usize] = false;
				self.king_castle_flags[C as usize] = false;
			}

			MoveFlag::QCastle => {
				let king_from_mask = 1u64 << from;
				let king_to_mask = 1u64 << to;
				let king_from_to = king_from_mask | king_to_mask;

				let rook_from_mask = 1u64 << to.wrapping_add_signed(direction::WW);
				let rook_to_mask = 1u64 << to.wrapping_add_signed(direction::E);
				let rook_from_to = rook_from_mask | rook_to_mask;

				let from_to = king_from_to | rook_from_to;

				self.pieces[C as usize][Piece::King as usize] ^= king_from_to;
				self.pieces[C as usize][Piece::Rook as usize] ^= rook_from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;

				self.queen_castle_flags[C as usize] = false;
				self.king_castle_flags[C as usize] = false;
			}

			MoveFlag::Captures => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let piece = self.get_piece_at_square::<C>(from)?;
				let victim = match C ^ 1 {
					WHITE => self.get_piece_at_square::<WHITE>(to)?,
					BLACK => self.get_piece_at_square::<BLACK>(to)?,
					_ => Piece::Pawn,
				};

				if piece == Piece::King {
					self.king_castle_flags[C as usize] = false;
					self.queen_castle_flags[C as usize] = false;
				}

				if (piece == Piece::Rook)
					&& (from
						== match C {
							WHITE => Squares::H1 as u8,
							BLACK => Squares::H8 as u8,
							_ => 0,
						}) {
					self.king_castle_flags[C as usize] = false;
				} else if (piece == Piece::Rook)
					&& (from
						== match C {
							WHITE => Squares::A1 as u8,
							BLACK => Squares::A8 as u8,
							_ => 0,
						}) {
					self.queen_castle_flags[C as usize] = false;
				}

				if to == Squares::A1 as u8 {
					self.queen_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::A8 as u8 {
					self.queen_castle_flags[BLACK as usize] = false;
				}

				if to == Squares::H1 as u8 {
					self.king_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::H8 as u8 {
					self.king_castle_flags[BLACK as usize] = false;
				}

				self.pieces[C as usize][piece as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces[(C ^ 1) as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[(C ^ 1) as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;

				self.halfmove_clock = 0;
			}

			MoveFlag::EpCaptures => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim_square = to.wrapping_add_signed(match C {
					WHITE => direction::S,
					BLACK => direction::N,
					_ => 0,
				});
				let victim_mask = 1u64 << victim_square;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces[(C ^ 1) as usize][Piece::Pawn as usize] ^= victim_mask;
				self.pieces_by_color[(C ^ 1) as usize] ^= victim_mask;
				self.empty ^= from_to ^ victim_mask;
				self.occupied ^= from_to ^ victim_mask;

				self.halfmove_clock = 0;
			}

			MoveFlag::KnightPromotion => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Knight as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;

				self.halfmove_clock = 0;
			}

			MoveFlag::BishopPromotion => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Bishop as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;

				self.halfmove_clock = 0;
			}

			MoveFlag::RookPromotion => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Rook as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;

				self.halfmove_clock = 0;
			}

			MoveFlag::QueenPromotion => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Queen as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;

				self.halfmove_clock = 0;
			}

			MoveFlag::KnightPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = match C ^ 1 {
					WHITE => self.get_piece_at_square::<WHITE>(to)?,
					BLACK => self.get_piece_at_square::<BLACK>(to)?,
					_ => Piece::Pawn,
				};

				if to == Squares::A1 as u8 {
					self.queen_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::A8 as u8 {
					self.queen_castle_flags[BLACK as usize] = false;
				}

				if to == Squares::H1 as u8 {
					self.king_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::H8 as u8 {
					self.king_castle_flags[BLACK as usize] = false;
				}

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Knight as usize] ^= to_mask;
				self.pieces[(C ^ 1) as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[(C ^ 1) as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;

				self.halfmove_clock = 0;
			}

			MoveFlag::BishopPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = match C ^ 1 {
					WHITE => self.get_piece_at_square::<WHITE>(to)?,
					BLACK => self.get_piece_at_square::<BLACK>(to)?,
					_ => Piece::Pawn,
				};

				if to == Squares::A1 as u8 {
					self.queen_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::A8 as u8 {
					self.queen_castle_flags[BLACK as usize] = false;
				}

				if to == Squares::H1 as u8 {
					self.king_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::H8 as u8 {
					self.king_castle_flags[BLACK as usize] = false;
				}

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Bishop as usize] ^= to_mask;
				self.pieces[(C ^ 1) as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[(C ^ 1) as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;

				self.halfmove_clock = 0;
			}

			MoveFlag::RookPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = match C ^ 1 {
					WHITE => self.get_piece_at_square::<WHITE>(to)?,
					BLACK => self.get_piece_at_square::<BLACK>(to)?,
					_ => Piece::Pawn,
				};

				if to == Squares::A1 as u8 {
					self.queen_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::A8 as u8 {
					self.queen_castle_flags[BLACK as usize] = false;
				}

				if to == Squares::H1 as u8 {
					self.king_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::H8 as u8 {
					self.king_castle_flags[BLACK as usize] = false;
				}

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Rook as usize] ^= to_mask;
				self.pieces[(C ^ 1) as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[(C ^ 1) as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;

				self.halfmove_clock = 0;
			}

			MoveFlag::QueenPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = match C ^ 1 {
					WHITE => self.get_piece_at_square::<WHITE>(to)?,
					BLACK => self.get_piece_at_square::<BLACK>(to)?,
					_ => Piece::Pawn,
				};

				if to == Squares::A1 as u8 {
					self.queen_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::A8 as u8 {
					self.queen_castle_flags[BLACK as usize] = false;
				}

				if to == Squares::H1 as u8 {
					self.king_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::H8 as u8 {
					self.king_castle_flags[BLACK as usize] = false;
				}

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Queen as usize] ^= to_mask;
				self.pieces[(C ^ 1) as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[(C ^ 1) as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;

				self.halfmove_clock = 0;
			}
		};

		if should_reset_enpassant {
			self.en_passant = None;
		}

		self.turn = match self.turn {
			Color::White => Color::Black,
			Color::Black => Color::White,
		};

		Some(())
	}
}
