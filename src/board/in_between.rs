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
