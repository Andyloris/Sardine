// ToDo: Test zobrist hashes someday

use crate::board::{
	Board,
	utils::{Color, PieceColorPair},
};

const fn fast_hash(state: u64) -> u64 {
	let mut h = state;
	h ^= h >> 23;
	h = h.wrapping_mul(0x2127599bf4325c37);
	h ^= h >> 47;
	h
}

const PIECE_IDX_OFF: usize = 0;
const SIDE_TO_MOVE_IDX: usize = 768;
// ToDo: Pack castle rights into a 4 bit value so that we can simply use a 16 bit index into the
// zobrist table
const CASTLE_RIGHTS_IDX_OFF: usize = 769;
// 64 en passant entries for speed
const EN_PASSANT_SQUARE_IDX_OFF: usize = 773;
const NUM_ZOBRIST_ENTRIES: usize = 837;

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

	table
};

pub enum ZobristDelta {
	PutRemove(PieceColorPair, u8),
	WhiteTurn,
	KCastleRights(Color),
	QCastleRights(Color),
	EnPassantSquareChange(u8),
}

fn get_zobrist_idx(delta: ZobristDelta) -> usize {
	match delta {
		ZobristDelta::PutRemove(PieceColorPair(piece, color), sq) => {
			PIECE_IDX_OFF + (color as usize + sq as usize * 2 + piece as usize * 128)
		}
		ZobristDelta::WhiteTurn => SIDE_TO_MOVE_IDX,
		ZobristDelta::KCastleRights(color) => CASTLE_RIGHTS_IDX_OFF + color as usize,
		ZobristDelta::QCastleRights(color) => CASTLE_RIGHTS_IDX_OFF + 2 + color as usize,
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
