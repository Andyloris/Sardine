use std::{io::stdin, process::exit};

use crate::{
	board::{
		Board,
		movegen::{Move, MoveFlag, MoveList},
		utils::{BLACK, Color, Square, WHITE},
	},
	search::SearchCtx,
	tt::TT,
};

pub struct UCIInstance {
	tt: TT,
	position: Board,
}

impl UCIInstance {
	pub fn new() -> Self {
		Self {
			tt: TT::new(23),
			position: Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
				.unwrap(),
		}
	}

	pub fn run(&mut self) -> ! {
		loop {
			let mut command = String::new();
			stdin().read_line(&mut command).unwrap();
			let params = command.split_whitespace().collect::<Vec<_>>();
			match params.as_slice() {
				["uci"] => Self::uci(),
				["isready"] => Self::isready(),

				["position", tokens @ ..] => self.position(tokens),
				["ucinewgame"] => self.ucinewgame(),
				["go", tokens @ ..] => self.go(tokens),

				["quit"] => std::process::exit(0),

				_ => eprintln!("Unknown command"),
			}
		}
	}

	fn from_lan(&self, lan: String) -> Option<Move> {
		let mut chars = lan.as_str().chars();
		let file1_char: char = chars.nth(0)?;
		let rank1_char: char = chars.nth(0)?;
		let file2_char: char = chars.nth(0)?;
		let rank2_char: char = chars.nth(0)?;

		let Square(from_sq) = Square::from_rank_file_chars_ascii(rank1_char, file1_char);
		let Square(to_sq) = Square::from_rank_file_chars_ascii(rank2_char, file2_char);

		let promotion_flag = chars.nth(0).map(|c| match c {
			'n' => [MoveFlag::KnightPromotion, MoveFlag::KnightPromoCapture],
			'b' => [MoveFlag::BishopPromotion, MoveFlag::BishopPromoCapture],
			'r' => [MoveFlag::RookPromotion, MoveFlag::RookPromoCapture],
			'q' => [MoveFlag::QueenPromotion, MoveFlag::QueenPromoCapture],
			_ => {
				eprintln!("Invalid UCI format move");
				exit(0)
			}
		});

		let mut move_list = MoveList::default();

		match self.position.get_turn() {
			Color::White => self
				.position
				.gen_all_pseudo_legal_moves::<WHITE>(&mut move_list),
			Color::Black => self
				.position
				.gen_all_pseudo_legal_moves::<BLACK>(&mut move_list),
		};

		let mut possibilities: Vec<Move> = vec![];
		for m in move_list.iter() {
			let (from, to, flag) = m.unpack();
			if from == from_sq && to == to_sq {
				if let Some(flags) = promotion_flag {
					if flags.contains(&flag) {
						possibilities.push(*m);
					}
				} else {
					possibilities.push(*m);
				}
			}
		}

		Some(possibilities[0])
	}

	fn make_move(&mut self, chess_move: String) -> Option<()> {
		match self.position.get_turn() {
			Color::White => self.position.do_move::<WHITE>(&self.from_lan(chess_move)?),
			Color::Black => self.position.do_move::<BLACK>(&self.from_lan(chess_move)?),
		};
		Some(())
	}

	fn uci() {
		println!("id name Sardine");
		println!("id author Andyloris");
		println!("uciok");
	}

	fn isready() {
		println!("readyok");
	}

	fn position(&mut self, mut params: &[&str]) {
		while !params.is_empty() {
			match params {
				["startpos", rest @ ..] => {
					self.position =
						Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
							.unwrap();
					params = rest;
				}

				["fen", rest @ ..] => {
					let fen = &rest.join(" ");
					let pos = Board::from_fen(fen);
					match pos {
						Some(pos) => self.position = pos,
						None => eprintln!("Invalid FEN"),
					}
					params = rest;
				}

				["moves", rest @ ..] => {
					for uci_move in rest.iter() {
						self.make_move(uci_move.to_string());
					}
					break;
				}

				_ => {
					params = &params[1..];
					continue;
				}
			}
		}
	}

	fn ucinewgame(&mut self) {
		self.position =
			Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
	}

	fn go(&mut self, params: &[&str]) {
		let mut depth: u16 = 255;
		let mut time: i32 = i32::MAX - 1;
		let mut inc: i32 = 0;
		for chunk in params.chunks(2) {
			if let [name, value] = *chunk {
				let Ok(value): Result<i32, std::num::ParseIntError> = value.parse() else {
					continue;
				};

				match name {
					"depth" if value > 0 => depth = value as u16,
					"wtime" if self.position.get_turn() == Color::White => time = value,
					"winc" if self.position.get_turn() == Color::White => inc = value,
					"btime" if self.position.get_turn() == Color::Black => time = value,
					"binc" if self.position.get_turn() == Color::Black => inc = value,
					_ => continue,
				}
			}
		}

		let mut search_ctx = SearchCtx::new(&mut self.tt, self.position.clone(), time, inc, depth);
		search_ctx.search();
	}
}
