mod move_ordering;

use std::time::Instant;

use crate::{
	board::{
		Board,
		movegen::{Move, MoveList},
		utils::{BLACK, Color, WHITE},
	},
	search::move_ordering::{MoveListStages, OrderedMoveList, StagedMoveList},
	tt::{ScoreType, TT},
};

const TIMER_CHECK_INTERVAL: usize = 4096;

mod node_types {
	pub const CUT: u8 = 0;
	pub const ALL: u8 = 1;
	pub const PV: u8 = 2;
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct StackElem {
	pub in_check: bool,
}

pub struct SearchCtx<'a> {
	tt: &'a mut TT,
	board: Board,
	search_start: Instant,
	allocated_time_millis: u64,
	max_depth: u16,
	check_counter: usize,
	stop_search: bool,
	nodes: u64,
	history: [[[i16; 64]; 64]; 2],
	killers: [[Move; 2]; 256],
	stack: [StackElem; 256],
}

impl<'a> SearchCtx<'a> {
	pub fn new(tt: &'a mut TT, board: Board, time: i32, inc: i32, max_depth: u16) -> Self {
		Self {
			tt,
			board,
			search_start: Instant::now(),
			allocated_time_millis: (time / 20 + inc / 2) as u64,
			max_depth,
			check_counter: 0,
			stop_search: false,
			nodes: 0,
			history: [[[0; 64]; 64]; 2],
			killers: [[Move::default(); 2]; 256],
			stack: core::array::from_fn(|_| Default::default()),
		}
	}

	fn quiescence_search(&mut self, ply_from_root: u16, mut alpha: i32, beta: i32) -> Option<i32> {
		self.check_counter += 1;

		if self.board.detect_repetition() || self.board.fifty_moves_rule() {
			// Draw score
			return Some(0);
		}

		if !self.stop_search
			&& self.check_counter.is_multiple_of(TIMER_CHECK_INTERVAL)
			&& self.search_start.elapsed().as_millis() as u64 >= self.allocated_time_millis
		{
			self.stop_search = true;
		}

		if self.stop_search {
			return None;
		}

		self.nodes += 1;

		let is_in_check = match self.board.get_turn() {
			Color::White => self.board.is_in_check::<WHITE>(),
			Color::Black => self.board.is_in_check::<BLACK>(),
		};

		let mut best_value = -999999;
		let mut move_list = MoveList::default();
		if is_in_check {
			match self.board.get_turn() {
				Color::White => self
					.board
					.gen_all_pseudo_legal_moves::<WHITE>(&mut move_list),
				Color::Black => self
					.board
					.gen_all_pseudo_legal_moves::<BLACK>(&mut move_list),
			}
		} else {
			// This avoids checking if in check twice
			match self.board.get_turn() {
				Color::White => self
					.board
					.gen_pseudo_legal_captures::<WHITE>(&mut move_list),
				Color::Black => self
					.board
					.gen_pseudo_legal_captures::<BLACK>(&mut move_list),
			}

			// Standing pat
			best_value = self.board.eval_objective()
				* match self.board.get_turn() {
					Color::White => 1,
					Color::Black => -1,
				};

			if best_value >= beta {
				return Some(best_value);
			}

			if best_value > alpha {
				alpha = best_value;
			}
		}

		// We don't use TT in the quiescence search, so staged movegen is useless
		let mut ordered_move_list =
			OrderedMoveList::from_move_list(&self.board, &mut move_list, None);

		let mut num_legal_moves = 0;
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

			let score = -self.quiescence_search(ply_from_root + 1, -beta, -alpha)?;

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

		if num_legal_moves == 0 && is_in_check {
			let mating_value = -99999 + ply_from_root as i32;
			return Some(mating_value);
		}

		Some(best_value)
	}

