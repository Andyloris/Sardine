use std::{fmt::Display, ops::Index, slice::Iter};

use crate::board::{
	Board,
	attacks::{
		get_king_attacks, get_knight_attacks, get_pawn_attacks, get_pawns_able_to_attack_east_bb,
		get_pawns_able_to_attack_west_bb, get_sliding_attacks,
	},
	in_between::in_between_mask,
	quiets::{get_pawns_able_to_double_push, get_pawns_able_to_push},
	utils::{
		BLACK, Color, KING_CASTLE_MASKS, Piece, QUEEN_CASTLE_MASKS, RANK_1, RANK_2, RANK_7, RANK_8,
		Square, Squares, WHITE, clear_lsb,
		direction::{self, N, S},
		shift_bb,
	},
};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MoveFlag {
	#[default]
	Quiet = 0,
	DoublePush = 1,
	KCastle = 2,
	QCastle = 3,
	Captures = 4,
	EpCaptures = 5,
	KnightPromotion = 8,
	BishopPromotion = 9,
	RookPromotion = 10,
	QueenPromotion = 11,
	KnightPromoCapture = 12,
	BishopPromoCapture = 13,
	RookPromoCapture = 14,
	QueenPromoCapture = 15,
}

impl From<u8> for MoveFlag {
	fn from(value: u8) -> Self {
		match value {
			0 => Self::Quiet,
			1 => Self::DoublePush,
			2 => Self::KCastle,
			3 => Self::QCastle,
			4 => Self::Captures,
			5 => Self::EpCaptures,
			8 => Self::KnightPromotion,
			9 => Self::BishopPromotion,
			10 => Self::RookPromotion,
			11 => Self::QueenPromotion,
			12 => Self::KnightPromoCapture,
			13 => Self::BishopPromoCapture,
			14 => Self::RookPromoCapture,
			15 => Self::QueenPromoCapture,
			_ => panic!("Unknown moveflag number: {}", value),
		}
	}
}

#[repr(transparent)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Move(u16);

impl Move {
	pub fn new(from: u8, to: u8, flags: MoveFlag) -> Self {
		let packed: u16 = from as u16 | ((to as u16) << 6) | ((flags as u16) << 12);
		Self(packed)
	}

	#[inline(always)]
	pub fn unpack(self) -> (u8, u8, MoveFlag) {
		(
			self.0 as u8 & 0x3F,
			(self.0 >> 6) as u8 & 0x3F,
			MoveFlag::from((self.0 >> 12) as u8 & 0xF),
		)
	}

	#[inline(always)]
	pub fn get_from(self) -> u8 {
		self.0 as u8 & 0x3F
	}

	#[inline(always)]
	pub fn get_to(self) -> u8 {
		(self.0 >> 6) as u8 & 0x3F
	}

	#[inline(always)]
	pub fn get_flags(self) -> MoveFlag {
		MoveFlag::from((self.0 >> 12) as u8 & 0xF)
	}

	#[inline(always)]
	pub fn is_quiet(self) -> bool {
		matches!(
			self.get_flags(),
			MoveFlag::Quiet
				| MoveFlag::DoublePush
				| MoveFlag::KCastle
				| MoveFlag::QCastle
				| MoveFlag::KnightPromotion
				| MoveFlag::BishopPromotion
				| MoveFlag::RookPromotion
				| MoveFlag::QueenPromotion
		)
	}

	#[inline(always)]
	pub fn is_promotion(self) -> bool {
		matches!(
			self.get_flags(),
			MoveFlag::KnightPromotion
				| MoveFlag::BishopPromotion
				| MoveFlag::RookPromotion
				| MoveFlag::QueenPromotion
				| MoveFlag::KnightPromoCapture
				| MoveFlag::BishopPromoCapture
				| MoveFlag::RookPromoCapture
				| MoveFlag::QueenPromoCapture
		)
	}
}

impl Display for Move {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let (from, to, flags) = self.unpack();
		let prom = match flags {
			MoveFlag::KnightPromotion | MoveFlag::KnightPromoCapture => "n",
			MoveFlag::BishopPromotion | MoveFlag::BishopPromoCapture => "b",
			MoveFlag::RookPromotion | MoveFlag::RookPromoCapture => "r",
			MoveFlag::QueenPromotion | MoveFlag::QueenPromoCapture => "q",
			_ => "",
		};
		write!(f, "{}{}{}", Square(from), Square(to), prom)
	}
}

pub const MAX_MOVE_LIST_SIZE: usize = 255;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveList([Move; MAX_MOVE_LIST_SIZE], usize);

impl Default for MoveList {
	fn default() -> Self {
		Self([Move::default(); MAX_MOVE_LIST_SIZE], 0)
	}
}

impl Index<usize> for MoveList {
	type Output = Move;
	fn index(&self, index: usize) -> &Self::Output {
		// ToDo: Unchecked version
		if index >= self.1 {
			panic!(
				"index out of bounds: the len is {} but the index is {}",
				self.1, index
			);
		}

		&self.0[index]
	}
}

impl MoveList {
	pub const fn push(&mut self, m: Move) {
		self.0[self.1] = m;
		self.1 += 1;
	}

	pub const fn clear(&mut self) {
		self.1 = 0;
	}

	pub fn iter(&self) -> Iter<'_, Move> {
		self.0[0..self.1].iter()
	}

	pub fn as_slice(&self) -> &[Move] {
		&self.0[0..self.1]
	}

	pub fn as_mut_slice(&mut self) -> &mut [Move] {
		&mut self.0[0..self.1]
	}

	pub const fn len(&self) -> usize {
		self.1
	}
}

pub mod move_gen_stages {
	pub const CAPTURES: u8 = 1;
	pub const QUIETS: u8 = 2;
	pub const ALL: u8 = 3;
}

mod serialization_flags {
	pub const CUSTOM: u8 = 0;
	pub const PROMOTIONS: u8 = 1;
	pub const PROMO_CAPTURES: u8 = 2;
}

