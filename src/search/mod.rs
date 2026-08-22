mod move_ordering;

use std::time::Instant;

use crate::{
	board::{
		Board,
		movegen::{Move, MoveList},
		utils::{BLACK, Color, Piece, WHITE},
	},
	search::move_ordering::{MoveListStages, StagedMoveList},
	tt::{ScoreType, TT},
};

const TIMER_CHECK_INTERVAL: usize = 4096;
const IMMEDIATE_MATE_SCORE: i32 = 32766;

mod node_types {
	pub const CUT: u8 = 0;
	pub const ALL: u8 = 1;
	pub const PV: u8 = 2;
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct StackElem {
	pub checkers_mask: u64,
	pub history_draw: bool,
	pub static_eval: i16,
	pub excluded: Move,
}

pub struct SupraContextualInfo {
	tt: TT,
	history: [[[i16; 64]; 64]; 2],
	capture_history: [[[i16; 64]; 6]; 6],
}

impl SupraContextualInfo {
	pub fn new(tt: TT) -> Self {
		Self {
			tt,
			history: [[[0; 64]; 64]; 2],
			capture_history: [[[0; 64]; 6]; 6],
		}
	}
}

pub struct SearchCtx<'a> {
	board: Board,
	search_start: Instant,
	allocated_time_millis: u64,
	max_depth: u8,
	seldepth: u8,
	check_counter: usize,
	stop_search: bool,
	nodes: u64,
	stack: [StackElem; 256],
	pv_array: [Move; 257 * 256 / 2],
	info: &'a mut SupraContextualInfo,
	killers: [[Move; 2]; 256],
}

impl<'a> SearchCtx<'a> {
	pub fn new(
		info: &'a mut SupraContextualInfo,
		board: Board,
		time: i32,
		inc: i32,
		max_depth: u8,
	) -> Self {
		Self {
			board,
			search_start: Instant::now(),
			allocated_time_millis: (time / 20 + inc / 2) as u64,
			max_depth,
			seldepth: 0,
			check_counter: 0,
			stop_search: false,
			nodes: 0,
			stack: core::array::from_fn(|_| Default::default()),
			pv_array: [Move::default(); _],
			info,
			killers: [[Move::default(); 2]; _],
		}
	}

	fn tt_score_to_search_score(score: i16, ply_from_root: u8) -> i16 {
		if Self::is_mating_value(score as i32) {
			(score.abs() - ply_from_root as i16) * score.signum()
		} else {
			score
		}
	}

	fn search_score_to_tt_score(score: i16, ply_from_root: u8) -> i16 {
		if Self::is_mating_value(score as i32) {
			(score.abs() + ply_from_root as i16) * score.signum()
		} else {
			score
		}
	}

	fn quiescence_search<const C: u8, const OPP: u8>(
		&mut self,
		ply_from_root: u8,
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

		// MDP
		alpha = alpha.max(-IMMEDIATE_MATE_SCORE + ply_from_root as i32);
		beta = beta.min(IMMEDIATE_MATE_SCORE - ply_from_root as i32 - 1);
		if alpha >= beta {
			return Some(alpha);
		}

		let checkers_mask = self.board.get_checkers_mask::<C, OPP>();
		let is_in_check = checkers_mask != 0;

		let mut best_value = -IMMEDIATE_MATE_SCORE - 1;
		let mut only_captures = true;
		if is_in_check {
			only_captures = false;
		} else {
			// Standing pat
			best_value = self.board.eval_objective::<C>() as i32
				* match C {
					WHITE => 1,
					BLACK => -1,
					_ => 0,
				};

			if best_value >= beta {
				return Some(best_value);
			}

			if best_value > alpha {
				alpha = best_value;
			}
		}

		let mut ordered_move_list =
			StagedMoveList::new::<C, OPP>(None, &self.board, only_captures, checkers_mask);

		let mut num_legal_moves = 0;
		while let Some(m) = ordered_move_list.pick_move::<C, OPP>(
			&self.board,
			Some(&self.info.history),
			&self.info.capture_history,
			Some(&self.killers[ply_from_root as usize]),
		) {
			if !self.board.see_ge::<C, OPP>(*m, -100) {
				continue;
			}

			if !self.board.is_legal::<C, OPP>(m) {
				continue;
			}
			num_legal_moves += 1;

			//for m in move_list.iter() {
			let undo_info = self.board.do_move::<C, OPP>(m)?;

			let score = -self.quiescence_search::<OPP, C>(ply_from_root + 1, -beta, -alpha)?;

			self.board.undo_move::<C, OPP>(undo_info, m)?;

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
			let mating_value = -IMMEDIATE_MATE_SCORE + ply_from_root as i32;
			return Some(mating_value);
		}

		Some(best_value)
	}

