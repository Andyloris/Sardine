use shakmaty::{Chess, Position, fen::Fen};

use crate::{
	board::{
		Board,
		movegen::{Move, MoveFlag, MoveList, move_gen_stages},
		utils::{BLACK, Bitboard, Color, Squares, WHITE},
	},
	uci::UCIInstance,
};

mod board;
mod eval;
mod search;
mod tt;
mod uci;

fn perft_driver(brd: &mut Board, d: usize) -> u64 {
	if d == 0 {
		return 1;
	}

	let mut move_list = MoveList::default();

	match brd.get_turn() {
		Color::White => brd.gen_all_pseudo_legal_moves::<WHITE>(&mut move_list),
		Color::Black => brd.gen_all_pseudo_legal_moves::<BLACK>(&mut move_list),
	};

	let mut nodes = 0;
	for m in move_list.iter() {
		let undo_info = match brd.get_turn() {
			Color::White => brd.do_move::<WHITE>(m),
			Color::Black => brd.do_move::<BLACK>(m),
		}
		.unwrap();

		if match brd.get_turn() {
			Color::White => brd.is_in_check::<BLACK>(),
			Color::Black => brd.is_in_check::<WHITE>(),
		} {
			match brd.get_turn() {
				Color::Black => brd.undo_move::<WHITE>(undo_info, m),
				Color::White => brd.undo_move::<BLACK>(undo_info, m),
			};
			continue;
		}

		nodes += perft_driver(brd, d - 1);

		match brd.get_turn() {
			Color::Black => brd.undo_move::<WHITE>(undo_info, m),
			Color::White => brd.undo_move::<BLACK>(undo_info, m),
		}
		.unwrap();
	}
	nodes
}

fn shakmaty_perft(pos: &Chess, depth: usize) -> u64 {
	shakmaty::perft(pos, depth as u32)
}

fn gen_diffs<'a, 'b>(
	v1: &'a Vec<(u8, u8, shakmaty::Move)>,
	v2: &'b Vec<(u8, u8, Move)>,
) -> (Vec<&'a (u8, u8, shakmaty::Move)>, Vec<&'b (u8, u8, Move)>) {
	let mut in_v1_not_v2 = v1.iter().map(Some).collect::<Vec<_>>();
	let mut in_v2_not_v1 = v2.iter().map(Some).collect::<Vec<_>>();

	for e in v2 {
		for (idx, v1e) in v1.iter().enumerate() {
			if (v1e.0 == e.0) && (v1e.1 == e.1) {
				in_v1_not_v2[idx] = None;
			}
		}
	}

	for e in v1 {
		for (idx, v2e) in v2.iter().enumerate() {
			if (v2e.0 == e.0) && (v2e.1 == e.1) {
				in_v2_not_v1[idx] = None;
			}
		}
	}

	in_v1_not_v2.retain(|e| e.is_some());
	in_v2_not_v1.retain(|e| e.is_some());
	(
		in_v1_not_v2.into_iter().flatten().collect(),
		in_v2_not_v1.into_iter().flatten().collect(),
	)
}