impl Board {
	fn serialize_with_offset<const C: u8>(
		mut bb: u64,
		offset: i8,
		flags: MoveFlag,
		buf: &mut MoveList,
	) {
		while bb != 0 {
			let from = bb.trailing_zeros() as u8;
			clear_lsb(&mut bb);
			match C {
				serialization_flags::CUSTOM => {
					let m = Move::new(from, from.wrapping_add_signed(offset), flags);
					buf.push(m);
				}

				serialization_flags::PROMOTIONS => {
					let m = Move::new(
						from,
						from.wrapping_add_signed(offset),
						MoveFlag::QueenPromotion,
					);
					buf.push(m);
					let m = Move::new(
						from,
						from.wrapping_add_signed(offset),
						MoveFlag::RookPromotion,
					);
					buf.push(m);
					let m = Move::new(
						from,
						from.wrapping_add_signed(offset),
						MoveFlag::BishopPromotion,
					);
					buf.push(m);
					let m = Move::new(
						from,
						from.wrapping_add_signed(offset),
						MoveFlag::KnightPromotion,
					);
					buf.push(m);
				}

				serialization_flags::PROMO_CAPTURES => {
					let m = Move::new(
						from,
						from.wrapping_add_signed(offset),
						MoveFlag::QueenPromoCapture,
					);
					buf.push(m);
					let m = Move::new(
						from,
						from.wrapping_add_signed(offset),
						MoveFlag::RookPromoCapture,
					);
					buf.push(m);
					let m = Move::new(
						from,
						from.wrapping_add_signed(offset),
						MoveFlag::BishopPromoCapture,
					);
					buf.push(m);
					let m = Move::new(
						from,
						from.wrapping_add_signed(offset),
						MoveFlag::KnightPromoCapture,
					);
					buf.push(m);
				}

				_ => {}
			}
		}
	}

	fn serialize_with_to_bb(from: u8, mut targets: u64, flags: MoveFlag, buf: &mut MoveList) {
		while targets != 0 {
			let to = targets.trailing_zeros() as u8;
			clear_lsb(&mut targets);
			let m = Move::new(from, to, flags);
			buf.push(m);
		}
	}

	pub fn is_square_attacked<const BY: u8>(&self, sq: u8) -> bool {
		let pawns = self.pieces[BY as usize][Piece::Pawn as usize];
		if match BY ^ 1 {
			WHITE => get_pawn_attacks::<WHITE>(sq) & pawns != 0,
			BLACK => get_pawn_attacks::<BLACK>(sq) & pawns != 0,
			_ => false,
		} {
			return true;
		}

		let knights = self.pieces[BY as usize][Piece::Knight as usize];
		if get_knight_attacks(sq) & knights != 0 {
			return true;
		}

		let king = self.pieces[BY as usize][Piece::King as usize];
		if get_king_attacks(sq) & king != 0 {
			return true;
		}

		let bishops_queens = self.pieces[BY as usize][Piece::Bishop as usize]
			| self.pieces[BY as usize][Piece::Queen as usize];
		if get_sliding_attacks::<true>(sq, self.occupied) & bishops_queens != 0 {
			return true;
		}

		let rooks_queens = self.pieces[BY as usize][Piece::Rook as usize]
			| self.pieces[BY as usize][Piece::Queen as usize];
		if get_sliding_attacks::<false>(sq, self.occupied) & rooks_queens != 0 {
			return true;
		}

		false
	}

	pub fn is_in_check<const KING_C: u8>(&self) -> bool {
		let king_sq = self.pieces[KING_C as usize][Piece::King as usize].trailing_zeros() as u8;
		match KING_C ^ 1 {
			WHITE => self.is_square_attacked::<WHITE>(king_sq),
			BLACK => self.is_square_attacked::<BLACK>(king_sq),
			_ => false,
		}
	}