	fn negamax<const NODE_TYPE: u8, const C: u8, const OPP: u8>(
		&mut self,
		mut depth: u8,
		ply_from_root: u8,
		mut alpha: i32,
		mut beta: i32,
		pv_index: usize,
	) -> Option<i32> {
		self.pv_array[pv_index] = Move::default();
		let pv_next_index = pv_index + 256 - ply_from_root as usize;

		self.seldepth = self.seldepth.max(ply_from_root);
		self.check_counter += 1;

		self.stack[ply_from_root as usize].history_draw = false;
		if self.board.detect_repetition() || self.board.fifty_moves_rule() {
			// Draw score
			self.stack[ply_from_root as usize].history_draw = true;
			return Some(0);
		}

		if self.board.draw_by_insufficient_material() {
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

		let entry = self.info.tt.probe::<C, OPP>(&self.board).cloned();
		if let Some(ref entry) = entry
			&& self.stack[ply_from_root as usize].excluded == Move::default()
			&& entry.depth >= depth
			&& (depth == 0 || NODE_TYPE != node_types::PV)
			&& (NODE_TYPE == node_types::CUT || entry.score as i32 <= alpha)
		{
			let score = Self::tt_score_to_search_score(entry.score, ply_from_root);
			match entry.get_score_type() {
				ScoreType::Exact => return Some(score as i32),
				ScoreType::Lower if score as i32 >= beta => return Some(score as i32),
				ScoreType::Upper if score as i32 <= alpha => return Some(score as i32),
				_ => {}
			};
		}

		let is_in_check = self.stack[ply_from_root as usize].checkers_mask != 0;

		if depth == 0 {
			return self.quiescence_search::<C, OPP>(ply_from_root, alpha, beta);
		}

		let hash_move = entry.as_ref().map(|e| e.best_move);

		let static_eval = if !is_in_check {
			self.board.eval_objective::<C>() as i32
				* match C {
					WHITE => 1,
					BLACK => -1,
					_ => 0,
				}
		} else {
			// Do not evaluate in check
			-IMMEDIATE_MATE_SCORE - 1
		};

		self.stack[ply_from_root as usize].static_eval = static_eval as i16;

		let improving = if is_in_check {
			false
		} else if (ply_from_root >= 2)
			&& (self.stack[ply_from_root as usize - 2].checkers_mask == 0)
		{
			static_eval > self.stack[ply_from_root as usize - 2].static_eval as i32
		} else if (ply_from_root >= 4)
			&& (self.stack[ply_from_root as usize - 4].checkers_mask == 0)
		{
			static_eval > self.stack[ply_from_root as usize - 4].static_eval as i32
		} else {
			true // ToDo: try returning true here someday
		};

		// MDP
		alpha = alpha.max(-IMMEDIATE_MATE_SCORE + ply_from_root as i32);
		beta = beta.min(IMMEDIATE_MATE_SCORE - ply_from_root as i32 - 1);
		if alpha >= beta {
			return Some(alpha);
		}

		let alpha_orig = alpha;
		let beta_orig = beta;

		// Reverse futility pruning
		if !is_in_check
			&& self.stack[ply_from_root as usize].excluded == Move::default()
			&& NODE_TYPE != node_types::PV
			&& !(Self::is_mating_value(beta) && beta < 0)
			&& static_eval >= beta + 150 * depth.saturating_sub(improving as u8) as i32
		{
			return Some(static_eval);
		}

		// NMP
		{
			if !is_in_check
				&& depth >= 2
				&& self.stack[ply_from_root as usize].excluded == Move::default()
				&& self.board.has_non_pawn_material::<C>()
				&& NODE_TYPE == node_types::CUT
				&& static_eval >= beta
			{
				let r = 3 + depth as i32 / 3 + improving as i32 + ((static_eval - beta) / 128);
				self.stack[ply_from_root as usize + 1].checkers_mask = 0;
				let undo_info = self.board.do_null_move();
				let score = -self.negamax::<{ node_types::CUT }, { OPP }, { C }>(
					depth.saturating_sub(r.min(255) as u8),
					ply_from_root + 1,
					-beta,
					-beta + 1,
					pv_next_index,
				)?;
				self.board.undo_null_move(undo_info);

				if score >= beta {
					return Some(score);
				}
			}
		}

		// Futility pruning
		let mut futile = false;
		if !is_in_check
			&& self.stack[ply_from_root as usize].excluded == Move::default()
			&& depth <= 4
			&& !Self::is_mating_value(alpha)
			&& !Self::is_mating_value(beta)
			&& 200 * depth as i32 + static_eval < alpha
		{
			futile = true;
		}

		// Internal iterative reductions
		if NODE_TYPE != node_types::ALL && depth > 4 && entry.is_none() {
			depth -= 1;
		}

		let mut ordered_move_list = StagedMoveList::new::<C, OPP>(
			hash_move,
			&self.board,
			false,
			self.stack[ply_from_root as usize].checkers_mask,
		);

		let mut searched_quiets = MoveList::default();
		let mut searched_captures = MoveList::default();

		let mut best_value = -IMMEDIATE_MATE_SCORE - 1;
		let mut num_legal_moves: usize = 0;
		let mut best_move = Move::default();
		let lmp_threshold = 3 + (depth as usize * depth as usize) / (2 - improving as usize);
		while let Some(m) = ordered_move_list
			.pick_move::<C, OPP>(
				&self.board,
				Some(&self.info.history),
				&self.info.capture_history,
				Some(&self.killers[ply_from_root as usize]),
			)
			.copied()
		{
			let mut r: u32 = 0;
			let mut extension = 0;
			if m == self.stack[ply_from_root as usize].excluded {
				continue;
			}

			if !self.board.is_legal::<C, OPP>(&m) {
				continue;
			}

			if depth < 5
				&& !self
					.board
					.see_ge::<C, OPP>(m, -50 * depth as i16 * depth as i16)
			{
				num_legal_moves += 1;
				continue;
			}

			if self.stack[ply_from_root as usize].excluded == Move::default()
				&& let Some(entry) = &entry
				&& ordered_move_list.stage() == MoveListStages::HashMove
				&& depth >= 8
				&& entry.depth >= depth.saturating_sub(3)
				&& !Self::is_mating_value(entry.score as i32)
				&& entry.get_score_type() != ScoreType::Upper
			{
				self.stack[ply_from_root as usize].excluded = m;
				let singular_beta = entry.score as i32 - depth as i32;
				let singular_score = self.negamax::<NODE_TYPE, C, OPP>(
					(depth - 1) / 2,
					ply_from_root,
					singular_beta - 1,
					singular_beta,
					pv_next_index,
				)?;
				self.stack[ply_from_root as usize].excluded = Move::default();

				if singular_score < singular_beta {
					extension += 1;
				} else if entry.score as i32 >= beta {
					r += 12288;
				}
			}

			//for m in move_list.iter() {
			let undo_info = self.board.do_move::<C, OPP>(&m).unwrap();

			let checkers_mask = self.board.get_checkers_mask::<OPP, C>();
			self.stack[ply_from_root as usize + 1].checkers_mask = checkers_mask;

			if m.is_quiet() && futile && (checkers_mask == 0) {
				self.board.undo_move::<C, OPP>(undo_info, &m).unwrap();
				num_legal_moves += 1;
				continue;
			}

			// Check extension
			if checkers_mask != 0 {
				extension += 1;
			}

			// Late move reductions (LMR)
			if (depth >= 3) && (num_legal_moves > 0) {
				r += 4096
					+ (depth.ilog2()
						* num_legal_moves.ilog2()
						* (725u32
							- 200 * improving as u32
							- 200
								* (m == self.killers[ply_from_root as usize][0]
									|| m == self.killers[ply_from_root as usize][1])
									as u32) + 200 * (NODE_TYPE == node_types::CUT) as u32);
			}

			r = r.saturating_sub(match NODE_TYPE {
				node_types::PV => 4096,
				_ => 0,
			});

			let new_depth = depth
				.saturating_sub((r / 4096) as u8)
				.saturating_add_signed(extension)
				.saturating_sub(1);

			// ToDo: Implementation leaves board messed up after quitting search when timing out
			let mut score: i32;
			if num_legal_moves == 0 {
				match NODE_TYPE {
					node_types::CUT => {
						score = -self.negamax::<{ node_types::ALL }, OPP, C>(
							new_depth,
							ply_from_root + 1,
							-beta,
							-alpha,
							pv_next_index,
						)?
					}
					node_types::ALL => {
						score = -self.negamax::<{ node_types::CUT }, OPP, C>(
							new_depth,
							ply_from_root + 1,
							-beta,
							-alpha,
							pv_next_index,
						)?
					}
					_ => {
						score = -self.negamax::<NODE_TYPE, OPP, C>(
							new_depth,
							ply_from_root + 1,
							-beta,
							-alpha,
							pv_next_index,
						)?
					}
				}
			} else {
				score = -self.negamax::<{ node_types::CUT }, OPP, C>(
					new_depth,
					ply_from_root + 1,
					-alpha - 1,
					-alpha,
					pv_next_index,
				)?;

				if score > alpha && r != 0 {
					let new_depth = depth - 1;
					match NODE_TYPE {
						node_types::CUT => {
							score = -self.negamax::<{ node_types::ALL }, OPP, C>(
								new_depth,
								ply_from_root + 1,
								-alpha - 1,
								-alpha,
								pv_next_index,
							)?
						}
						node_types::ALL => {
							score = -self.negamax::<{ node_types::CUT }, OPP, C>(
								new_depth,
								ply_from_root + 1,
								-alpha - 1,
								-alpha,
								pv_next_index,
							)?
						}
						_ => {
							score = -self.negamax::<NODE_TYPE, OPP, C>(
								new_depth,
								ply_from_root + 1,
								-alpha - 1,
								-alpha,
								pv_next_index,
							)?
						}
					}
				}

				if score > alpha && NODE_TYPE == node_types::PV {
					score = -self.negamax::<{ node_types::PV }, OPP, C>(
						depth - 1,
						ply_from_root + 1,
						-beta,
						-alpha,
						pv_next_index,
					)?;
				}
			}

			num_legal_moves += 1;

			if depth >= 3 && num_legal_moves >= lmp_threshold {
				futile = true;
			}

			self.board.undo_move::<C, OPP>(undo_info, &m).unwrap();

			// fail-soft
			if score > best_value {
				best_value = score;
				best_move = m;
				self.stack[ply_from_root as usize].history_draw =
					score == 0 && self.stack[ply_from_root as usize + 1].history_draw;
				self.pv_array[pv_index] = m;
				for i in 0..(256 - ply_from_root as usize - 1) {
					let src = self.pv_array[pv_next_index + i];
					self.pv_array[pv_index + i + 1] = src;
					if src == Move::default() {
						break;
					}
				}
			}

			if score >= alpha {
				alpha = score;
			}

			if alpha >= beta {
				if m.is_quiet() && !m.is_promotion() {
					let bonus = depth as i16 * depth as i16;
					self.killers[ply_from_root as usize][1] =
						self.killers[ply_from_root as usize][0];
					self.killers[ply_from_root as usize][0] = m;

					for quiet in searched_quiets.iter() {
						let (from, to, _) = quiet.unpack();
						move_ordering::update_history::<C>(&mut self.info.history, from, to, -bonus)
					}

					let (from, to, _) = m.unpack();
					move_ordering::update_history::<C>(&mut self.info.history, from, to, bonus)
				} else if !m.is_quiet() && !m.is_promotion() {
					let bonus = (168 * depth as i16 - 100).min(1718);
					let (from, to, _) = m.unpack();
					move_ordering::update_capture_history(
						&mut self.info.capture_history,
						self.board.get_piece_at_square::<C>(from)?,
						self.board
							.get_piece_at_square::<OPP>(to)
							.unwrap_or(Piece::Pawn),
						to,
						bonus,
					);
				}

				let malus = (768 * depth as i16 - 257).min(2357);
				for capture in searched_captures.iter() {
					let (from, to, _) = capture.unpack();
					move_ordering::update_capture_history(
						&mut self.info.capture_history,
						self.board.get_piece_at_square::<C>(from)?,
						self.board
							.get_piece_at_square::<OPP>(to)
							.unwrap_or(Piece::Pawn),
						to,
						-malus,
					);
				}

				break; // Beta-cutoff
			}

			if m.is_quiet() {
				searched_quiets.push(m);
			} else {
				searched_captures.push(m);
			}
		}

		if num_legal_moves == 0 {
			if is_in_check {
				let mating_value = -IMMEDIATE_MATE_SCORE + ply_from_root as i32;
				return Some(mating_value);
			}

			return Some(0);
		}

		let tt_score_type = if best_value < alpha_orig {
			ScoreType::Upper
		} else if best_value >= beta_orig {
			ScoreType::Lower
		} else {
			ScoreType::Exact
		};

		if !self.stack[ply_from_root as usize].history_draw {
			self.info.tt.add_entry(
				&self.board,
				best_move,
				depth,
				Self::search_score_to_tt_score(
					best_value.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
					ply_from_root,
				),
				tt_score_type,
			);
		}

		Some(best_value)
	}

	fn is_mating_value(v: i32) -> bool {
		(IMMEDIATE_MATE_SCORE - v.abs()) <= 1000
	}

	fn get_dtm_from_score(v: i32) -> i32 {
		(IMMEDIATE_MATE_SCORE - v.abs() + 1) / 2
	}

	fn uci_print_score(
		score: i32,
		depth: u8,
		seldepth: u8,
		nodes: u64,
		search_start: Instant,
		pv: &[Move],
	) {
		let search_time = search_start.elapsed().as_millis().max(1);
		let nps = (nodes as u128 * 1000) / search_time;
		let mut pv_string = String::new();
		for m in pv {
			if *m == Move::default() {
				break;
			}

			pv_string = format!("{}{} ", pv_string, m);
		}

		if Self::is_mating_value(score) {
			let dtm = Self::get_dtm_from_score(score) * score.signum();
			println!(
				"info depth {} seldepth {} score mate {} pv {}nodes {} nps {} time {}",
				depth, seldepth, dtm, pv_string, nodes, nps, search_time
			);
			return;
		}

		println!(
			"info depth {} seldepth {} score cp {} pv {}nodes {} nps {} time {}",
			depth, seldepth, score, pv_string, nodes, nps, search_time
		);
	}

	fn bestmove<const C: u8, const OPP: u8>(
		&mut self,
		depth: u8,
		mut alpha: i32,
		mut beta: i32,
	) -> Option<(Move, i32)> {
		self.pv_array[0] = Move::default();
		let pv_next_index = 256usize;

		alpha = alpha.max(-IMMEDIATE_MATE_SCORE);
		beta = beta.min(IMMEDIATE_MATE_SCORE);
		let alpha_orig = alpha;
		let beta_orig = beta;

		let entry = self.info.tt.probe::<C, OPP>(&self.board);
		if let Some(entry) = entry
			&& entry.depth >= depth
			&& depth == 0
			&& entry.score as i32 <= alpha
		{
			// No adjustment for mate scores since ply from root == 0
			match entry.get_score_type() {
				ScoreType::Exact => {
					return Some((entry.best_move, entry.score as i32));
				}
				ScoreType::Lower if entry.score as i32 >= beta => {
					return Some((entry.best_move, entry.score as i32));
				}
				ScoreType::Upper if entry.score as i32 <= alpha => {
					return Some((entry.best_move, entry.score as i32));
				}
				_ => {}
			};
		}
		let hash_move = entry.map(|e| e.best_move);
		self.stack[0].checkers_mask = self.board.get_checkers_mask::<C, OPP>();
		let is_in_check = self.stack[0].checkers_mask != 0;

		let static_eval = if !is_in_check {
			self.board.eval_objective::<C>() as i32
				* match C {
					WHITE => 1,
					BLACK => -1,
					_ => 0,
				}
		} else {
			-IMMEDIATE_MATE_SCORE - 1
		};

		self.stack[0].static_eval = static_eval as i16;

		let mut ordered_move_list = StagedMoveList::new::<C, OPP>(
			hash_move,
			&self.board,
			false,
			self.stack[0].checkers_mask,
		);

		let mut best_value = -IMMEDIATE_MATE_SCORE - 1;
		let mut num_legal_moves = 0;
		let mut best_move = Move::default();
		while let Some(m) = ordered_move_list.pick_move::<C, OPP>(
			&self.board,
			Some(&self.info.history),
			&self.info.capture_history,
			None,
		) {
			if !self.board.is_legal::<C, OPP>(m) {
				continue;
			}
			//for m in move_list.iter() {

			// Avoids playing illegal moves when running out of time
			if best_move == Move::default() {
				best_move = *m;
			}

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

			let undo_info = self.board.do_move::<C, OPP>(m)?;

			let checkers_mask = self.board.get_checkers_mask::<OPP, C>();
			self.stack[1].checkers_mask = checkers_mask;

			let mut score: i32;
			if num_legal_moves == 0 {
				score = -self.negamax::<{ node_types::PV }, OPP, C>(
					depth - 1,
					1,
					-beta,
					-alpha,
					pv_next_index,
				)?;
			} else {
				score = -self.negamax::<{ node_types::CUT }, OPP, C>(
					depth - 1,
					1,
					-alpha - 1,
					-alpha,
					pv_next_index,
				)?;
				if score > alpha {
					score = -self.negamax::<{ node_types::PV }, OPP, C>(
						depth - 1,
						1,
						-beta,
						-alpha,
						pv_next_index,
					)?;
				}
			}

			num_legal_moves += 1;

			self.board.undo_move::<C, OPP>(undo_info, m)?;

			if score > best_value {
				best_move = *m;
				best_value = score;
				self.pv_array[0] = *m;
				for i in 0..(256 - 1) {
					let src = self.pv_array[pv_next_index + i];
					self.pv_array[i + 1] = src;
					if src == Move::default() {
						break;
					}
				}
			}

			if score >= alpha {
				alpha = score;
			}

			if alpha >= beta {
				break;
			}
		}

		if !self.stop_search {
			let tt_score_type = if best_value < alpha_orig {
				ScoreType::Upper
			} else if best_value >= beta_orig {
				ScoreType::Lower
			} else {
				ScoreType::Exact
			};

			self.info.tt.add_entry(
				&self.board,
				best_move,
				depth,
				// This clamp is here just in case
				best_value.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
				tt_score_type,
			);
		}

		Some((best_move, best_value))
	}

	fn aspirated_search(&mut self, depth: u8, score: i32) -> Option<(Move, i32)> {
		let delta = 50;
		let mut delta = (delta / 4) * 4;
		let mut alpha = score - delta;
		let mut beta = score + delta;

		loop {
			let search_info = match self.board.get_turn() {
				Color::White => self.bestmove::<WHITE, BLACK>(depth, alpha, beta)?,
				Color::Black => self.bestmove::<BLACK, WHITE>(depth, alpha, beta)?,
			};
			if search_info.1 <= alpha {
				beta = (alpha + beta) / 2;
				alpha -= delta;
				delta *= 2;
				delta = (delta / 4) * 4;
				continue;
			}

			if search_info.1 >= beta {
				beta += delta;
				delta *= 2;
				delta = (delta / 4) * 4;
				continue;
			}

			// No bound failures, return score and report to user
			Self::uci_print_score(
				search_info.1,
				depth,
				self.seldepth,
				self.nodes,
				self.search_start,
				&self.pv_array[0..256],
			);
			return Some(search_info);
		}
	}

	pub fn search(&mut self) -> (Move, i32) {
		self.search_start = Instant::now();
		self.nodes = 0;
		self.seldepth = 0;

		// Age history values between iterations to avoid staleness
		for i in 0..64 {
			for j in 0..64 {
				self.info.history[WHITE as usize][i][j] =
					(self.info.history[WHITE as usize][i][j] * 3) / 4;
				self.info.history[BLACK as usize][i][j] =
					(self.info.history[BLACK as usize][i][j] * 3) / 4;
			}
		}

		for i in 0..6 {
			for j in 0..6 {
				for k in 0..64 {
					self.info.capture_history[i][j][k] =
						(self.info.capture_history[i][j][k] * 3) / 4;
				}
			}
		}

		let mut best_info = match self.board.get_turn() {
			Color::White => self.bestmove::<WHITE, BLACK>(
				1,
				-IMMEDIATE_MATE_SCORE - 1,
				IMMEDIATE_MATE_SCORE + 1,
			),
			Color::Black => self.bestmove::<BLACK, WHITE>(
				1,
				-IMMEDIATE_MATE_SCORE - 1,
				IMMEDIATE_MATE_SCORE + 1,
			),
		}
		.expect("Failed depth 1 search in time");
		Self::uci_print_score(
			best_info.1,
			1,
			self.seldepth,
			self.nodes,
			self.search_start,
			&self.pv_array[0..256],
		);

		for depth in 2..=self.max_depth {
			self.seldepth = 0;
			let info = self.aspirated_search(depth, best_info.1);
			if self.stop_search {
				break;
			}

			if let Some(info) = info {
				best_info = info;
			} else {
				break;
			}
		}

		println!("bestmove {}", best_info.0);
		best_info
	}
}
