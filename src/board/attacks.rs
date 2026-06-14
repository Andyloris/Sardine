use crate::board::{
	sliding_attack_table::{BISHOP_MAGICS, Magic, ROOK_MAGICS, SLIDING_ATTACKS},
	utils::{BLACK, WHITE, direction, shift_bb},
};

pub const fn get_pawn_attacks_bb<const C: u8>(bb: u64) -> u64 {
	match C {
		WHITE => shift_bb::<{ direction::NE }>(bb) | shift_bb::<{ direction::NW }>(bb),
		BLACK => shift_bb::<{ direction::SE }>(bb) | shift_bb::<{ direction::SW }>(bb),
		_ => 0,
	}
}

pub const fn get_pawns_able_to_attack_west_bb<const C: u8>(bb: u64, opp: u64) -> u64 {
	match C {
		WHITE => shift_bb::<{ direction::SE }>(opp) & bb,
		BLACK => shift_bb::<{ direction::NE }>(opp) & bb,
		_ => 0,
	}
}

pub const fn get_pawns_able_to_attack_east_bb<const C: u8>(bb: u64, opp: u64) -> u64 {
	match C {
		WHITE => shift_bb::<{ direction::SW }>(opp) & bb,
		BLACK => shift_bb::<{ direction::NW }>(opp) & bb,
		_ => 0,
	}
}

pub const fn get_knight_attacks_bb(bb: u64) -> u64 {
	let l1: u64 = shift_bb::<{ direction::W }>(bb);
	let l2: u64 = shift_bb::<{ direction::WW }>(bb);
	let r1: u64 = shift_bb::<{ direction::E }>(bb);
	let r2: u64 = shift_bb::<{ direction::EE }>(bb);
	let h1: u64 = l1 | r1;
	let h2: u64 = l2 | r2;

	shift_bb::<{ direction::NN }>(h1)
		| shift_bb::<{ direction::SS }>(h1)
		| shift_bb::<{ direction::N }>(h2)
		| shift_bb::<{ direction::S }>(h2)
}

pub const fn get_king_attacks_bb(bb: u64) -> u64 {
	let attacks = shift_bb::<{ direction::E }>(bb) | shift_bb::<{ direction::W }>(bb);
	attacks
		| shift_bb::<{ direction::N }>(bb | attacks)
		| shift_bb::<{ direction::S }>(bb | attacks)
}

static PAWN_ATTACKS_TABLE: [[u64; 64]; 2] = {
	let mut table = [[0; 64]; 2];
	let mut sq = 0;

	while sq < 64 {
		table[0][sq] = get_pawn_attacks_bb::<0>(1u64 << sq);
		sq += 1;
	}

	sq = 0;

	while sq < 64 {
		table[1][sq] = get_pawn_attacks_bb::<1>(1u64 << sq);
		sq += 1;
	}

	table
};

static KNIGHT_ATTACKS_TABLE: [u64; 64] = {
	let mut table = [0; 64];
	let mut sq = 0;

	while sq < 64 {
		table[sq] = get_knight_attacks_bb(1u64 << sq);
		sq += 1;
	}
	table
};

static KING_ATTACKS_TABLE: [u64; 64] = {
	let mut table = [0; 64];
	let mut sq = 0;

	while sq < 64 {
		table[sq] = get_king_attacks_bb(1u64 << sq);
		sq += 1;
	}
	table
};

pub const fn get_pawn_attacks<const C: u8>(sq: u8) -> u64 {
	PAWN_ATTACKS_TABLE[C as usize][sq as usize]
}

pub const fn get_knight_attacks(sq: u8) -> u64 {
	KNIGHT_ATTACKS_TABLE[sq as usize]
}

pub const fn get_king_attacks(sq: u8) -> u64 {
	KING_ATTACKS_TABLE[sq as usize]
}

#[inline(always)]
#[cfg(not(feature = "pext_magics"))]
const fn magic_hash(magic: &Magic, occ: u64) -> usize {
	return (((occ & magic.mask) * magic.magic) >> magic.shift) as usize;
}

#[inline(always)]
#[cfg(feature = "pext_magics")]
fn magic_hash(magic: &Magic, occ: u64) -> usize {
	unsafe { std::arch::x86_64::_pext_u64(occ, magic.mask) as usize }
}

// NOTE: Do not mask the occupancy before calling this
pub fn get_sliding_attacks<const IS_BISHOP: bool>(sq: u8, occ: u64) -> u64 {
	let magic: &Magic = if IS_BISHOP {
		&BISHOP_MAGICS[sq as usize]
	} else {
		&ROOK_MAGICS[sq as usize]
	};

	SLIDING_ATTACKS[magic_hash(magic, occ) + magic.offset]
}