	fn negamax<const NODE_TYPE: u8>(
		&mut self,
		depth: u16,
		ply_from_root: u16,
		mut alpha: i32,
		mut beta: i32,
	) -> Option<i32> {
		self.check_counter += 1;

		if self.board.detect_repetition() || self.board.fifty_moves_rule() {
			// Draw score
			return Some(0);
		}

		if !self.stop_search
			&& self.check_counter.is_multiple_of(TIMER_CHECK_INTERVAL)
			&& self.search_start.elapsed().as_millis() as u64 >= self.allocated_time_millis
		{
			self.stop_search = true;
		}

		if self.stop_search {
			return None;
		}

		self.nodes += 1;

		let entry = self.tt.probe(&self.board);
		if let Some(entry) = entry
			&& entry.depth >= depth as i16
			&& (depth == 0 || NODE_TYPE != node_types::PV)
			&& (NODE_TYPE == node_types::CUT || entry.score.inner() <= alpha)
		{
			match entry.score {
				ScoreType::Exact(v) => {
					return if Self::is_mating_value(v) {
						Some((v.abs() - ply_from_root as i32) * v.signum())
					} else {
						Some(v)
					};
				}
				ScoreType::Lower(v) if v >= beta => {
					return if Self::is_mating_value(v) {
						Some((v.abs() - ply_from_root as i32) * v.signum())
					} else {
						Some(v)
					};
				}
				ScoreType::Upper(v) if v <= alpha => {
					return if Self::is_mating_value(v) {
						Some((v.abs() - ply_from_root as i32) * v.signum())
					} else {
						Some(v)
					};
				}
				_ => {}
			};
		}

		if depth == 0 {
			return self.quiescence_search(ply_from_root, alpha, beta);
		}

		let hash_move = entry.map(|e| e.best_move);

		let in_check = match self.board.get_turn() {
			Color::White => self.board.is_in_check::<WHITE>(),
			Color::Black => self.board.is_in_check::<BLACK>(),
		};
		self.stack[ply_from_root as usize] = StackElem { in_check };

		// MDP
		alpha = alpha.max(-99999 + ply_from_root as i32);
		beta = beta.min(99999 - ply_from_root as i32);
		if alpha >= beta {
			return Some(alpha);
		}

		let alpha_orig = alpha;
		let beta_orig = beta;

		// NMP
		{
			let r = 3 + depth / 3;
			if !in_check
				&& depth >= 2
				&& match self.board.get_turn() {
					Color::White => self.board.has_non_pawn_material::<WHITE>(),
					Color::Black => self.board.has_non_pawn_material::<BLACK>(),
				} && NODE_TYPE == node_types::CUT
			{
				let undo_info = self.board.do_null_move();
				let score = -self.negamax::<{ node_types::CUT }>(
					depth.saturating_sub(r),
					ply_from_root + 1,
					-beta,
					-beta + 1,
				)?;
				self.board.undo_null_move(undo_info);

				if score >= beta {
					return Some(score);
				}
			}
		}

		let mut ordered_move_list = match self.board.get_turn() {
			Color::White => StagedMoveList::new::<WHITE>(hash_move, &self.board, false),
			Color::Black => StagedMoveList::new::<BLACK>(hash_move, &self.board, false),
		};

		let mut searched_quiets = MoveList::default();

		let mut best_value = -999999;
		let mut num_legal_moves: usize = 0;
		let mut best_move = Move::default();
		while let Some(m) = match self.board.get_turn() {
			Color::White => ordered_move_list.pick_move::<WHITE>(
				&self.board,
				Some(&self.history),
				Some(&self.killers[ply_from_root as usize]),
			),
			Color::Black => ordered_move_list.pick_move::<BLACK>(
				&self.board,
				Some(&self.history),
				Some(&self.killers[ply_from_root as usize]),
			),
		}
		.copied()
		{
			//for m in move_list.iter() {
			let undo_info = match self.board.get_turn() {
				Color::White => self.board.do_move::<WHITE>(&m),
				Color::Black => self.board.do_move::<BLACK>(&m),
			}
			.unwrap();

			if match self.board.get_turn() {
				Color::White => self.board.is_in_check::<BLACK>(),
				Color::Black => self.board.is_in_check::<WHITE>(),
			} {
				match self.board.get_turn() {
					Color::Black => self.board.undo_move::<WHITE>(undo_info, &m),
					Color::White => self.board.undo_move::<BLACK>(undo_info, &m),
				};
				continue;
			}

			let mut r: i16 = 1;
			// Late move reductions (LMR)
			if (depth >= 3) && (num_legal_moves > 0) {
				r += (1 + (depth.ilog2() * num_legal_moves.ilog2() * 625u32) / 4096u32) as i16;
			}

			// ToDo: Implementation leaves board messed up after quitting search when timing out
			let mut score: i32;
			if num_legal_moves == 0 {
				match NODE_TYPE {
					node_types::CUT => {
						score = -self.negamax::<{ node_types::ALL }>(
							depth.saturating_sub_signed(r),
							ply_from_root + 1,
							-beta,
							-alpha,
						)?
					}
					node_types::ALL => {
						score = -self.negamax::<{ node_types::CUT }>(
							depth.saturating_sub_signed(r),
							ply_from_root + 1,
							-beta,
							-alpha,
						)?
					}
					_ => {
						score = -self.negamax::<NODE_TYPE>(
							depth.saturating_sub_signed(r),
							ply_from_root + 1,
							-beta,
							-alpha,
						)?
					}
				}
			} else {
				score = -self.negamax::<{ node_types::CUT }>(
					depth.saturating_sub_signed(r),
					ply_from_root + 1,
					-alpha - 1,
					-alpha,
				)?;
				if score > alpha && NODE_TYPE == node_types::PV {
					score = -self.negamax::<{ node_types::PV }>(
						depth - 1,
						ply_from_root + 1,
						-beta,
						-alpha,
					)?;
				}
			}

			num_legal_moves += 1;

			match self.board.get_turn() {
				Color::Black => self.board.undo_move::<WHITE>(undo_info, &m),
				Color::White => self.board.undo_move::<BLACK>(undo_info, &m),
			}
			.unwrap();

			// fail-soft
			if score > best_value {
				best_value = score;
				best_move = m;
			}

			if score >= alpha {
				alpha = score;
			}

			if alpha >= beta {
				if m.is_quiet() && !m.is_promotion() {
					self.killers[ply_from_root as usize][1] =
						self.killers[ply_from_root as usize][0];
					self.killers[ply_from_root as usize][0] = m;

					let bonus = 16 * depth * depth;
					for quiet in searched_quiets.iter() {
						let (from, to, _) = quiet.unpack();
						match self.board.get_turn() {
							Color::White => move_ordering::update_history::<WHITE>(
								&mut self.history,
								from,
								to,
								-(bonus as i16),
							),
							Color::Black => move_ordering::update_history::<BLACK>(
								&mut self.history,
								from,
								to,
								-(bonus as i16),
							),
						}
					}

					let (from, to, _) = m.unpack();
					match self.board.get_turn() {
						Color::White => move_ordering::update_history::<WHITE>(
							&mut self.history,
							from,
							to,
							bonus as i16,
						),
						Color::Black => move_ordering::update_history::<BLACK>(
							&mut self.history,
							from,
							to,
							bonus as i16,
						),
					}
				}
				break; // Beta-cutoff
			}

			if m.is_quiet() {
				searched_quiets.push(m);
			}
		}

		if num_legal_moves == 0 {
			if match self.board.get_turn() {
				Color::White => self.board.is_in_check::<WHITE>(),
				Color::Black => self.board.is_in_check::<BLACK>(),
			} {
				let mating_value = -99999 + ply_from_root as i32;
				return Some(mating_value);
			}

			return Some(0);
		}

		let tt_score = if best_value < alpha_orig {
			ScoreType::Upper(best_value)
		} else if best_value >= beta_orig {
			ScoreType::Lower(best_value)
		} else {
			ScoreType::Exact(best_value)
		};

		self.tt
			.add_entry(&self.board, best_move, depth as i16, tt_score);

		Some(best_value)
	}

