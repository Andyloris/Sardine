use std::num::NonZero;

use crate::board::{
	Board,
	movegen::{Move, MoveFlag},
	utils::{BLACK, Color, Piece, PieceColorPair, Squares, WHITE, direction},
	zobrist::ZobristDelta,
};

pub struct UndoInfo {
	victim: Option<Piece>,
	en_passant: Option<NonZero<u8>>,
	halfmove_clock: usize,
	zobrist: u64,
	king_castle_flags: [bool; 2],
	queen_castle_flags: [bool; 2],
}

impl Board {
	pub fn do_move<const C: u8>(&mut self, m: &Move) -> Option<UndoInfo> {
		let (from, to, flags) = m.unpack();
		let mut should_reset_enpassant = true;
		self.halfmove_clock += 1;

		let mut undo_info = UndoInfo {
			victim: None,
			en_passant: self.en_passant,
			halfmove_clock: self.halfmove_clock,
			zobrist: self.zobrist,
			king_castle_flags: self.king_castle_flags,
			queen_castle_flags: self.queen_castle_flags,
		};

		match flags {
			MoveFlag::Quiet => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let piece = self.get_piece_at_square::<C>(from)?;

				if piece == Piece::King {
					if self.king_castle_flags[C as usize] {
						self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::from(C)));
						self.king_castle_flags[C as usize] = false;
					}

					if self.queen_castle_flags[C as usize] {
						self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::from(C)));
						self.queen_castle_flags[C as usize] = false;
					}
				}

				if (piece == Piece::Rook)
					&& (from
						== match C {
							WHITE => Squares::H1 as u8,
							BLACK => Squares::H8 as u8,
							_ => 0,
						}) {
					if self.king_castle_flags[C as usize] {
						self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::from(C)));
						self.king_castle_flags[C as usize] = false;
					}
				} else if (piece == Piece::Rook)
					&& (from
						== match C {
							WHITE => Squares::A1 as u8,
							BLACK => Squares::A8 as u8,
							_ => 0,
						}) && self.queen_castle_flags[C as usize]
				{
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::from(C)));
					self.queen_castle_flags[C as usize] = false;
				}

				self.pieces[C as usize][piece as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(piece, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(piece, Color::from(C)),
					to,
				));

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

				self.apply_zobrist_delta(ZobristDelta::EnPassantSquareChange(
					self.en_passant.map(|v| v.get()).unwrap_or(0),
				));

				let en_passant_sq = to.wrapping_add_signed(en_passant_off);
				self.apply_zobrist_delta(ZobristDelta::EnPassantSquareChange(en_passant_sq));

				self.en_passant = NonZero::new(en_passant_sq);

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C)),
					to,
				));
				should_reset_enpassant = false;
				self.halfmove_clock = 0;
			}

			MoveFlag::KCastle => {
				let king_from_mask = 1u64 << from;
				let king_to_mask = 1u64 << to;
				let king_from_to = king_from_mask | king_to_mask;

				let rook_from = to.wrapping_add_signed(direction::E);
				let rook_to = to.wrapping_add_signed(direction::W);

				let rook_from_mask = 1u64 << rook_from;
				let rook_to_mask = 1u64 << rook_to;
				let rook_from_to = rook_from_mask | rook_to_mask;

				let from_to = king_from_to | rook_from_to;

				self.pieces[C as usize][Piece::King as usize] ^= king_from_to;
				self.pieces[C as usize][Piece::Rook as usize] ^= rook_from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::King, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::King, Color::from(C)),
					to,
				));

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Rook, Color::from(C)),
					rook_from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Rook, Color::from(C)),
					rook_to,
				));

				if self.king_castle_flags[C as usize] {
					self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::from(C)));
					self.king_castle_flags[C as usize] = false;
				}

				if self.queen_castle_flags[C as usize] {
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::from(C)));
					self.queen_castle_flags[C as usize] = false;
				}
			}

			MoveFlag::QCastle => {
				let king_from_mask = 1u64 << from;
				let king_to_mask = 1u64 << to;
				let king_from_to = king_from_mask | king_to_mask;

				let rook_from = to.wrapping_add_signed(direction::WW);
				let rook_to = to.wrapping_add_signed(direction::E);

				let rook_from_mask = 1u64 << rook_from;
				let rook_to_mask = 1u64 << rook_to;
				let rook_from_to = rook_from_mask | rook_to_mask;

				let from_to = king_from_to | rook_from_to;

				self.pieces[C as usize][Piece::King as usize] ^= king_from_to;
				self.pieces[C as usize][Piece::Rook as usize] ^= rook_from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::King, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::King, Color::from(C)),
					to,
				));

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Rook, Color::from(C)),
					rook_from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Rook, Color::from(C)),
					rook_to,
				));

				if self.king_castle_flags[C as usize] {
					self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::from(C)));
					self.king_castle_flags[C as usize] = false;
				}

				if self.queen_castle_flags[C as usize] {
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::from(C)));
					self.queen_castle_flags[C as usize] = false;
				}
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

				undo_info.victim = Some(victim);

				if piece == Piece::King {
					if self.king_castle_flags[C as usize] {
						self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::from(C)));
						self.king_castle_flags[C as usize] = false;
					}

					if self.queen_castle_flags[C as usize] {
						self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::from(C)));
						self.queen_castle_flags[C as usize] = false;
					}
				} else if piece == Piece::Rook
					&& (from
						== match C {
							WHITE => Squares::H1 as u8,
							BLACK => Squares::H8 as u8,
							_ => 0,
						}) {
					if self.king_castle_flags[C as usize] {
						self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::from(C)));
						self.king_castle_flags[C as usize] = false;
					}
				} else if (piece == Piece::Rook)
					&& (from
						== match C {
							WHITE => Squares::A1 as u8,
							BLACK => Squares::A8 as u8,
							_ => 0,
						}) && self.queen_castle_flags[C as usize]
				{
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::from(C)));
					self.queen_castle_flags[C as usize] = false;
				}

				if to == Squares::A1 as u8 && self.queen_castle_flags[WHITE as usize] {
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::White));
					self.queen_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::A8 as u8 && self.queen_castle_flags[BLACK as usize] {
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::Black));
					self.queen_castle_flags[BLACK as usize] = false;
				}

				if to == Squares::H1 as u8 && self.king_castle_flags[WHITE as usize] {
					self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::White));
					self.king_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::H8 as u8 && self.king_castle_flags[BLACK as usize] {
					self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::Black));
					self.king_castle_flags[BLACK as usize] = false;
				}

				self.pieces[C as usize][piece as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces[(C ^ 1) as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[(C ^ 1) as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;

				self.halfmove_clock = 0;

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(piece, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(piece, Color::from(C)),
					to,
				));

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(victim, Color::from(C ^ 1)),
					to,
				));
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

				undo_info.victim = Some(Piece::Pawn);

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces[(C ^ 1) as usize][Piece::Pawn as usize] ^= victim_mask;
				self.pieces_by_color[(C ^ 1) as usize] ^= victim_mask;
				self.empty ^= from_to ^ victim_mask;
				self.occupied ^= from_to ^ victim_mask;

				self.halfmove_clock = 0;

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C)),
					to,
				));

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C ^ 1)),
					to,
				));
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

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Knight, Color::from(C)),
					to,
				));
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

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Bishop, Color::from(C)),
					to,
				));
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

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Rook, Color::from(C)),
					to,
				));
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

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Queen, Color::from(C)),
					to,
				));
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

				undo_info.victim = Some(victim);

				if to == Squares::A1 as u8 && self.queen_castle_flags[WHITE as usize] {
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::White));
					self.queen_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::A8 as u8 && self.queen_castle_flags[BLACK as usize] {
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::Black));
					self.queen_castle_flags[BLACK as usize] = false;
				}

				if to == Squares::H1 as u8 && self.king_castle_flags[WHITE as usize] {
					self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::White));
					self.king_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::H8 as u8 && self.king_castle_flags[BLACK as usize] {
					self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::Black));
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

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Knight, Color::from(C)),
					to,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(victim, Color::from(C ^ 1)),
					to,
				));
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

				undo_info.victim = Some(victim);

				if to == Squares::A1 as u8 && self.queen_castle_flags[WHITE as usize] {
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::White));
					self.queen_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::A8 as u8 && self.queen_castle_flags[BLACK as usize] {
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::Black));
					self.queen_castle_flags[BLACK as usize] = false;
				}

				if to == Squares::H1 as u8 && self.king_castle_flags[WHITE as usize] {
					self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::White));
					self.king_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::H8 as u8 && self.king_castle_flags[BLACK as usize] {
					self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::Black));
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

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Bishop, Color::from(C)),
					to,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(victim, Color::from(C ^ 1)),
					to,
				));
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

				undo_info.victim = Some(victim);

				if to == Squares::A1 as u8 && self.queen_castle_flags[WHITE as usize] {
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::White));
					self.queen_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::A8 as u8 && self.queen_castle_flags[BLACK as usize] {
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::Black));
					self.queen_castle_flags[BLACK as usize] = false;
				}

				if to == Squares::H1 as u8 && self.king_castle_flags[WHITE as usize] {
					self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::White));
					self.king_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::H8 as u8 && self.king_castle_flags[BLACK as usize] {
					self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::Black));
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

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Rook, Color::from(C)),
					to,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(victim, Color::from(C ^ 1)),
					to,
				));
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

				undo_info.victim = Some(victim);

				if to == Squares::A1 as u8 && self.queen_castle_flags[WHITE as usize] {
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::White));
					self.queen_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::A8 as u8 && self.queen_castle_flags[BLACK as usize] {
					self.apply_zobrist_delta(ZobristDelta::QCastleRights(Color::Black));
					self.queen_castle_flags[BLACK as usize] = false;
				}

				if to == Squares::H1 as u8 && self.king_castle_flags[WHITE as usize] {
					self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::White));
					self.king_castle_flags[WHITE as usize] = false;
				}

				if to == Squares::H8 as u8 && self.king_castle_flags[BLACK as usize] {
					self.apply_zobrist_delta(ZobristDelta::KCastleRights(Color::Black));
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

				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Pawn, Color::from(C)),
					from,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(Piece::Queen, Color::from(C)),
					to,
				));
				self.apply_zobrist_delta(ZobristDelta::PutRemove(
					PieceColorPair(victim, Color::from(C ^ 1)),
					to,
				));
			}
		};

		if should_reset_enpassant && let Some(sq) = self.en_passant {
			let sq = sq.get();
			self.apply_zobrist_delta(ZobristDelta::EnPassantSquareChange(sq));
			self.apply_zobrist_delta(ZobristDelta::EnPassantSquareChange(0));
			self.en_passant = None;
		}

		self.turn = match self.turn {
			Color::White => Color::Black,
			Color::Black => Color::White,
		};

		self.apply_zobrist_delta(ZobristDelta::WhiteTurn);

		Some(undo_info)
	}

	pub fn undo_move<const C: u8>(&mut self, undo_info: UndoInfo, m: &Move) -> Option<()> {
		// ToDo, threefold repetition detection too
		self.apply_zobrist_delta(ZobristDelta::WhiteTurn);

		self.turn = match self.turn {
			Color::White => Color::Black,
			Color::Black => Color::White,
		};

		self.en_passant = undo_info.en_passant;
		self.halfmove_clock = undo_info.halfmove_clock;
		self.zobrist = undo_info.zobrist;
		self.king_castle_flags = undo_info.king_castle_flags;
		self.queen_castle_flags = undo_info.queen_castle_flags;

		let (from, to, flags) = m.unpack();
		match flags {
			MoveFlag::Quiet => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let piece = self.get_piece_at_square::<C>(to)?;

				self.pieces[C as usize][piece as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;
			}

			MoveFlag::DoublePush => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;
			}

			MoveFlag::KCastle => {
				let king_from_mask = 1u64 << from;
				let king_to_mask = 1u64 << to;
				let king_from_to = king_from_mask | king_to_mask;

				let rook_from = to.wrapping_add_signed(direction::E);
				let rook_to = to.wrapping_add_signed(direction::W);

				let rook_from_mask = 1u64 << rook_from;
				let rook_to_mask = 1u64 << rook_to;
				let rook_from_to = rook_from_mask | rook_to_mask;

				let from_to = king_from_to | rook_from_to;

				self.pieces[C as usize][Piece::King as usize] ^= king_from_to;
				self.pieces[C as usize][Piece::Rook as usize] ^= rook_from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;
			}

			MoveFlag::QCastle => {
				let king_from_mask = 1u64 << from;
				let king_to_mask = 1u64 << to;
				let king_from_to = king_from_mask | king_to_mask;

				let rook_from = to.wrapping_add_signed(direction::WW);
				let rook_to = to.wrapping_add_signed(direction::E);

				let rook_from_mask = 1u64 << rook_from;
				let rook_to_mask = 1u64 << rook_to;
				let rook_from_to = rook_from_mask | rook_to_mask;

				let from_to = king_from_to | rook_from_to;

				self.pieces[C as usize][Piece::King as usize] ^= king_from_to;
				self.pieces[C as usize][Piece::Rook as usize] ^= rook_from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;
			}

			MoveFlag::Captures => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let piece = self.get_piece_at_square::<C>(to)?;
				let victim = undo_info.victim?;

				self.pieces[C as usize][piece as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces[(C ^ 1) as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[(C ^ 1) as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;
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
			}

			MoveFlag::KnightPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = undo_info.victim?;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Knight as usize] ^= to_mask;
				self.pieces[(C ^ 1) as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[(C ^ 1) as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;
			}

			MoveFlag::BishopPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = undo_info.victim?;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Bishop as usize] ^= to_mask;
				self.pieces[(C ^ 1) as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[(C ^ 1) as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;
			}

			MoveFlag::RookPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = undo_info.victim?;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Rook as usize] ^= to_mask;
				self.pieces[(C ^ 1) as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[(C ^ 1) as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;
			}

			MoveFlag::QueenPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = undo_info.victim?;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Queen as usize] ^= to_mask;
				self.pieces[(C ^ 1) as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[(C ^ 1) as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;
			}
		}

		Some(())
	}
}
