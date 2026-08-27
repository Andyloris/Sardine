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
	castle_rights: u8,
}

impl Board {
	pub fn do_null_move(&mut self) -> UndoInfo {
		let undo_info = UndoInfo {
			victim: None,
			en_passant: self.en_passant,
			halfmove_clock: self.halfmove_clock,
			zobrist: self.zobrist,
			castle_rights: self.castle_rights,
		};

		self.halfmove_clock += 1;

		if let Some(sq) = self.en_passant {
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
		self.hash_history.push(self.zobrist);
		undo_info
	}

	pub fn undo_null_move(&mut self, undo_info: UndoInfo) {
		self.hash_history.pop();

		self.turn = match self.turn {
			Color::White => Color::Black,
			Color::Black => Color::White,
		};

		self.en_passant = undo_info.en_passant;
		self.halfmove_clock = undo_info.halfmove_clock;
		self.zobrist = undo_info.zobrist;
	}

	pub fn do_move<const C: u8, const OPP: u8>(&mut self, m: &Move) -> Option<UndoInfo> {
		let mut undo_info = UndoInfo {
			victim: None,
			en_passant: self.en_passant,
			halfmove_clock: self.halfmove_clock,
			zobrist: self.zobrist,
			castle_rights: self.castle_rights,
		};

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
					if self.castle_rights & Self::KING_CASTLE[C as usize] != 0 {
						self.apply_zobrist_delta(ZobristDelta::CastleRights(
							Self::KING_CASTLE[C as usize],
						));
						self.castle_rights ^= Self::KING_CASTLE[C as usize];
					}

					if self.castle_rights & Self::QUEEN_CASTLE[C as usize] != 0 {
						self.apply_zobrist_delta(ZobristDelta::CastleRights(
							Self::QUEEN_CASTLE[C as usize],
						));
						self.castle_rights ^= Self::QUEEN_CASTLE[C as usize];
					}
				}

				if (piece == Piece::Rook)
					&& (from
						== match C {
							WHITE => Squares::H1 as u8,
							BLACK => Squares::H8 as u8,
							_ => 0,
						}) {
					if self.castle_rights & Self::KING_CASTLE[C as usize] != 0 {
						self.apply_zobrist_delta(ZobristDelta::CastleRights(
							Self::KING_CASTLE[C as usize],
						));
						self.castle_rights ^= Self::KING_CASTLE[C as usize];
					}
				} else if (piece == Piece::Rook)
					&& (from
						== match C {
							WHITE => Squares::A1 as u8,
							BLACK => Squares::A8 as u8,
							_ => 0,
						}) && self.castle_rights & Self::QUEEN_CASTLE[C as usize] != 0
				{
					self.apply_zobrist_delta(ZobristDelta::CastleRights(
						Self::QUEEN_CASTLE[C as usize],
					));
					self.castle_rights ^= Self::QUEEN_CASTLE[C as usize];
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

				self.evaluation_move_piece::<C>(from, to, piece);
				self.piece_list.piece_list_move_piece(from, to);

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

				self.evaluation_move_piece::<C>(from, to, Piece::Pawn);
				self.piece_list.piece_list_move_piece(from, to);

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

				self.evaluation_move_piece::<C>(from, to, Piece::King);
				self.evaluation_move_piece::<C>(rook_from, rook_to, Piece::Rook);
				self.piece_list.piece_list_move_piece(from, to);
				self.piece_list.piece_list_move_piece(rook_from, rook_to);

				if self.castle_rights & Self::KING_CASTLE[C as usize] != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(
						Self::KING_CASTLE[C as usize],
					));
					self.castle_rights ^= Self::KING_CASTLE[C as usize];
				}

