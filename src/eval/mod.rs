use crate::board::{
	Board,
	utils::{BLACK, Color, NUM_PIECES, PIECES, WHITE},
};

const MATERIAL_WEIGHTS: [i32; NUM_PIECES] = [100, 300, 330, 500, 900, 20000];

pub fn eval_board_objective(board: &Board) -> i32 {
	let mut material = 0;

	for piece in PIECES {
		material += MATERIAL_WEIGHTS[piece as usize]
			* (board.get_num_pieces::<{ WHITE }>(piece) as i32
				- board.get_num_pieces::<{ BLACK }>(piece) as i32);
	}

	material
}