fn coupled_perft(shakmaty_pos: &Chess, brd: &Board, depth: usize, pv: &mut Vec<Move>) {
	if depth == 0 {
		return;
	}

	let mut correct_moves = shakmaty_pos
		.legal_moves()
		.into_iter()
		.map(|m| {
			let uci = m.to_uci(shakmaty::CastlingMode::Standard);
			(uci.from().unwrap() as u8, uci.to().unwrap() as u8, m)
		})
		.collect::<Vec<_>>();
	correct_moves.sort_by_key(|a| (a.0, a.1));
	let mut move_list = MoveList::default();

	match brd.get_turn() {
		Color::White => brd.gen_all_pseudo_legal_moves::<WHITE>(&mut move_list),
		Color::Black => brd.gen_all_pseudo_legal_moves::<BLACK>(&mut move_list),
	};
	let mut my_moves = move_list
		.iter()
		.filter_map(|m| {
			let mut brd_cpy = brd.clone();
			match brd.get_turn() {
				Color::White => brd_cpy.do_move::<WHITE>(m),
				Color::Black => brd_cpy.do_move::<BLACK>(m),
			};

			if match brd_cpy.get_turn() {
				Color::White => brd_cpy.is_in_check::<BLACK>(),
				Color::Black => brd_cpy.is_in_check::<WHITE>(),
			} {
				None
			} else {
				let (from, to, _) = m.unpack();
				Some((from, to, *m))
			}
		})
		.collect::<Vec<_>>();
	my_moves.sort_by_key(|a| (a.0, a.1));
	let diffs = gen_diffs(&correct_moves, &my_moves);
	let mut failed = false;

	if diffs.0.len() != 0 {
		print!("Reached problematic position with pv: ");
		for pv_moves in pv.iter() {
			print!("{} ", pv_moves);
		}
		println!();
		print!("Not generating moves: ");
		for (_, _, m) in diffs.0 {
			print!("{m} ");
		}
		println!();
		failed = true;
	}

	if diffs.1.len() != 0 {
		if !failed {
			print!("Reached problematic position with pv: ");
			for pv_moves in pv.iter() {
				print!("{} ", pv_moves);
			}
			println!();
		}
		print!("Generating illegal moves: ");
		for (_, _, m) in diffs.1 {
			print!("{m} ");
		}
		println!();
		failed = true;
	}

	// Safe guard here
	if correct_moves.len() != my_moves.len() {
		if !failed {
			print!("Reached problematic position with pv: ");
			for pv_moves in pv.iter() {
				print!("{} ", pv_moves);
			}
			println!();
		}

		println!(
			"Expected {} moves, found {}",
			correct_moves.len(),
			my_moves.len()
		);

		print!("Rare guardrail activated, my_moves: ");
		for (_, _, m) in &my_moves {
			print!("{m} ");
		}
		println!();

		failed = true;
	}

	if failed {
		println!();
		return;
	}

	for (idx, m) in my_moves.iter().enumerate() {
		let correct_move = correct_moves[idx];

		pv.push(m.2);
		let mut brd_cpy = brd.clone();
		match brd.get_turn() {
			Color::White => brd_cpy.do_move::<WHITE>(&my_moves[idx].2),
			Color::Black => brd_cpy.do_move::<BLACK>(&my_moves[idx].2),
		};

		let mut shakmaty_cpy = shakmaty_pos.clone();
		shakmaty_cpy.play_unchecked(correct_move.2);
		coupled_perft(&shakmaty_cpy, &brd_cpy, depth - 1, pv);
		pv.pop();
	}
}

fn test_against_shakmaty(fen: &str, max_depth: usize) {
	// Get perft numbers between me and shakmaty at each depth
	let mut board = Board::from_fen(fen).expect("I failed to parse the FEN");
	let shakmaty_pos: Chess = {
		let fen: Fen = fen.parse().unwrap();
		fen.into_position(shakmaty::CastlingMode::Standard).unwrap()
	};
	for d in 1..=max_depth {
		//let mut pv = vec![];
		//coupled_perft(&shakmaty_pos, &board, d, &mut pv);
		println!("Searched nodes: {}", perft_driver(&mut board, d));
		println!("Shakmaty nodes: {}\n", shakmaty_perft(&shakmaty_pos, d));
	}
}

fn main() {
	/*let mut board =
	Board::from_fen().unwrap();*/
	/*let mut movebuf = MoveList::default();
	board.gen_pseudo_legal_moves::<{ move_gen_stages::ALL }, { WHITE }>(&mut movebuf);
	for m in movebuf.iter() {
		println!("{}", m);
	}
	println!("{}", movebuf.len());*/
	//	println!("Perft: {}", perft(&board, 2));
	//perft(&board, 3);
	/*test_against_shakmaty(
		"r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
		6,
	);*/

	let mut uci = UCIInstance::new();
	uci.run();
}