	fn is_mating_value(v: i32) -> bool {
		(99999 - v.abs()) <= 256
	}

	fn get_dtm_from_score(v: i32) -> i32 {
		(99999 - v.abs()) / 2
	}

	fn uci_print_score(score: i32, depth: u16, best_move: Move, nodes: u64) {
		if Self::is_mating_value(score) {
			let dtm = Self::get_dtm_from_score(score) * score.signum();
			println!(
				"info depth {} score mate {} pv {} nodes {}",
				depth, dtm, best_move, nodes
			);
			return;
		}

		println!(
			"info depth {} score cp {} pv {} nodes {}",
			depth, score, best_move, nodes
		);
	}

	fn bestmove(&mut self, depth: u16, mut alpha: i32, beta: i32) -> Option<(Move, i32)> {
		let alpha_orig = alpha;
		let beta_orig = beta;

		let entry = self.tt.probe(&self.board);
		if let Some(entry) = entry
			&& entry.depth >= depth as i16
			&& depth == 0
		{
			// No adjustment for mate scores since ply from root == 0
			match entry.score {
				ScoreType::Exact(v) => {
					Self::uci_print_score(v, depth, entry.best_move, self.nodes);
					return Some((entry.best_move, v));
				}
				ScoreType::Lower(v) if v >= beta => {
					Self::uci_print_score(v, depth, entry.best_move, self.nodes);
					return Some((entry.best_move, v));
				}
				ScoreType::Upper(v) if v <= alpha => {
					Self::uci_print_score(v, depth, entry.best_move, self.nodes);
					return Some((entry.best_move, v));
				}
				_ => {}
			};
		}
		let hash_move = entry.map(|e| e.best_move);

		let mut move_list = MoveList::default();

		self.board
			.gen_all_pseudo_legal_moves_non_monomorphizing(&mut move_list);

		let mut ordered_move_list = match self.board.get_turn() {
			Color::White => StagedMoveList::new::<WHITE>(hash_move, &self.board, false),
			Color::Black => StagedMoveList::new::<BLACK>(hash_move, &self.board, false),
		};

		let mut best_value = -999999;
		let mut num_legal_moves = 0;
		let mut best_move = Move::default();
		while let Some(m) = match self.board.get_turn() {
			Color::White => {
				ordered_move_list.pick_move::<WHITE>(&self.board, Some(&self.history), None)
			}
			Color::Black => {
				ordered_move_list.pick_move::<BLACK>(&self.board, Some(&self.history), None)
			}
		} {
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

			let mut score: i32;
			if num_legal_moves == 0 {
				score = -self.negamax::<{ node_types::PV }>(depth - 1, 1, -beta, -alpha)?;
			} else {
				score = -self.negamax::<{ node_types::CUT }>(depth - 1, 1, -alpha - 1, -alpha)?;
				if score > alpha {
					score = -self.negamax::<{ node_types::PV }>(depth - 1, 1, -beta, -alpha)?;
				}
			}

			num_legal_moves += 1;

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
			Self::uci_print_score(best_value, depth, best_move, self.nodes);

			let tt_score = if best_value < alpha_orig {
				ScoreType::Upper(best_value)
			} else if best_value >= beta_orig {
				ScoreType::Lower(best_value)
			} else {
				ScoreType::Exact(best_value)
			};

			self.tt
				.add_entry(&self.board, best_move, depth as i16, tt_score);
		}

		Some((best_move, best_value))
	}

	pub fn search(&mut self) -> (Move, i32) {
		let mut best_info = (Move::default(), 0);
		self.search_start = Instant::now();
		self.nodes = 0;

		let mut alpha = -999999999;
		let mut beta = 999999999;
		let mut d = 1;

		loop {
			if d >= self.max_depth.max(1) {
				break;
			}

			let cur_best_info = self.bestmove(d, alpha, beta);
			if self.stop_search {
				break;
			}
			best_info = cur_best_info.unwrap();
			let score = best_info.1;

			if score <= alpha || score >= beta {
				alpha = -999999999;
				beta = 999999999;
			} else {
				d += 1;
				alpha = best_info.1 - 50;
				beta = best_info.1 + 50;
			}
		}

		println!("bestmove {}", best_info.0);
		best_info
	}
}