	pub fn gen_pseudo_legal_captures<const C: u8>(&self, buf: &mut MoveList) {
		let targets = self.get_opponent_bb::<C>();

		// PAWNS
		let friendly_pawns = self.pieces[C as usize][Piece::Pawn as usize];
		let promo_pawns = friendly_pawns
			& match C {
				WHITE => RANK_7,
				BLACK => RANK_2,
				_ => 0,
			};
		let no_promo_pawns = friendly_pawns ^ promo_pawns;

		let no_promo_pawns_capt_west =
			get_pawns_able_to_attack_west_bb::<C>(no_promo_pawns, targets);
		let no_promo_pawns_capt_east =
			get_pawns_able_to_attack_east_bb::<C>(no_promo_pawns, targets);
		let promo_pawns_capt_west = get_pawns_able_to_attack_west_bb::<C>(promo_pawns, targets);
		let promo_pawns_capt_east = get_pawns_able_to_attack_east_bb::<C>(promo_pawns, targets);

		let pawn_capt_west_offset = match C {
			WHITE => direction::NW,
			BLACK => direction::SW,
			_ => 0,
		};

		let pawn_capt_east_offset = match C {
			WHITE => direction::NE,
			BLACK => direction::SE,
			_ => 0,
		};

		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_pawns_capt_west,
			pawn_capt_west_offset,
			MoveFlag::Captures,
			buf,
		);

		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_pawns_capt_east,
			pawn_capt_east_offset,
			MoveFlag::Captures,
			buf,
		);

		Self::serialize_with_offset::<{ serialization_flags::PROMO_CAPTURES }>(
			promo_pawns_capt_west,
			pawn_capt_west_offset,
			MoveFlag::Captures,
			buf,
		);

		Self::serialize_with_offset::<{ serialization_flags::PROMO_CAPTURES }>(
			promo_pawns_capt_east,
			pawn_capt_east_offset,
			MoveFlag::Captures,
			buf,
		);

		if let Some(en_passant) = self.en_passant {
			let mut en_passant_pawns = match C ^ 1 {
				WHITE => get_pawn_attacks::<{ WHITE }>(en_passant.get()),
				BLACK => get_pawn_attacks::<{ BLACK }>(en_passant.get()),
				_ => 0,
			} & friendly_pawns;

			while en_passant_pawns != 0 {
				let from = en_passant_pawns.trailing_zeros() as u8;
				clear_lsb(&mut en_passant_pawns);
				let m = Move::new(from, en_passant.get(), MoveFlag::EpCaptures);
				buf.push(m);
			}
		}

		// KNIGHTS
		let mut knights = self.pieces[C as usize][Piece::Knight as usize];
		while knights != 0 {
			let from = knights.trailing_zeros() as u8;
			clear_lsb(&mut knights);
			let knight_targets = get_knight_attacks(from) & targets;
			Self::serialize_with_to_bb(from, knight_targets, MoveFlag::Captures, buf);
		}

		// BISHOPS/QUEENS
		let mut bishops_queens = self.pieces[C as usize][Piece::Bishop as usize]
			| self.pieces[C as usize][Piece::Queen as usize];
		while bishops_queens != 0 {
			let from = bishops_queens.trailing_zeros() as u8;
			clear_lsb(&mut bishops_queens);
			let bishop_targets = get_sliding_attacks::<true>(from, self.occupied) & targets;
			Self::serialize_with_to_bb(from, bishop_targets, MoveFlag::Captures, buf);
		}

		// ROOKS/QUEENS
		let mut rooks_queens = self.pieces[C as usize][Piece::Rook as usize]
			| self.pieces[C as usize][Piece::Queen as usize];
		while rooks_queens != 0 {
			let from = rooks_queens.trailing_zeros() as u8;
			clear_lsb(&mut rooks_queens);
			let rook_targets = get_sliding_attacks::<false>(from, self.occupied) & targets;
			Self::serialize_with_to_bb(from, rook_targets, MoveFlag::Captures, buf);
		}

		// KING
		let king = self.pieces[C as usize][Piece::King as usize];
		{
			let from = king.trailing_zeros() as u8;
			let king_targets = get_king_attacks(from) & targets;
			Self::serialize_with_to_bb(from, king_targets, MoveFlag::Captures, buf);
		}
	}

	pub fn gen_pseudo_legal_quiets<const C: u8>(&self, buf: &mut MoveList) {
		// PAWNS
		let friendly_pawns = self.pieces[C as usize][Piece::Pawn as usize];
		let promo_pawns = friendly_pawns
			& match C {
				WHITE => RANK_7,
				BLACK => RANK_2,
				_ => 0,
			};
		let no_promo_pawns = friendly_pawns ^ promo_pawns;
		let promo_single_push = get_pawns_able_to_push::<C>(promo_pawns, self.empty);
		let no_promo_single_push = get_pawns_able_to_push::<C>(no_promo_pawns, self.empty);
		let no_promo_double_push = get_pawns_able_to_double_push::<C>(no_promo_pawns, self.empty);

		// Partition into pawns that can only single push
		let no_promo_single_push = no_promo_double_push ^ no_promo_single_push;
		let single_push_off = match C {
			WHITE => N,
			BLACK => S,
			_ => 0,
		};

		let double_push_off = match C {
			WHITE => 2 * N,
			BLACK => 2 * S,
			_ => 0,
		};

		Self::serialize_with_offset::<{ serialization_flags::PROMOTIONS }>(
			promo_single_push,
			single_push_off,
			MoveFlag::Quiet,
			buf,
		);
		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_single_push,
			single_push_off,
			MoveFlag::Quiet,
			buf,
		);
		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_double_push,
			single_push_off,
			MoveFlag::Quiet,
			buf,
		);
		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_double_push,
			double_push_off,
			MoveFlag::DoublePush,
			buf,
		);

		// KNIGHTS
		let mut knights = self.pieces[C as usize][Piece::Knight as usize];
		while knights != 0 {
			let from = knights.trailing_zeros() as u8;
			clear_lsb(&mut knights);
			let knight_targets = get_knight_attacks(from) & self.empty;
			Self::serialize_with_to_bb(from, knight_targets, MoveFlag::Quiet, buf);
		}

		// BISHOPS/QUEENS
		let mut bishops_queens = self.pieces[C as usize][Piece::Bishop as usize]
			| self.pieces[C as usize][Piece::Queen as usize];
		while bishops_queens != 0 {
			let from = bishops_queens.trailing_zeros() as u8;
			clear_lsb(&mut bishops_queens);
			let bishop_targets = get_sliding_attacks::<true>(from, self.occupied) & self.empty;
			Self::serialize_with_to_bb(from, bishop_targets, MoveFlag::Quiet, buf);
		}

		// ROOKS/QUEENS
		let mut rooks_queens = self.pieces[C as usize][Piece::Rook as usize]
			| self.pieces[C as usize][Piece::Queen as usize];
		while rooks_queens != 0 {
			let from = rooks_queens.trailing_zeros() as u8;
			clear_lsb(&mut rooks_queens);
			let rook_targets = get_sliding_attacks::<false>(from, self.occupied) & self.empty;
			Self::serialize_with_to_bb(from, rook_targets, MoveFlag::Quiet, buf);
		}

		// KING
		let king = self.pieces[C as usize][Piece::King as usize];
		{
			let from = king.trailing_zeros() as u8;
			let king_targets = get_king_attacks(from) & self.empty;
			Self::serialize_with_to_bb(from, king_targets, MoveFlag::Quiet, buf);
		}

		if self.king_castle_flags[C as usize] {
			let rook_sq = match C {
				WHITE => Squares::F1 as u8,
				BLACK => Squares::F8 as u8,
				_ => 0,
			};

			let king_sq = match C {
				WHITE => Squares::E1 as u8,
				BLACK => Squares::E8 as u8,
				_ => 0,
			};

			let can_castle = match C ^ 1 {
				WHITE => {
					!self.is_square_attacked::<WHITE>(rook_sq)
						&& !self.is_square_attacked::<WHITE>(king_sq)
				}
				BLACK => {
					!self.is_square_attacked::<BLACK>(rook_sq)
						&& !self.is_square_attacked::<BLACK>(king_sq)
				}
				_ => false,
			} && (self.occupied & KING_CASTLE_MASKS[C as usize] == 0);

			if can_castle {
				let m = Move::new(
					king_sq,
					king_sq.wrapping_add_signed(direction::EE),
					MoveFlag::KCastle,
				);
				buf.push(m);
			}
		}

		if self.queen_castle_flags[C as usize] {
			let rook_sq = match C {
				WHITE => Squares::D1 as u8,
				BLACK => Squares::D8 as u8,
				_ => 0,
			};

			let king_sq = match C {
				WHITE => Squares::E1 as u8,
				BLACK => Squares::E8 as u8,
				_ => 0,
			};

			let can_castle = match C ^ 1 {
				WHITE => {
					!self.is_square_attacked::<WHITE>(rook_sq)
						&& !self.is_square_attacked::<WHITE>(king_sq)
				}
				BLACK => {
					!self.is_square_attacked::<BLACK>(rook_sq)
						&& !self.is_square_attacked::<BLACK>(king_sq)
				}
				_ => false,
			} && (self.occupied & QUEEN_CASTLE_MASKS[C as usize] == 0);

			if can_castle {
				let m = Move::new(
					king_sq,
					king_sq.wrapping_add_signed(direction::WW),
					MoveFlag::QCastle,
				);
				buf.push(m);
			}
		}
	}

	pub fn gen_all_pseudo_legal_moves<const C: u8>(&self, buf: &mut MoveList) {
		let targets = self.get_opponent_bb::<C>();

		// PAWNS
		let friendly_pawns = self.pieces[C as usize][Piece::Pawn as usize];
		let promo_pawns = friendly_pawns
			& match C {
				WHITE => RANK_7,
				BLACK => RANK_2,
				_ => 0,
			};
		let no_promo_pawns = friendly_pawns ^ promo_pawns;

		let no_promo_pawns_capt_west =
			get_pawns_able_to_attack_west_bb::<C>(no_promo_pawns, targets);
		let no_promo_pawns_capt_east =
			get_pawns_able_to_attack_east_bb::<C>(no_promo_pawns, targets);
		let promo_pawns_capt_west = get_pawns_able_to_attack_west_bb::<C>(promo_pawns, targets);
		let promo_pawns_capt_east = get_pawns_able_to_attack_east_bb::<C>(promo_pawns, targets);

		let promo_single_push = get_pawns_able_to_push::<C>(promo_pawns, self.empty);
		let no_promo_single_push = get_pawns_able_to_push::<C>(no_promo_pawns, self.empty);
		let no_promo_double_push = get_pawns_able_to_double_push::<C>(no_promo_pawns, self.empty);

		// Partition into pawns that can only single push
		let no_promo_single_push = no_promo_double_push ^ no_promo_single_push;

		let pawn_capt_west_offset = match C {
			WHITE => direction::NW,
			BLACK => direction::SW,
			_ => 0,
		};

		let pawn_capt_east_offset = match C {
			WHITE => direction::NE,
			BLACK => direction::SE,
			_ => 0,
		};

		let single_push_off = match C {
			WHITE => N,
			BLACK => S,
			_ => 0,
		};

		let double_push_off = match C {
			WHITE => 2 * N,
			BLACK => 2 * S,
			_ => 0,
		};

		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_pawns_capt_west,
			pawn_capt_west_offset,
			MoveFlag::Captures,
			buf,
		);

		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_pawns_capt_east,
			pawn_capt_east_offset,
			MoveFlag::Captures,
			buf,
		);

		Self::serialize_with_offset::<{ serialization_flags::PROMO_CAPTURES }>(
			promo_pawns_capt_west,
			pawn_capt_west_offset,
			MoveFlag::Captures,
			buf,
		);

		Self::serialize_with_offset::<{ serialization_flags::PROMO_CAPTURES }>(
			promo_pawns_capt_east,
			pawn_capt_east_offset,
			MoveFlag::Captures,
			buf,
		);

		if let Some(en_passant) = self.en_passant {
			let mut en_passant_pawns = match C ^ 1 {
				WHITE => get_pawn_attacks::<{ WHITE }>(en_passant.get()),
				BLACK => get_pawn_attacks::<{ BLACK }>(en_passant.get()),
				_ => 0,
			} & friendly_pawns;

			while en_passant_pawns != 0 {
				let from = en_passant_pawns.trailing_zeros() as u8;
				clear_lsb(&mut en_passant_pawns);
				let m = Move::new(from, en_passant.get(), MoveFlag::EpCaptures);
				buf.push(m);
			}
		}

		Self::serialize_with_offset::<{ serialization_flags::PROMOTIONS }>(
			promo_single_push,
			single_push_off,
			MoveFlag::Quiet,
			buf,
		);
		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_single_push,
			single_push_off,
			MoveFlag::Quiet,
			buf,
		);
		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_double_push,
			single_push_off,
			MoveFlag::Quiet,
			buf,
		);
		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_double_push,
			double_push_off,
			MoveFlag::DoublePush,
			buf,
		);

		// KNIGHTS
		let mut knights = self.pieces[C as usize][Piece::Knight as usize];
		while knights != 0 {
			let from = knights.trailing_zeros() as u8;
			clear_lsb(&mut knights);
			let knight_attacks = get_knight_attacks(from);
			let knight_attack_targets = knight_attacks & targets;
			let knight_targets = knight_attacks & self.empty;
			Self::serialize_with_to_bb(from, knight_attack_targets, MoveFlag::Captures, buf);
			Self::serialize_with_to_bb(from, knight_targets, MoveFlag::Quiet, buf);
		}

		// BISHOPS/QUEENS
		let mut bishops_queens = self.pieces[C as usize][Piece::Bishop as usize]
			| self.pieces[C as usize][Piece::Queen as usize];
		while bishops_queens != 0 {
			let from = bishops_queens.trailing_zeros() as u8;
			clear_lsb(&mut bishops_queens);
			let bishop_attacks = get_sliding_attacks::<true>(from, self.occupied);
			let bishop_attack_targets = bishop_attacks & targets;
			let bishop_targets = bishop_attacks & self.empty;
			Self::serialize_with_to_bb(from, bishop_attack_targets, MoveFlag::Captures, buf);
			Self::serialize_with_to_bb(from, bishop_targets, MoveFlag::Quiet, buf);
		}

		// ROOKS/QUEENS
		let mut rooks_queens = self.pieces[C as usize][Piece::Rook as usize]
			| self.pieces[C as usize][Piece::Queen as usize];
		while rooks_queens != 0 {
			let from = rooks_queens.trailing_zeros() as u8;
			clear_lsb(&mut rooks_queens);
			let rook_attacks = get_sliding_attacks::<false>(from, self.occupied);
			let rook_attack_targets = rook_attacks & targets;
			let rook_targets = rook_attacks & self.empty;
			Self::serialize_with_to_bb(from, rook_attack_targets, MoveFlag::Captures, buf);
			Self::serialize_with_to_bb(from, rook_targets, MoveFlag::Quiet, buf);
		}

		// KING
		let king = self.pieces[C as usize][Piece::King as usize];
		{
			let from = king.trailing_zeros() as u8;
			let king_attacks = get_king_attacks(from);
			let king_attack_targets = king_attacks & targets;
			let king_targets = king_attacks & self.empty;
			Self::serialize_with_to_bb(from, king_attack_targets, MoveFlag::Captures, buf);
			Self::serialize_with_to_bb(from, king_targets, MoveFlag::Quiet, buf);
		}

		if self.king_castle_flags[C as usize] {
			let rook_sq = match C {
				WHITE => Squares::F1 as u8,
				BLACK => Squares::F8 as u8,
				_ => 0,
			};

			let king_sq = match C {
				WHITE => Squares::E1 as u8,
				BLACK => Squares::E8 as u8,
				_ => 0,
			};

			let can_castle = match C ^ 1 {
				WHITE => {
					!self.is_square_attacked::<WHITE>(rook_sq)
						&& !self.is_square_attacked::<WHITE>(king_sq)
				}
				BLACK => {
					!self.is_square_attacked::<BLACK>(rook_sq)
						&& !self.is_square_attacked::<BLACK>(king_sq)
				}
				_ => false,
			} && (self.occupied & KING_CASTLE_MASKS[C as usize] == 0);

			if can_castle {
				let m = Move::new(
					king_sq,
					king_sq.wrapping_add_signed(direction::EE),
					MoveFlag::KCastle,
				);
				buf.push(m);
			}
		}

		if self.queen_castle_flags[C as usize] {
			let rook_sq = match C {
				WHITE => Squares::D1 as u8,
				BLACK => Squares::D8 as u8,
				_ => 0,
			};

			let king_sq = match C {
				WHITE => Squares::E1 as u8,
				BLACK => Squares::E8 as u8,
				_ => 0,
			};

			let can_castle = match C ^ 1 {
				WHITE => {
					!self.is_square_attacked::<WHITE>(rook_sq)
						&& !self.is_square_attacked::<WHITE>(king_sq)
				}
				BLACK => {
					!self.is_square_attacked::<BLACK>(rook_sq)
						&& !self.is_square_attacked::<BLACK>(king_sq)
				}
				_ => false,
			} && (self.occupied & QUEEN_CASTLE_MASKS[C as usize] == 0);

			if can_castle {
				let m = Move::new(
					king_sq,
					king_sq.wrapping_add_signed(direction::WW),
					MoveFlag::QCastle,
				);
				buf.push(m);
			}
		}
	}

	// Also tells us if we are in check
	pub fn gen_pseudo_legal_captures_in_check<const C: u8>(&self, buf: &mut MoveList) -> bool {
		// When in check: generate all king moves, moves that capture the checker, and moves to the
		// squares in between us and the checker
		let targets = self.get_opponent_bb::<C>();
		let king = self.pieces[C as usize][Piece::King as usize];
		let king_sq = king.trailing_zeros() as u8;
		let checkers_mask: u64;
		{
			let pawns = self.pieces[(C ^ 1) as usize][Piece::Pawn as usize];
			let pawn_checker_mask = match C {
				WHITE => get_pawn_attacks::<WHITE>(king_sq) & pawns,
				BLACK => get_pawn_attacks::<BLACK>(king_sq) & pawns,
				_ => 0,
			};

			let knights = self.pieces[(C ^ 1) as usize][Piece::Knight as usize];
			let knight_checker_mask = get_knight_attacks(king_sq) & knights;

			let opp_king = self.pieces[(C ^ 1) as usize][Piece::King as usize];
			let king_checker_mask = get_king_attacks(king_sq) & opp_king;

			let bishops_queens = self.pieces[(C ^ 1) as usize][Piece::Bishop as usize]
				| self.pieces[(C ^ 1) as usize][Piece::Queen as usize];
			let bishops_queens_checker_mask =
				get_sliding_attacks::<true>(king_sq, self.occupied) & bishops_queens;

			let rooks_queens = self.pieces[(C ^ 1) as usize][Piece::Rook as usize]
				| self.pieces[(C ^ 1) as usize][Piece::Queen as usize];
			let rooks_queens_checker_mask =
				get_sliding_attacks::<false>(king_sq, self.occupied) & rooks_queens;
			checkers_mask = pawn_checker_mask
				| knight_checker_mask
				| king_checker_mask
				| bishops_queens_checker_mask
				| rooks_queens_checker_mask;
		}

		if checkers_mask == 0 {
			self.gen_pseudo_legal_captures::<{ C }>(buf);
			return false;
		}

		let king_attacks = get_king_attacks(king_sq);
		let king_attack_targets = king_attacks & targets;
		Self::serialize_with_to_bb(king_sq, king_attack_targets, MoveFlag::Captures, buf);

		// If in double check, stop at king moves
		if checkers_mask.count_ones() >= 2 {
			return true;
		}

		let checker_sq: u8 = checkers_mask.trailing_zeros() as u8;

		let friendly_pawns = self.pieces[C as usize][Piece::Pawn as usize];
		let promo_pawns = friendly_pawns
			& match C {
				WHITE => RANK_7,
				BLACK => RANK_2,
				_ => 0,
			};
		let no_promo_pawns = friendly_pawns ^ promo_pawns;

		let no_promo_pawns_capt_west =
			get_pawns_able_to_attack_west_bb::<C>(no_promo_pawns, checkers_mask);
		let no_promo_pawns_capt_east =
			get_pawns_able_to_attack_east_bb::<C>(no_promo_pawns, checkers_mask);
		let promo_pawns_capt_west =
			get_pawns_able_to_attack_west_bb::<C>(promo_pawns, checkers_mask);
		let promo_pawns_capt_east =
			get_pawns_able_to_attack_east_bb::<C>(promo_pawns, checkers_mask);

		let pawn_capt_west_offset = match C {
			WHITE => direction::NW,
			BLACK => direction::SW,
			_ => 0,
		};

		let pawn_capt_east_offset = match C {
			WHITE => direction::NE,
			BLACK => direction::SE,
			_ => 0,
		};

		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_pawns_capt_west,
			pawn_capt_west_offset,
			MoveFlag::Captures,
			buf,
		);

		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_pawns_capt_east,
			pawn_capt_east_offset,
			MoveFlag::Captures,
			buf,
		);

		Self::serialize_with_offset::<{ serialization_flags::PROMO_CAPTURES }>(
			promo_pawns_capt_west,
			pawn_capt_west_offset,
			MoveFlag::Captures,
			buf,
		);

		Self::serialize_with_offset::<{ serialization_flags::PROMO_CAPTURES }>(
			promo_pawns_capt_east,
			pawn_capt_east_offset,
			MoveFlag::Captures,
			buf,
		);

		let en_passant_victim_off = match C {
			WHITE => direction::S,
			BLACK => direction::N,
			_ => 0,
		};
		if let Some(en_passant) = self.en_passant
			&& en_passant.get().wrapping_add_signed(en_passant_victim_off) == checker_sq
		{
			let mut en_passant_pawns = match C ^ 1 {
				WHITE => get_pawn_attacks::<{ WHITE }>(en_passant.get()),
				BLACK => get_pawn_attacks::<{ BLACK }>(en_passant.get()),
				_ => 0,
			} & friendly_pawns;

			while en_passant_pawns != 0 {
				let from = en_passant_pawns.trailing_zeros() as u8;
				clear_lsb(&mut en_passant_pawns);
				let m = Move::new(from, en_passant.get(), MoveFlag::EpCaptures);
				buf.push(m);
			}
		}

		// KNIGHTS
		let mut knights = self.pieces[C as usize][Piece::Knight as usize];
		while knights != 0 {
			let from = knights.trailing_zeros() as u8;
			clear_lsb(&mut knights);
			let knight_targets = get_knight_attacks(from) & checkers_mask;
			Self::serialize_with_to_bb(from, knight_targets, MoveFlag::Captures, buf);
		}

		// BISHOPS/QUEENS
		let mut bishops_queens = self.pieces[C as usize][Piece::Bishop as usize]
			| self.pieces[C as usize][Piece::Queen as usize];
		while bishops_queens != 0 {
			let from = bishops_queens.trailing_zeros() as u8;
			clear_lsb(&mut bishops_queens);
			let bishop_targets = get_sliding_attacks::<true>(from, self.occupied) & checkers_mask;
			Self::serialize_with_to_bb(from, bishop_targets, MoveFlag::Captures, buf);
		}

		// ROOKS/QUEENS
		let mut rooks_queens = self.pieces[C as usize][Piece::Rook as usize]
			| self.pieces[C as usize][Piece::Queen as usize];
		while rooks_queens != 0 {
			let from = rooks_queens.trailing_zeros() as u8;
			clear_lsb(&mut rooks_queens);
			let rook_targets = get_sliding_attacks::<false>(from, self.occupied) & checkers_mask;
			Self::serialize_with_to_bb(from, rook_targets, MoveFlag::Captures, buf);
		}

		true
	}

	pub fn gen_pseudo_legal_quiets_in_check<const C: u8>(&self, buf: &mut MoveList) -> bool {
		// When in check: generate all king moves, moves that capture the checker, and moves to the
		// squares in between us and the checker
		let king = self.pieces[C as usize][Piece::King as usize];
		let king_sq = king.trailing_zeros() as u8;
		let checkers_mask: u64;
		{
			let pawns = self.pieces[(C ^ 1) as usize][Piece::Pawn as usize];
			let pawn_checker_mask = match C {
				WHITE => get_pawn_attacks::<WHITE>(king_sq) & pawns,
				BLACK => get_pawn_attacks::<BLACK>(king_sq) & pawns,
				_ => 0,
			};

			let knights = self.pieces[(C ^ 1) as usize][Piece::Knight as usize];
			let knight_checker_mask = get_knight_attacks(king_sq) & knights;

			let opp_king = self.pieces[(C ^ 1) as usize][Piece::King as usize];
			let king_checker_mask = get_king_attacks(king_sq) & opp_king;

			let bishops_queens = self.pieces[(C ^ 1) as usize][Piece::Bishop as usize]
				| self.pieces[(C ^ 1) as usize][Piece::Queen as usize];
			let bishops_queens_checker_mask =
				get_sliding_attacks::<true>(king_sq, self.occupied) & bishops_queens;

			let rooks_queens = self.pieces[(C ^ 1) as usize][Piece::Rook as usize]
				| self.pieces[(C ^ 1) as usize][Piece::Queen as usize];
			let rooks_queens_checker_mask =
				get_sliding_attacks::<false>(king_sq, self.occupied) & rooks_queens;
			checkers_mask = pawn_checker_mask
				| knight_checker_mask
				| king_checker_mask
				| bishops_queens_checker_mask
				| rooks_queens_checker_mask;
		}

		if checkers_mask == 0 {
			self.gen_pseudo_legal_quiets::<{ C }>(buf);
			return false;
		}

		let king_attacks = get_king_attacks(king_sq);
		let king_targets = king_attacks & self.empty;
		Self::serialize_with_to_bb(king_sq, king_targets, MoveFlag::Quiet, buf);

		// If in double check, stop at king moves
		if checkers_mask.count_ones() >= 2 {
			return true;
		}

		let checker_sq: u8 = checkers_mask.trailing_zeros() as u8;

		// NOTE: Since we are in check, this mask must be empty
		let target_squares_mask = in_between_mask(king_sq, checker_sq);
		// PAWNS
		let friendly_pawns = self.pieces[C as usize][Piece::Pawn as usize];
		let promo_pawns = friendly_pawns
			& match C {
				WHITE => RANK_7,
				BLACK => RANK_2,
				_ => 0,
			};
		let no_promo_pawns = friendly_pawns ^ promo_pawns;
		let promo_single_push = get_pawns_able_to_push::<C>(promo_pawns, target_squares_mask);
		let no_promo_single_push = get_pawns_able_to_push::<C>(no_promo_pawns, target_squares_mask);
		let no_promo_double_push = get_pawns_able_to_double_push::<C>(
			no_promo_pawns,
			target_squares_mask
				| (self.empty
					& match C {
						WHITE => shift_bb::<{ direction::S }>(target_squares_mask),
						BLACK => shift_bb::<{ direction::N }>(target_squares_mask),
						_ => 0,
					}),
		);

		// Partition into pawns that can only single push
		let no_promo_single_push = no_promo_double_push ^ no_promo_single_push;
		let single_push_off = match C {
			WHITE => N,
			BLACK => S,
			_ => 0,
		};

		let double_push_off = match C {
			WHITE => 2 * N,
			BLACK => 2 * S,
			_ => 0,
		};

		Self::serialize_with_offset::<{ serialization_flags::PROMOTIONS }>(
			promo_single_push,
			single_push_off,
			MoveFlag::Quiet,
			buf,
		);
		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_single_push,
			single_push_off,
			MoveFlag::Quiet,
			buf,
		);
		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_double_push,
			single_push_off,
			MoveFlag::Quiet,
			buf,
		);
		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_double_push,
			double_push_off,
			MoveFlag::DoublePush,
			buf,
		);

		// KNIGHTS
		let mut knights = self.pieces[C as usize][Piece::Knight as usize];
		while knights != 0 {
			let from = knights.trailing_zeros() as u8;
			clear_lsb(&mut knights);
			let knight_targets = get_knight_attacks(from) & target_squares_mask;
			Self::serialize_with_to_bb(from, knight_targets, MoveFlag::Quiet, buf);
		}

		// BISHOPS/QUEENS
		let mut bishops_queens = self.pieces[C as usize][Piece::Bishop as usize]
			| self.pieces[C as usize][Piece::Queen as usize];
		while bishops_queens != 0 {
			let from = bishops_queens.trailing_zeros() as u8;
			clear_lsb(&mut bishops_queens);
			let bishop_targets =
				get_sliding_attacks::<true>(from, self.occupied) & target_squares_mask;
			Self::serialize_with_to_bb(from, bishop_targets, MoveFlag::Quiet, buf);
		}

		// ROOKS/QUEENS
		let mut rooks_queens = self.pieces[C as usize][Piece::Rook as usize]
			| self.pieces[C as usize][Piece::Queen as usize];
		while rooks_queens != 0 {
			let from = rooks_queens.trailing_zeros() as u8;
			clear_lsb(&mut rooks_queens);
			let rook_targets =
				get_sliding_attacks::<false>(from, self.occupied) & target_squares_mask;
			Self::serialize_with_to_bb(from, rook_targets, MoveFlag::Quiet, buf);
		}

		true
	}

	pub fn gen_all_pseudo_legal_moves_in_check<const C: u8>(&self, buf: &mut MoveList) -> bool {
		let targets = self.get_opponent_bb::<C>();
		let king = self.pieces[C as usize][Piece::King as usize];
		let king_sq = king.trailing_zeros() as u8;
		let checkers_mask: u64;
		{
			let pawns = self.pieces[(C ^ 1) as usize][Piece::Pawn as usize];
			let pawn_checker_mask = match C {
				WHITE => get_pawn_attacks::<WHITE>(king_sq) & pawns,
				BLACK => get_pawn_attacks::<BLACK>(king_sq) & pawns,
				_ => 0,
			};

			let knights = self.pieces[(C ^ 1) as usize][Piece::Knight as usize];
			let knight_checker_mask = get_knight_attacks(king_sq) & knights;

			let opp_king = self.pieces[(C ^ 1) as usize][Piece::King as usize];
			let king_checker_mask = get_king_attacks(king_sq) & opp_king;

			let bishops_queens = self.pieces[(C ^ 1) as usize][Piece::Bishop as usize]
				| self.pieces[(C ^ 1) as usize][Piece::Queen as usize];
			let bishops_queens_checker_mask =
				get_sliding_attacks::<true>(king_sq, self.occupied) & bishops_queens;

			let rooks_queens = self.pieces[(C ^ 1) as usize][Piece::Rook as usize]
				| self.pieces[(C ^ 1) as usize][Piece::Queen as usize];
			let rooks_queens_checker_mask =
				get_sliding_attacks::<false>(king_sq, self.occupied) & rooks_queens;
			checkers_mask = pawn_checker_mask
				| knight_checker_mask
				| king_checker_mask
				| bishops_queens_checker_mask
				| rooks_queens_checker_mask;
		}

		if checkers_mask == 0 {
			self.gen_all_pseudo_legal_moves::<{ C }>(buf);
			return false;
		}

		let king_attacks = get_king_attacks(king_sq);
		let king_targets = king_attacks & self.empty;
		let king_attack_targets = king_attacks & targets;
		Self::serialize_with_to_bb(king_sq, king_attack_targets, MoveFlag::Captures, buf);
		Self::serialize_with_to_bb(king_sq, king_targets, MoveFlag::Quiet, buf);

		// If in double check, stop at king moves
		if checkers_mask.count_ones() >= 2 {
			return true;
		}

		let checker_sq: u8 = checkers_mask.trailing_zeros() as u8;
		// NOTE: Since we are in check, this mask must be empty
		let target_squares_mask = in_between_mask(king_sq, checker_sq);

		let friendly_pawns = self.pieces[C as usize][Piece::Pawn as usize];
		let promo_pawns = friendly_pawns
			& match C {
				WHITE => RANK_7,
				BLACK => RANK_2,
				_ => 0,
			};
		let no_promo_pawns = friendly_pawns ^ promo_pawns;

		let no_promo_pawns_capt_west =
			get_pawns_able_to_attack_west_bb::<C>(no_promo_pawns, checkers_mask);
		let no_promo_pawns_capt_east =
			get_pawns_able_to_attack_east_bb::<C>(no_promo_pawns, checkers_mask);
		let promo_pawns_capt_west =
			get_pawns_able_to_attack_west_bb::<C>(promo_pawns, checkers_mask);
		let promo_pawns_capt_east =
			get_pawns_able_to_attack_east_bb::<C>(promo_pawns, checkers_mask);
		let promo_single_push = get_pawns_able_to_push::<C>(promo_pawns, target_squares_mask);
		let no_promo_single_push = get_pawns_able_to_push::<C>(no_promo_pawns, target_squares_mask);
		let no_promo_double_push = get_pawns_able_to_double_push::<C>(
			no_promo_pawns,
			target_squares_mask
				| (self.empty
					& match C {
						WHITE => shift_bb::<{ direction::S }>(target_squares_mask),
						BLACK => shift_bb::<{ direction::N }>(target_squares_mask),
						_ => 0,
					}),
		);

		// Partition into pawns that can only single push
		let no_promo_single_push = no_promo_double_push ^ no_promo_single_push;

		let pawn_capt_west_offset = match C {
			WHITE => direction::NW,
			BLACK => direction::SW,
			_ => 0,
		};

		let pawn_capt_east_offset = match C {
			WHITE => direction::NE,
			BLACK => direction::SE,
			_ => 0,
		};

		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_pawns_capt_west,
			pawn_capt_west_offset,
			MoveFlag::Captures,
			buf,
		);

		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_pawns_capt_east,
			pawn_capt_east_offset,
			MoveFlag::Captures,
			buf,
		);

		Self::serialize_with_offset::<{ serialization_flags::PROMO_CAPTURES }>(
			promo_pawns_capt_west,
			pawn_capt_west_offset,
			MoveFlag::Captures,
			buf,
		);

		Self::serialize_with_offset::<{ serialization_flags::PROMO_CAPTURES }>(
			promo_pawns_capt_east,
			pawn_capt_east_offset,
			MoveFlag::Captures,
			buf,
		);

		let en_passant_victim_off = match C {
			WHITE => direction::S,
			BLACK => direction::N,
			_ => 0,
		};
		if let Some(en_passant) = self.en_passant
			&& en_passant.get().wrapping_add_signed(en_passant_victim_off) == checker_sq
		{
			let mut en_passant_pawns = match C ^ 1 {
				WHITE => get_pawn_attacks::<{ WHITE }>(en_passant.get()),
				BLACK => get_pawn_attacks::<{ BLACK }>(en_passant.get()),
				_ => 0,
			} & friendly_pawns;

			while en_passant_pawns != 0 {
				let from = en_passant_pawns.trailing_zeros() as u8;
				clear_lsb(&mut en_passant_pawns);
				let m = Move::new(from, en_passant.get(), MoveFlag::EpCaptures);
				buf.push(m);
			}
		}

		let single_push_off = match C {
			WHITE => N,
			BLACK => S,
			_ => 0,
		};

		let double_push_off = match C {
			WHITE => 2 * N,
			BLACK => 2 * S,
			_ => 0,
		};

		Self::serialize_with_offset::<{ serialization_flags::PROMOTIONS }>(
			promo_single_push,
			single_push_off,
			MoveFlag::Quiet,
			buf,
		);
		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_single_push,
			single_push_off,
			MoveFlag::Quiet,
			buf,
		);
		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_double_push,
			single_push_off,
			MoveFlag::Quiet,
			buf,
		);
		Self::serialize_with_offset::<{ serialization_flags::CUSTOM }>(
			no_promo_double_push,
			double_push_off,
			MoveFlag::DoublePush,
			buf,
		);

		// KNIGHTS
		let mut knights = self.pieces[C as usize][Piece::Knight as usize];
		while knights != 0 {
			let from = knights.trailing_zeros() as u8;
			clear_lsb(&mut knights);
			let knight_attacks = get_knight_attacks(from);
			let knight_attack_targets = knight_attacks & checkers_mask;
			let knight_targets = knight_attacks & target_squares_mask;
			Self::serialize_with_to_bb(from, knight_attack_targets, MoveFlag::Captures, buf);
			Self::serialize_with_to_bb(from, knight_targets, MoveFlag::Quiet, buf);
		}

		// BISHOPS/QUEENS
		let mut bishops_queens = self.pieces[C as usize][Piece::Bishop as usize]
			| self.pieces[C as usize][Piece::Queen as usize];
		while bishops_queens != 0 {
			let from = bishops_queens.trailing_zeros() as u8;
			clear_lsb(&mut bishops_queens);
			let bishop_attacks = get_sliding_attacks::<true>(from, self.occupied);
			let bishop_attack_targets = bishop_attacks & checkers_mask;
			let bishop_targets = bishop_attacks & target_squares_mask;
			Self::serialize_with_to_bb(from, bishop_attack_targets, MoveFlag::Captures, buf);
			Self::serialize_with_to_bb(from, bishop_targets, MoveFlag::Quiet, buf);
		}

		// ROOKS/QUEENS
		let mut rooks_queens = self.pieces[C as usize][Piece::Rook as usize]
			| self.pieces[C as usize][Piece::Queen as usize];
		while rooks_queens != 0 {
			let from = rooks_queens.trailing_zeros() as u8;
			clear_lsb(&mut rooks_queens);
			let rook_attacks = get_sliding_attacks::<false>(from, self.occupied);
			let rook_attack_targets = rook_attacks & checkers_mask;
			let rook_targets = rook_attacks & target_squares_mask;
			Self::serialize_with_to_bb(from, rook_attack_targets, MoveFlag::Captures, buf);
			Self::serialize_with_to_bb(from, rook_targets, MoveFlag::Quiet, buf);
		}

		true
	}

	pub fn gen_pseudo_legal_moves<const S: u8, const C: u8>(&self, buf: &mut MoveList) {
		match S {
			move_gen_stages::CAPTURES => self.gen_pseudo_legal_captures::<C>(buf),
			move_gen_stages::QUIETS => self.gen_pseudo_legal_quiets::<C>(buf),
			move_gen_stages::ALL => self.gen_all_pseudo_legal_moves::<C>(buf),
			_ => {}
		};
	}

	pub fn gen_all_pseudo_legal_moves_non_monomorphizing(&self, buf: &mut MoveList) -> bool {
		// Integrates a check if no in check anyways
		match self.turn {
			Color::White => self.gen_all_pseudo_legal_moves_in_check::<WHITE>(buf),
			Color::Black => self.gen_all_pseudo_legal_moves_in_check::<BLACK>(buf),
		}
	}

	pub fn gen_pseudo_legal_captures_non_monomorphizing(&self, buf: &mut MoveList) -> bool {
		match self.turn {
			Color::White => self.gen_pseudo_legal_captures_in_check::<WHITE>(buf),
			Color::Black => self.gen_pseudo_legal_captures_in_check::<BLACK>(buf),
		}
	}

	pub fn is_pseudo_legal<const C: u8>(&self, m: &Move) -> bool {
		let (from, to, flags) = m.unpack();
		let from_piece = self.get_piece_at_square::<C>(from);
		if from_piece.is_none() {
			return false;
		}
		let from_piece = from_piece.unwrap();

		let to_mask = 1u64 << to;

		match flags {
			MoveFlag::Quiet => {
				((to_mask & self.occupied) == 0)
					&& match from_piece {
						Piece::Pawn => {
							to == from.wrapping_add_signed(match C {
								WHITE => N,
								BLACK => S,
								_ => 0,
							})
						}
						Piece::Knight => (get_knight_attacks(from) & to_mask) != 0,
						Piece::Bishop => {
							(get_sliding_attacks::<true>(from, self.occupied) & to_mask) != 0
						}
						Piece::Rook => {
							(get_sliding_attacks::<false>(from, self.occupied) & to_mask) != 0
						}
						Piece::Queen => {
							(get_sliding_attacks::<true>(from, self.occupied) & to_mask) != 0
								|| (get_sliding_attacks::<false>(from, self.occupied) & to_mask)
									!= 0
						}
						Piece::King => (get_king_attacks(from) & to_mask) != 0,
					}
			}

			MoveFlag::DoublePush => {
				(from_piece == Piece::Pawn)
					&& to
						== from.wrapping_add_signed(match C {
							WHITE => 2 * N,
							BLACK => 2 * S,
							_ => 0,
						})
			}

			MoveFlag::KCastle => {
				let (king_square, rook_square, rook_target_square) = match C {
					WHITE => (Squares::E1 as u8, Squares::H1 as u8, Squares::F1 as u8),
					BLACK => (Squares::E8 as u8, Squares::H8 as u8, Squares::F8 as u8),
					_ => (0, 0, 0),
				};

				let rook_mask = 1u64 << rook_square;

				self.king_castle_flags[C as usize]
					&& (from_piece == Piece::King)
					&& (from == king_square)
					&& (self.pieces[C as usize][Piece::Rook as usize] & rook_mask) != 0
					&& (self.occupied & KING_CASTLE_MASKS[C as usize]) == 0
					&& match C {
						WHITE => !self.is_square_attacked::<BLACK>(rook_target_square),
						BLACK => !self.is_square_attacked::<WHITE>(rook_target_square),
						_ => false,
					}
			}

			MoveFlag::QCastle => {
				let (king_square, rook_square, rook_target_square) = match C {
					WHITE => (Squares::E1 as u8, Squares::A1 as u8, Squares::D1 as u8),
					BLACK => (Squares::E8 as u8, Squares::A8 as u8, Squares::D8 as u8),
					_ => (0, 0, 0),
				};

				let rook_mask = 1u64 << rook_square;

				self.queen_castle_flags[C as usize]
					&& (from_piece == Piece::King)
					&& (from == king_square)
					&& (self.pieces[C as usize][Piece::Rook as usize] & rook_mask) != 0
					&& (self.occupied & QUEEN_CASTLE_MASKS[C as usize]) == 0
					&& match C {
						WHITE => !self.is_square_attacked::<BLACK>(rook_target_square),
						BLACK => !self.is_square_attacked::<WHITE>(rook_target_square),
						_ => false,
					}
			}

			MoveFlag::Captures => {
				((to_mask & self.pieces_by_color[C as usize ^ 1]) != 0)
					&& match from_piece {
						Piece::Pawn => get_pawn_attacks::<C>(from) & to_mask != 0,
						Piece::Knight => (get_knight_attacks(from) & to_mask) != 0,
						Piece::Bishop => {
							(get_sliding_attacks::<true>(from, self.occupied) & to_mask) != 0
						}
						Piece::Rook => {
							(get_sliding_attacks::<false>(from, self.occupied) & to_mask) != 0
						}
						Piece::Queen => {
							(get_sliding_attacks::<true>(from, self.occupied) & to_mask) != 0
								|| (get_sliding_attacks::<false>(from, self.occupied) & to_mask)
									!= 0
						}
						Piece::King => (get_king_attacks(from) & to_mask) != 0,
					}
			}

			MoveFlag::EpCaptures => {
				(from_piece == Piece::Pawn)
					&& self.en_passant.is_some()
					&& self.en_passant.unwrap().get() == to
			}

			MoveFlag::KnightPromotion
			| MoveFlag::BishopPromotion
			| MoveFlag::RookPromotion
			| MoveFlag::QueenPromotion => {
				(from_piece == Piece::Pawn)
					&& (to_mask & self.occupied) == 0
					&& (to_mask
						& match C {
							WHITE => RANK_8,
							BLACK => RANK_1,
							_ => 0,
						}) != 0
			}

			MoveFlag::KnightPromoCapture
			| MoveFlag::BishopPromoCapture
			| MoveFlag::RookPromoCapture
			| MoveFlag::QueenPromoCapture => {
				(from_piece == Piece::Pawn)
					&& (to_mask & self.pieces_by_color[C as usize ^ 1]) != 0
					&& (to_mask & get_pawn_attacks::<C>(from)) != 0
			}
		}
	}
}