				if self.castle_rights & Self::QUEEN_CASTLE[C as usize] != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(
						Self::QUEEN_CASTLE[C as usize],
					));
					self.castle_rights ^= Self::QUEEN_CASTLE[C as usize];
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

				self.evaluation_move_piece::<C>(from, to, Piece::King);
				self.evaluation_move_piece::<C>(rook_from, rook_to, Piece::Rook);
				self.piece_list.piece_list_move_piece(from, to);
				self.piece_list.piece_list_move_piece(rook_from, rook_to);

				if self.castle_rights & Self::KING_CASTLE[C as usize] != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(
						Self::KING_CASTLE[C as usize],
					));
					self.castle_rights ^= Self::KING_CASTLE[C as usize];
				}

				if self.castle_rights & Self::QUEEN_CASTLE[C as usize] != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(
						Self::QUEEN_CASTLE[C as usize],
					));
					self.castle_rights ^= Self::QUEEN_CASTLE[C as usize];
				}
			}

			MoveFlag::Captures => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let piece = self.get_piece_at_square::<C>(from)?;
				let victim = self.get_piece_at_square::<OPP>(to)?;

				undo_info.victim = Some(victim);

				if piece == Piece::King {
					if self.castle_rights & Self::KING_CASTLE[C as usize] != 0 {
						self.apply_zobrist_delta(ZobristDelta::CastleRights(
							Self::KING_CASTLE[C as usize],
						));
						self.castle_rights ^= Self::KING_CASTLE[C as usize];
					}

					if self.castle_rights & Self::QUEEN_CASTLE[C as usize] != 0 {
						self.apply_zobrist_delta(ZobristDelta::CastleRights(
							Self::QUEEN_CASTLE[C as usize],
						));
						self.castle_rights ^= Self::QUEEN_CASTLE[C as usize];
					}
				} else if piece == Piece::Rook
					&& (from
						== match C {
							WHITE => Squares::H1 as u8,
							BLACK => Squares::H8 as u8,
							_ => 0,
						}) {
					if self.castle_rights & Self::KING_CASTLE[C as usize] != 0 {
						self.apply_zobrist_delta(ZobristDelta::CastleRights(
							Self::KING_CASTLE[C as usize],
						));
						self.castle_rights ^= Self::KING_CASTLE[C as usize];
					}
				} else if (piece == Piece::Rook)
					&& (from
						== match C {
							WHITE => Squares::A1 as u8,
							BLACK => Squares::A8 as u8,
							_ => 0,
						}) && self.castle_rights & Self::QUEEN_CASTLE[C as usize] != 0
				{
					self.apply_zobrist_delta(ZobristDelta::CastleRights(
						Self::QUEEN_CASTLE[C as usize],
					));
					self.castle_rights ^= Self::QUEEN_CASTLE[C as usize];
				}

				if to == Squares::A1 as u8 && self.castle_rights & Self::WQUEEN_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::WQUEEN_CASTLE));
					self.castle_rights ^= Self::WQUEEN_CASTLE;
				}

				if to == Squares::A8 as u8 && self.castle_rights & Self::BQUEEN_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::BQUEEN_CASTLE));
					self.castle_rights ^= Self::BQUEEN_CASTLE;
				}

				if to == Squares::H1 as u8 && self.castle_rights & Self::WKING_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::WKING_CASTLE));
					self.castle_rights ^= Self::WKING_CASTLE;
				}

				if to == Squares::H8 as u8 && self.castle_rights & Self::BKING_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::BKING_CASTLE));
					self.castle_rights ^= Self::BKING_CASTLE;
				}

				self.pieces[C as usize][piece as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces[OPP as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[OPP as usize] ^= to_mask;
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
					PieceColorPair(victim, Color::from(OPP)),
					to,
				));

				self.evaluation_move_piece::<C>(from, to, piece);
				self.evaluation_remove_piece::<OPP>(to, victim);
				self.piece_list.piece_list_move_piece(from, to);
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
				self.pieces[OPP as usize][Piece::Pawn as usize] ^= victim_mask;
				self.pieces_by_color[OPP as usize] ^= victim_mask;
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
					PieceColorPair(Piece::Pawn, Color::from(OPP)),
					victim_square,
				));

				self.evaluation_move_piece::<C>(from, to, Piece::Pawn);
				self.evaluation_remove_piece::<OPP>(victim_square, Piece::Pawn);
				self.piece_list.piece_list_move_piece(from, to);
				self.piece_list.piece_list_remove_piece(victim_square);
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

				self.evaluation_remove_piece::<C>(from, Piece::Pawn);
				self.evaluation_add_piece::<C>(to, Piece::Knight);
				self.piece_list.piece_list_remove_piece(from);
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(Piece::Knight, Color::from(C)));
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

				self.evaluation_remove_piece::<C>(from, Piece::Pawn);
				self.evaluation_add_piece::<C>(to, Piece::Bishop);
				self.piece_list.piece_list_remove_piece(from);
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(Piece::Bishop, Color::from(C)));
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

				self.evaluation_remove_piece::<C>(from, Piece::Pawn);
				self.evaluation_add_piece::<C>(to, Piece::Rook);
				self.piece_list.piece_list_remove_piece(from);
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(Piece::Rook, Color::from(C)));
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

				self.evaluation_remove_piece::<C>(from, Piece::Pawn);
				self.evaluation_add_piece::<C>(to, Piece::Queen);
				self.piece_list.piece_list_remove_piece(from);
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(Piece::Queen, Color::from(C)));
			}

			MoveFlag::KnightPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = self.get_piece_at_square::<OPP>(to)?;

				undo_info.victim = Some(victim);

				if to == Squares::A1 as u8 && self.castle_rights & Self::WQUEEN_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::WQUEEN_CASTLE));
					self.castle_rights ^= Self::WQUEEN_CASTLE;
				}

				if to == Squares::A8 as u8 && self.castle_rights & Self::BQUEEN_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::BQUEEN_CASTLE));
					self.castle_rights ^= Self::BQUEEN_CASTLE;
				}

				if to == Squares::H1 as u8 && self.castle_rights & Self::WKING_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::WKING_CASTLE));
					self.castle_rights ^= Self::WKING_CASTLE;
				}

				if to == Squares::H8 as u8 && self.castle_rights & Self::BKING_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::BKING_CASTLE));
					self.castle_rights ^= Self::BKING_CASTLE;
				}

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Knight as usize] ^= to_mask;
				self.pieces[OPP as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[OPP as usize] ^= to_mask;
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
					PieceColorPair(victim, Color::from(OPP)),
					to,
				));

				self.evaluation_remove_piece::<C>(from, Piece::Pawn);
				self.evaluation_remove_piece::<OPP>(to, victim);
				self.evaluation_add_piece::<C>(to, Piece::Knight);
				self.piece_list.piece_list_remove_piece(from);
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(Piece::Knight, Color::from(C)));
			}

			MoveFlag::BishopPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = self.get_piece_at_square::<OPP>(to)?;

				undo_info.victim = Some(victim);

				if to == Squares::A1 as u8 && self.castle_rights & Self::WQUEEN_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::WQUEEN_CASTLE));
					self.castle_rights ^= Self::WQUEEN_CASTLE;
				}

				if to == Squares::A8 as u8 && self.castle_rights & Self::BQUEEN_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::BQUEEN_CASTLE));
					self.castle_rights ^= Self::BQUEEN_CASTLE;
				}

				if to == Squares::H1 as u8 && self.castle_rights & Self::WKING_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::WKING_CASTLE));
					self.castle_rights ^= Self::WKING_CASTLE;
				}

				if to == Squares::H8 as u8 && self.castle_rights & Self::BKING_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::BKING_CASTLE));
					self.castle_rights ^= Self::BKING_CASTLE;
				}

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Bishop as usize] ^= to_mask;
				self.pieces[OPP as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[OPP as usize] ^= to_mask;
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
					PieceColorPair(victim, Color::from(OPP)),
					to,
				));

				self.evaluation_remove_piece::<C>(from, Piece::Pawn);
				self.evaluation_remove_piece::<OPP>(to, victim);
				self.evaluation_add_piece::<C>(to, Piece::Bishop);
				self.piece_list.piece_list_remove_piece(from);
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(Piece::Bishop, Color::from(C)));
			}

			MoveFlag::RookPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = self.get_piece_at_square::<OPP>(to)?;

				undo_info.victim = Some(victim);

				if to == Squares::A1 as u8 && self.castle_rights & Self::WQUEEN_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::WQUEEN_CASTLE));
					self.castle_rights ^= Self::WQUEEN_CASTLE;
				}

				if to == Squares::A8 as u8 && self.castle_rights & Self::BQUEEN_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::BQUEEN_CASTLE));
					self.castle_rights ^= Self::BQUEEN_CASTLE;
				}

				if to == Squares::H1 as u8 && self.castle_rights & Self::WKING_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::WKING_CASTLE));
					self.castle_rights ^= Self::WKING_CASTLE;
				}

				if to == Squares::H8 as u8 && self.castle_rights & Self::BKING_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::BKING_CASTLE));
					self.castle_rights ^= Self::BKING_CASTLE;
				}

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Rook as usize] ^= to_mask;
				self.pieces[OPP as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[OPP as usize] ^= to_mask;
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
					PieceColorPair(victim, Color::from(OPP)),
					to,
				));

				self.evaluation_remove_piece::<C>(from, Piece::Pawn);
				self.evaluation_remove_piece::<OPP>(to, victim);
				self.evaluation_add_piece::<C>(to, Piece::Rook);
				self.piece_list.piece_list_remove_piece(from);
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(Piece::Rook, Color::from(C)));
			}

			MoveFlag::QueenPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = self.get_piece_at_square::<OPP>(to)?;

				undo_info.victim = Some(victim);

				if to == Squares::A1 as u8 && self.castle_rights & Self::WQUEEN_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::WQUEEN_CASTLE));
					self.castle_rights ^= Self::WQUEEN_CASTLE;
				}

				if to == Squares::A8 as u8 && self.castle_rights & Self::BQUEEN_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::BQUEEN_CASTLE));
					self.castle_rights ^= Self::BQUEEN_CASTLE;
				}

				if to == Squares::H1 as u8 && self.castle_rights & Self::WKING_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::WKING_CASTLE));
					self.castle_rights ^= Self::WKING_CASTLE;
				}

				if to == Squares::H8 as u8 && self.castle_rights & Self::BKING_CASTLE != 0 {
					self.apply_zobrist_delta(ZobristDelta::CastleRights(Self::BKING_CASTLE));
					self.castle_rights ^= Self::BKING_CASTLE;
				}

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Queen as usize] ^= to_mask;
				self.pieces[OPP as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[OPP as usize] ^= to_mask;
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
					PieceColorPair(victim, Color::from(OPP)),
					to,
				));

				self.evaluation_remove_piece::<C>(from, Piece::Pawn);
				self.evaluation_remove_piece::<OPP>(to, victim);
				self.evaluation_add_piece::<C>(to, Piece::Queen);
				self.piece_list.piece_list_remove_piece(from);
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(Piece::Queen, Color::from(C)));
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
		self.hash_history.push(self.zobrist);

		Some(undo_info)
	}

	pub fn undo_move<const C: u8, const OPP: u8>(
		&mut self,
		undo_info: UndoInfo,
		m: &Move,
	) -> Option<()> {
		self.hash_history.pop();

		self.turn = match self.turn {
			Color::White => Color::Black,
			Color::Black => Color::White,
		};

		self.en_passant = undo_info.en_passant;
		self.halfmove_clock = undo_info.halfmove_clock;
		self.zobrist = undo_info.zobrist;
		self.castle_rights = undo_info.castle_rights;

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

				self.evaluation_move_piece::<C>(to, from, piece);
				self.piece_list.piece_list_move_piece(to, from);
			}

			MoveFlag::DoublePush => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.empty ^= from_to;
				self.occupied ^= from_to;

				self.evaluation_move_piece::<C>(to, from, Piece::Pawn);
				self.piece_list.piece_list_move_piece(to, from);
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

				self.evaluation_move_piece::<C>(to, from, Piece::King);
				self.evaluation_move_piece::<C>(rook_to, rook_from, Piece::Rook);
				self.piece_list.piece_list_move_piece(to, from);
				self.piece_list.piece_list_move_piece(rook_to, rook_from);
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

				self.evaluation_move_piece::<C>(to, from, Piece::King);
				self.evaluation_move_piece::<C>(rook_to, rook_from, Piece::Rook);
				self.piece_list.piece_list_move_piece(to, from);
				self.piece_list.piece_list_move_piece(rook_to, rook_from);
			}

			MoveFlag::Captures => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let piece = self.get_piece_at_square::<C>(to)?;
				let victim = undo_info.victim?;

				self.pieces[C as usize][piece as usize] ^= from_to;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces[OPP as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[OPP as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;

				self.evaluation_move_piece::<C>(to, from, piece);
				self.evaluation_add_piece::<OPP>(to, victim);
				self.piece_list.piece_list_move_piece(to, from);
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(victim, Color::from(OPP)));
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
				self.pieces[OPP as usize][Piece::Pawn as usize] ^= victim_mask;
				self.pieces_by_color[OPP as usize] ^= victim_mask;
				self.empty ^= from_to ^ victim_mask;
				self.occupied ^= from_to ^ victim_mask;

				self.evaluation_move_piece::<C>(to, from, Piece::Pawn);
				self.evaluation_add_piece::<OPP>(victim_square, Piece::Pawn);
				self.piece_list.piece_list_move_piece(to, from);
				self.piece_list.piece_list_add_piece(
					victim_square,
					PieceColorPair(Piece::Pawn, Color::from(OPP)),
				);
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

				self.evaluation_add_piece::<C>(from, Piece::Pawn);
				self.evaluation_remove_piece::<C>(to, Piece::Knight);
				self.piece_list
					.piece_list_add_piece(from, PieceColorPair(Piece::Pawn, Color::from(C)));
				self.piece_list.piece_list_remove_piece(to);
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

				self.evaluation_add_piece::<C>(from, Piece::Pawn);
				self.evaluation_remove_piece::<C>(to, Piece::Bishop);
				self.piece_list
					.piece_list_add_piece(from, PieceColorPair(Piece::Pawn, Color::from(C)));
				self.piece_list.piece_list_remove_piece(to);
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

				self.evaluation_add_piece::<C>(from, Piece::Pawn);
				self.evaluation_remove_piece::<C>(to, Piece::Rook);
				self.piece_list
					.piece_list_add_piece(from, PieceColorPair(Piece::Pawn, Color::from(C)));
				self.piece_list.piece_list_remove_piece(to);
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

				self.evaluation_add_piece::<C>(from, Piece::Pawn);
				self.evaluation_remove_piece::<C>(to, Piece::Queen);
				self.piece_list
					.piece_list_add_piece(from, PieceColorPair(Piece::Pawn, Color::from(C)));
				self.piece_list.piece_list_remove_piece(to);
			}

			MoveFlag::KnightPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = undo_info.victim?;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Knight as usize] ^= to_mask;
				self.pieces[OPP as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[OPP as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;

				self.evaluation_add_piece::<C>(from, Piece::Pawn);
				self.evaluation_remove_piece::<C>(to, Piece::Knight);
				self.evaluation_add_piece::<OPP>(to, victim);
				self.piece_list
					.piece_list_add_piece(from, PieceColorPair(Piece::Pawn, Color::from(C)));
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(victim, Color::from(OPP)));
			}

			MoveFlag::BishopPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = undo_info.victim?;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Bishop as usize] ^= to_mask;
				self.pieces[OPP as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[OPP as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;

				self.evaluation_add_piece::<C>(from, Piece::Pawn);
				self.evaluation_remove_piece::<C>(to, Piece::Bishop);
				self.evaluation_add_piece::<OPP>(to, victim);
				self.piece_list
					.piece_list_add_piece(from, PieceColorPair(Piece::Pawn, Color::from(C)));
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(victim, Color::from(OPP)));
			}

			MoveFlag::RookPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = undo_info.victim?;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Rook as usize] ^= to_mask;
				self.pieces[OPP as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[OPP as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;

				self.evaluation_add_piece::<C>(from, Piece::Pawn);
				self.evaluation_remove_piece::<C>(to, Piece::Rook);
				self.evaluation_add_piece::<OPP>(to, victim);
				self.piece_list
					.piece_list_add_piece(from, PieceColorPair(Piece::Pawn, Color::from(C)));
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(victim, Color::from(OPP)));
			}

			MoveFlag::QueenPromoCapture => {
				let from_mask = 1u64 << from;
				let to_mask = 1u64 << to;
				let from_to = from_mask | to_mask;
				let victim = undo_info.victim?;

				self.pieces[C as usize][Piece::Pawn as usize] ^= from_mask;
				self.pieces[C as usize][Piece::Queen as usize] ^= to_mask;
				self.pieces[OPP as usize][victim as usize] ^= to_mask;
				self.pieces_by_color[C as usize] ^= from_to;
				self.pieces_by_color[OPP as usize] ^= to_mask;
				self.empty ^= from_mask;
				self.occupied ^= from_mask;

				self.evaluation_add_piece::<C>(from, Piece::Pawn);
				self.evaluation_remove_piece::<C>(to, Piece::Queen);
				self.evaluation_add_piece::<OPP>(to, victim);
				self.piece_list
					.piece_list_add_piece(from, PieceColorPair(Piece::Pawn, Color::from(C)));
				self.piece_list
					.piece_list_add_piece(to, PieceColorPair(victim, Color::from(OPP)));
			}
		}

		Some(())
	}
}
