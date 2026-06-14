use crate::board::utils::{BLACK, RANK_4, RANK_5, WHITE, direction, shift_bb};

pub fn get_pawns_able_to_push<const C: u8>(bb: u64, empty: u64) -> u64 {
	match C {
		WHITE => shift_bb::<{ direction::S }>(empty) & bb,
		BLACK => shift_bb::<{ direction::N }>(empty) & bb,
		_ => 0,
	}
}

pub fn get_pawns_able_to_double_push<const C: u8>(bb: u64, empty: u64) -> u64 {
	match C {
		WHITE => {
			get_pawns_able_to_push::<C>(bb, shift_bb::<{ direction::S }>(empty & RANK_4) & empty)
		}
		BLACK => {
			get_pawns_able_to_push::<C>(bb, shift_bb::<{ direction::N }>(empty & RANK_5) & empty)
		}
		_ => 0,
	}
}
