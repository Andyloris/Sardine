use crate::board::utils::{direction, shift_bb};

// From https://www.chessprogramming.org/Square_Attacked_By#InBetween
const fn calc_in_between(sq1: u8, sq2: u8) -> u64 {
	const M1: u64 = 0xFFFFFFFFFFFFFFFF;
	const A2A7: u64 = 0x0001010101010100;
	const B2G7: u64 = 0x0040201008040200;
	const H1B7: u64 = 0x0002040810204080;

	let btwn = (M1 << sq1) ^ (M1 << sq2);
	let file: u64 = (sq2 as u64 & 7).wrapping_sub(sq1 as u64 & 7);
	let rank: u64 = ((sq2 as u64 | 7).wrapping_sub(sq1 as u64)) >> 3;
	let mut line = ((file & 7).wrapping_sub(1)) & A2A7; /* a2a7 if same file */
	line += 2 * (((rank & 7).wrapping_sub(1)) >> 58); /* b1g1 if same rank */
	line += (((rank.wrapping_sub(file)) & 15).wrapping_sub(1)) & B2G7; /* b2g7 if same diagonal */
	line += (((rank.wrapping_add(file)) & 15).wrapping_sub(1)) & H1B7; /* h1b7 if same antidiag */
	line = line.wrapping_mul(btwn & btwn.wrapping_neg()); /* mul acts like shift by smaller square */
	line & btwn
}

const fn calc_line_bb(sq1: u8, sq2: u8) -> u64 {
	if sq1 == sq2 {
		return 0;
	}

	let mut in_between = calc_in_between(sq1, sq2);
	let rank_1 = (sq1 >> 3) as i8;
	let rank_2 = (sq2 >> 3) as i8;
	let file_1 = (sq1 & 7) as i8;
	let file_2 = (sq2 & 7) as i8;
	let delta_1 = rank_2 - rank_1;
	let delta_2 = file_2 - file_1;

	// Alignement check with edge case detection using chebychev distances
	if in_between == 0 {
		let dist = if delta_1.abs() > delta_2.abs() {
			delta_1.abs()
		} else {
			delta_2.abs()
		};

		if dist > 1 {
			return 0;
		}
	}

	in_between |= (1 << sq1) | (1 << sq2);

	// Choose suitable shifts to propagate the ray direction in a loop wise fashion until the ray is
	// entierly filled.

	// Cases: vertical propagate N and S, horizontal propagate W and E
	// diagonal parallel to A1H8, propagate NE and SW, diagonal parallel A8H1, propagate SE and NW

	let ns = file_1 == file_2;
	let we = rank_1 == rank_2;
	let nesw = delta_1 * delta_2 > 0;
	let senw = delta_1 * delta_2 < 0;

	if ns {
		loop {
			let tmp = in_between;
			in_between |= shift_bb::<{ direction::N }>(in_between);
			in_between |= shift_bb::<{ direction::S }>(in_between);
			if tmp == in_between {
				break;
			}
		}
	}

	if we {
		loop {
			let tmp = in_between;
			in_between |= shift_bb::<{ direction::W }>(in_between);
			in_between |= shift_bb::<{ direction::E }>(in_between);
			if tmp == in_between {
				break;
			}
		}
	}

	if nesw {
		loop {
			let tmp = in_between;
			in_between |= shift_bb::<{ direction::NE }>(in_between)
				| shift_bb::<{ direction::SW }>(in_between);
			if tmp == in_between {
				break;
			}
		}
	}

	if senw {
		loop {
			let tmp = in_between;
			in_between |= shift_bb::<{ direction::SE }>(in_between)
				| shift_bb::<{ direction::NW }>(in_between);
			if tmp == in_between {
				break;
			}
		}
	}

	in_between
}

static LINE_BB_TABLE: [[u64; 64]; 64] = {
	let mut table = [[0; 64]; 64];
	let mut sq1 = 0;
	loop {
		if sq1 >= 64 {
			break;
		}

		let mut sq2 = 0;
		loop {
			if sq2 >= 64 {
				break;
			}

			table[sq1][sq2] = calc_line_bb(sq1 as u8, sq2 as u8);

			sq2 += 1;
		}

		sq1 += 1;
	}
	table
};

static IN_BETWEEN_TABLE: [[u64; 64]; 64] = {
	let mut table = [[0; 64]; 64];
	let mut sq1 = 0;
	loop {
		if sq1 >= 64 {
			break;
		}

		let mut sq2 = 0;
		loop {
			if sq2 >= 64 {
				break;
			}

			table[sq1][sq2] = calc_in_between(sq1 as u8, sq2 as u8);

			sq2 += 1;
		}

		sq1 += 1;
	}
	table
};

pub const fn in_between_mask(sq1: u8, sq2: u8) -> u64 {
	IN_BETWEEN_TABLE[sq1 as usize][sq2 as usize]
}

pub const fn line_bb_mask(sq1: u8, sq2: u8) -> u64 {
	LINE_BB_TABLE[sq1 as usize][sq2 as usize]
}
