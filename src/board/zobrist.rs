// ToDo: Test zobrist hashes someday

use crate::board::{Board, utils::PieceColorPair};

const fn fast_hash(state: u64) -> u64 {
	let mut h = state;
	h ^= h >> 23;
	h = h.wrapping_mul(0x2127599bf4325c37);
	h ^= h >> 47;
	h
}

const PIECE_IDX_OFF: usize = 0;
const SIDE_TO_MOVE_IDX: usize = 768;
// 16 castling entries for speed
const CASTLE_RIGHTS_IDX_OFF: usize = 769;
// 64 en passant entries for speed
const EN_PASSANT_SQUARE_IDX_OFF: usize = 785;
const NUM_ZOBRIST_ENTRIES: usize = 849;

static ZOBRIST_TABLE: [u64; NUM_ZOBRIST_ENTRIES] = {
	let mut table: [u64; NUM_ZOBRIST_ENTRIES] = [0; NUM_ZOBRIST_ENTRIES];
	let mut state = 1;
	let mut idx: usize = 0;

	loop {
		if idx >= NUM_ZOBRIST_ENTRIES {
			break;
		}

		table[idx] = fast_hash(state);
		state += 1;

		idx += 1;
	}

	// Special handling for castling flags
	let wking_castle = fast_hash(state);
	let wqueen_castle = fast_hash(state + 1);
	let bking_castle = fast_hash(state + 2);
	let bqueen_castle = fast_hash(state + 3);
	idx = CASTLE_RIGHTS_IDX_OFF;
	loop {
		if idx >= EN_PASSANT_SQUARE_IDX_OFF {
			break;
		}

		let rights = (idx - CASTLE_RIGHTS_IDX_OFF) as u8;
		table[idx] = if rights & Board::WKING_CASTLE != 0 {
			wking_castle
		} else {
			0
		} ^ if rights & Board::BKING_CASTLE != 0 {
			bking_castle
		} else {
			0
		} ^ if rights & Board::WQUEEN_CASTLE != 0 {
			wqueen_castle
		} else {
			0
		} ^ if rights & Board::BQUEEN_CASTLE != 0 {
			bqueen_castle
		} else {
			0
		};

		idx += 1;
	}

	table
};

pub enum ZobristDelta {
	PutRemove(PieceColorPair, u8),
	WhiteTurn,
	CastleRights(u8),
	EnPassantSquareChange(u8),
}

fn get_zobrist_idx(delta: ZobristDelta) -> usize {
	match delta {
		ZobristDelta::PutRemove(PieceColorPair(piece, color), sq) => {
			PIECE_IDX_OFF + (color as usize + sq as usize * 2 + piece as usize * 128)
		}
		ZobristDelta::WhiteTurn => SIDE_TO_MOVE_IDX,
		ZobristDelta::CastleRights(rights) => CASTLE_RIGHTS_IDX_OFF + rights as usize,
		ZobristDelta::EnPassantSquareChange(sq) => EN_PASSANT_SQUARE_IDX_OFF + sq as usize,
	}
}

impl Board {
	pub(super) fn apply_zobrist_delta(&mut self, delta: ZobristDelta) {
		let idx = get_zobrist_idx(delta);
		let hash_update = ZOBRIST_TABLE[idx];
		self.zobrist ^= hash_update;
	}
}
