use shakmaty::{Chess, Position, fen::Fen};

use crate::{
	board::{
		Board,
		movegen::{Move, MoveList},
		utils::{BLACK, Color, WHITE},
	},
	tuning::{TUNE_PARAMS, print_tune_info},
	uci::UCIInstance,
};

mod board;
mod search;
mod tt;
mod tuning;
mod uci;

fn perft_driver<const C: u8, const OPP: u8>(brd: &mut Board, d: usize) -> u64 {
	if d == 0 {
		return 1;
	}

	let mut move_list = MoveList::default();

	brd.gen_all_pseudo_legal_moves_no_context(&mut move_list);

	let mut nodes = 0;
	for m in move_list.iter() {
		if !brd.is_legal::<C, OPP>(m) {
			continue;
		}

		let undo_info = brd.do_move::<C, OPP>(m).unwrap();
		nodes += perft_driver::<OPP, C>(brd, d - 1);
		brd.undo_move::<C, OPP>(undo_info, m).unwrap();
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

	brd.gen_all_pseudo_legal_moves_no_context(&mut move_list);
	let mut my_moves = move_list
		.iter()
		.filter_map(|m| {
			if match brd.get_turn() {
				Color::White => !brd.is_legal::<WHITE, BLACK>(m),
				Color::Black => !brd.is_legal::<BLACK, WHITE>(m),
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

	if diffs.0.is_empty() {
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

	if diffs.1.is_empty() {
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
			Color::White => brd_cpy.do_move::<WHITE, BLACK>(&my_moves[idx].2),
			Color::Black => brd_cpy.do_move::<BLACK, WHITE>(&my_moves[idx].2),
		};

		let mut shakmaty_cpy = shakmaty_pos.clone();
		shakmaty_cpy.play_unchecked(correct_move.2);
		coupled_perft(&shakmaty_cpy, &brd_cpy, depth - 1, pv);
		pv.pop();
	}
}

#[allow(unused)]
fn test_against_shakmaty(fen: &str, max_depth: usize) {
	// Get perft numbers between me and shakmaty at each depth
	let mut board = Board::from_fen(fen).expect("I failed to parse the FEN");
	println!("{:#?}", board);
	let shakmaty_pos: Chess = {
		let fen: Fen = fen.parse().unwrap();
		fen.into_position(shakmaty::CastlingMode::Standard).unwrap()
	};
	for d in 1..=max_depth {
		let mut pv = vec![];
		coupled_perft(&shakmaty_pos, &board, d, &mut pv);
		println!(
			"Searched nodes: {}",
			match board.get_turn() {
				Color::White => perft_driver::<WHITE, BLACK>(&mut board, d),
				Color::Black => perft_driver::<BLACK, WHITE>(&mut board, d),
			}
		);
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
	//test_against_shakmaty("8/8/3P4/2K5/5p2/8/8/1Q4k1 b - - 11 29", 5);

	let params = std::env::args().collect::<Vec<String>>();

	let mut expecting_params = false;
	for p in params {
		if p == "tunables" {
			expecting_params = true;
			continue;
		}

		if expecting_params {
			expecting_params = false;
			// This must be the params, parse them
			for (cur_param, value) in p.split(',').enumerate() {
				if value.is_empty() {
					break;
				}

				let value: f64 = value.parse::<f64>().expect("Passed invalid param");
				TUNE_PARAMS[cur_param].set(value);
			}
			break;
		}
	}

	if expecting_params {
		// No params passed, print tunables and return
		print_tune_info();
		return;
	}

	let mut uci = UCIInstance::new();
	uci.run();
}
