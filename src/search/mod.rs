mod move_ordering;

use std::time::Instant;

use crate::{
	board::{
		Board,
		movegen::{Move, MoveList},
		utils::{BLACK, Color, WHITE},
	},
	eval,
	search::move_ordering::OrderedMoveList,
};

const TIMER_CHECK_INTERVAL: usize = 4096;

pub struct SearchCtx {
	board: Board,
	search_start: Instant,
	allocated_time_millis: u64,
	max_depth: usize,
	check_counter: usize,
	stop_search: bool,
	nodes: u64,
}

impl SearchCtx {
	pub fn new(board: Board, time: i32, inc: i32, max_depth: usize) -> Self {
		Self {
			board,
			search_start: Instant::now(),
			allocated_time_millis: (time / 20 + inc / 2) as u64,
			max_depth,
			check_counter: 0,
			stop_search: false,
			nodes: 0,
		}
	}

	fn negamax(&mut self, depth: usize, mut alpha: i32, beta: i32) -> Option<i32> {
		self.nodes += 1;
		if self.board.detect_threefold_repetition() || self.board.fifty_moves_rule() {
			// Draw score
			return Some(0);
		}

		if depth == 0 {
			return Some(
				eval::eval_board_objective(&self.board)
					* match self.board.get_turn() {
						Color::White => 1,
						Color::Black => -1,
					},
			);
		}

		self.check_counter += 1;

		if !self.stop_search
			&& self.check_counter.is_multiple_of(TIMER_CHECK_INTERVAL)
			&& self.search_start.elapsed().as_millis() as u64 >= self.allocated_time_millis
		{
			self.stop_search = true;
		}

		if self.stop_search {
			return None;
		}

		let mut move_list = MoveList::default();
		match self.board.get_turn() {
			Color::White => self
				.board
				.gen_all_pseudo_legal_moves::<WHITE>(&mut move_list),
			Color::Black => self
				.board
				.gen_all_pseudo_legal_moves::<BLACK>(&mut move_list),
		};

		let mut ordered_move_list = OrderedMoveList::from_move_list(&self.board, &mut move_list);

		let mut best_value = -999999;
		let mut num_legal_moves: usize = 0;
		while let Some(m) = ordered_move_list.pick_move(&self.board) {
			//for m in move_list.iter() {
			let undo_info = match self.board.get_turn() {
				Color::White => self.board.do_move::<WHITE>(m),
				Color::Black => self.board.do_move::<BLACK>(m),
			}
			.unwrap();

			if match self.board.get_turn() {
				Color::White => self.board.is_in_check::<BLACK>(),
				Color::Black => self.board.is_in_check::<WHITE>(),
			} {
				match self.board.get_turn() {
					Color::Black => self.board.undo_move::<WHITE>(undo_info, m),
					Color::White => self.board.undo_move::<BLACK>(undo_info, m),
				};
				continue;
			}

			num_legal_moves += 1;

			let score = -self.negamax(depth - 1, -beta, -alpha)?;

			match self.board.get_turn() {
				Color::Black => self.board.undo_move::<WHITE>(undo_info, m),
				Color::White => self.board.undo_move::<BLACK>(undo_info, m),
			}
			.unwrap();

			// fail-soft
			if score > best_value {
				best_value = score;
			}

			if score >= alpha {
				alpha = score;
			}

			if alpha >= beta {
				break; // Beta-cutoff
			}
		}

		if num_legal_moves == 0 {
			if match self.board.get_turn() {
				Color::White => self.board.is_in_check::<WHITE>(),
				Color::Black => self.board.is_in_check::<BLACK>(),
			} {
				return Some(-99999);
			}

			return Some(0);
		}

		Some(best_value)
	}

	fn bestmove(&mut self, depth: usize, mut alpha: i32, beta: i32) -> Option<(Move, i32)> {
		self.nodes += 1;
		let mut move_list = MoveList::default();

		match self.board.get_turn() {
			Color::White => self
				.board
				.gen_all_pseudo_legal_moves::<WHITE>(&mut move_list),
			Color::Black => self
				.board
				.gen_all_pseudo_legal_moves::<BLACK>(&mut move_list),
		};

		let mut ordered_move_list = OrderedMoveList::from_move_list(&self.board, &mut move_list);

		let mut best_value = -999999;
		let mut best_move = Move::default();
		while let Some(m) = ordered_move_list.pick_move(&self.board) {
			//for m in move_list.iter() {
			self.check_counter += 1;

			if !self.stop_search
				&& self.check_counter.is_multiple_of(TIMER_CHECK_INTERVAL)
				&& self.search_start.elapsed().as_millis() as u64 >= self.allocated_time_millis
			{
				self.stop_search = true;
			}

			if self.stop_search {
				break;
			}

			let undo_info = match self.board.get_turn() {
				Color::White => self.board.do_move::<WHITE>(m),
				Color::Black => self.board.do_move::<BLACK>(m),
			}
			.unwrap();

			if match self.board.get_turn() {
				Color::White => self.board.is_in_check::<BLACK>(),
				Color::Black => self.board.is_in_check::<WHITE>(),
			} {
				match self.board.get_turn() {
					Color::Black => self.board.undo_move::<WHITE>(undo_info, m),
					Color::White => self.board.undo_move::<BLACK>(undo_info, m),
				};
				continue;
			}

			let score = -self.negamax(depth - 1, -beta, -alpha)?;

			match self.board.get_turn() {
				Color::Black => self.board.undo_move::<WHITE>(undo_info, m),
				Color::White => self.board.undo_move::<BLACK>(undo_info, m),
			}
			.unwrap();

			if score > best_value {
				best_move = *m;
				best_value = score;
			}

			if score >= alpha {
				alpha = score;
			}
		}

		if !self.stop_search {
			println!(
				"info depth {} score cp {} pv {} nodes {}",
				depth, best_value, best_move, self.nodes
			);
		}

		Some((best_move, best_value))
	}

	pub fn search(&mut self) -> (Move, i32) {
		let mut best_info = (Move::default(), 0);
		self.search_start = Instant::now();

		for d in 1..=self.max_depth.max(1) {
			let cur_best_info = self.bestmove(d, i32::MIN + 1, i32::MAX);
			if self.stop_search {
				break;
			}
			best_info = cur_best_info.unwrap();
		}

		println!("bestmove {}", best_info.0);

		best_info
	}
}
