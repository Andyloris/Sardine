use std::fmt::Display;

pub const A_FILE: u64 = 0x0101010101010101;
pub const H_FILE: u64 = 0x8080808080808080;

pub const AB_FILE: u64 = 0x0303030303030303;
pub const GH_FILE: u64 = 0xC0C0C0C0C0C0C0C0;

pub const RANK_1: u64 = 0x00000000000000ff;
pub const RANK_2: u64 = 0x000000000000ff00;
pub const RANK_4: u64 = 0x00000000ff000000;
pub const RANK_5: u64 = 0x000000ff00000000;
pub const RANK_7: u64 = 0x00ff000000000000;
pub const RANK_8: u64 = 0xff00000000000000;

pub const BLACK_SQUARES: u64 = 0xAA55AA55AA55AA55;
pub const WHITE_SQUARES: u64 = !BLACK_SQUARES;

pub const QUEEN_CASTLE_MASKS: [u64; 2] = [0xE, 0xE00000000000000];
pub const KING_CASTLE_MASKS: [u64; 2] = [0x60, 0x6000000000000000];

pub mod direction {
	pub const N: i8 = 8;
	pub const NN: i8 = 2 * 8;
	pub const S: i8 = -8;
	pub const SS: i8 = 2 * -8;
	pub const E: i8 = 1;
	pub const EE: i8 = 2;
	pub const W: i8 = -1;
	pub const WW: i8 = -2;
	pub const NW: i8 = 7;
	pub const NE: i8 = 9;
	pub const SE: i8 = -7;
	pub const SW: i8 = -9;

	pub const NWW: i8 = 6;
	pub const NNW: i8 = 15;
	pub const NNE: i8 = 17;
	pub const NEE: i8 = 10;
	pub const SEE: i8 = -6;
	pub const SSE: i8 = -15;
	pub const SSW: i8 = -17;
	pub const SWW: i8 = -10;
}

pub const fn shift_bb<const D: i8>(bb: u64) -> u64 {
	match D {
		direction::N => bb << 8,
		direction::S => bb >> 8,
		direction::NN => bb << 16,
		direction::SS => bb >> 16,
		direction::E => (bb & !H_FILE) << 1,
		direction::EE => (bb & !GH_FILE) << 2,
		direction::W => (bb & !A_FILE) >> 1,
		direction::WW => (bb & !AB_FILE) >> 2,
		direction::NE => (bb & !H_FILE) << 9,
		direction::NW => (bb & !A_FILE) << 7,
		direction::SE => (bb & !H_FILE) >> 7,
		direction::SW => (bb & !A_FILE) >> 9,
		direction::NWW => (bb & !AB_FILE) << 6,
		direction::NNW => (bb & !A_FILE) << 15,
		direction::NNE => (bb & !H_FILE) << 17,
		direction::NEE => (bb & !GH_FILE) << 10,
		direction::SEE => (bb & !GH_FILE) >> 6,
		direction::SSE => (bb & !H_FILE) >> 15,
		direction::SSW => (bb & !A_FILE) >> 17,
		direction::SWW => (bb & !AB_FILE) >> 10,
		_ => 0,
	}
}

pub const NUM_PIECES: usize = 6;
pub const PIECES: [Piece; 6] = [
	Piece::Pawn,
	Piece::Knight,
	Piece::Bishop,
	Piece::Rook,
	Piece::Queen,
	Piece::King,
];

pub const NUM_COLORS: usize = 2;
#[allow(unused)]
pub const COLORS: [Color; 2] = [Color::White, Color::Black];

pub const WHITE: u8 = 0;
pub const BLACK: u8 = 1;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Color {
	#[default]
	White = WHITE,
	Black = BLACK,
}

impl From<u8> for Color {
	fn from(value: u8) -> Self {
		match value {
			0 => Self::White,
			1 => Self::Black,
			_ => panic!("Invalid color index: {}", value),
		}
	}
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Piece {
	#[default]
	Pawn = 0,
	Knight = 1,
	Bishop = 2,
	Rook = 3,
	Queen = 4,
	King = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PieceColorPair(pub Piece, pub Color);

impl From<PieceColorPair> for char {
	fn from(val: PieceColorPair) -> Self {
		match (val.0, val.1) {
			(Piece::Pawn, Color::White) => 'P',
			(Piece::Pawn, Color::Black) => 'p',
			(Piece::Knight, Color::White) => 'N',
			(Piece::Knight, Color::Black) => 'n',
			(Piece::Bishop, Color::White) => 'B',
			(Piece::Bishop, Color::Black) => 'b',
			(Piece::Rook, Color::White) => 'R',
			(Piece::Rook, Color::Black) => 'r',
			(Piece::Queen, Color::White) => 'Q',
			(Piece::Queen, Color::Black) => 'q',
			(Piece::King, Color::White) => 'K',
			(Piece::King, Color::Black) => 'k',
		}
	}
}

impl TryFrom<char> for PieceColorPair {
	type Error = ();
	fn try_from(value: char) -> Result<Self, ()> {
		Ok(match value {
			'P' => PieceColorPair(Piece::Pawn, Color::White),
			'p' => PieceColorPair(Piece::Pawn, Color::Black),
			'N' => PieceColorPair(Piece::Knight, Color::White),
			'n' => PieceColorPair(Piece::Knight, Color::Black),
			'B' => PieceColorPair(Piece::Bishop, Color::White),
			'b' => PieceColorPair(Piece::Bishop, Color::Black),
			'R' => PieceColorPair(Piece::Rook, Color::White),
			'r' => PieceColorPair(Piece::Rook, Color::Black),
			'Q' => PieceColorPair(Piece::Queen, Color::White),
			'q' => PieceColorPair(Piece::Queen, Color::Black),
			'K' => PieceColorPair(Piece::King, Color::White),
			'k' => PieceColorPair(Piece::King, Color::Black),
			_ => return Err(()),
		})
	}
}

impl From<u8> for Piece {
	fn from(value: u8) -> Self {
		match value {
			0 => Self::Pawn,
			1 => Self::Knight,
			2 => Self::Bishop,
			3 => Self::Rook,
			4 => Self::Queen,
			5 => Self::King,
			_ => panic!("Invalid piece index: {}", value),
		}
	}
}

pub const RANKS: [char; 8] = ['1', '2', '3', '4', '5', '6', '7', '8'];
pub const FILES: [char; 8] = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Square(pub u8);

impl Square {
	pub fn from_rank_file(rank: u8, file: u8) -> Self {
		Square(8 * rank + file)
	}

	// May give nonsensical results if given nonsensical characters
	pub fn from_rank_file_chars_ascii(rank: char, file: char) -> Self {
		let rank = rank as u8 - 0x31;
		let file = file as u8 - 0x61;
		Square::from_rank_file(rank, file)
	}

	pub fn to_rank_file(self) -> (u8, u8) {
		(self.0 / 8, self.0 % 8)
	}
}

impl Display for Square {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let (rank, file) = self.to_rank_file();
		write!(f, "{}{}", FILES[file as usize], RANKS[rank as usize])
	}
}

#[allow(unused)]
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bitboard(pub u64);

impl Display for Bitboard {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		writeln!(f, "  A B C D E F G H")?;
		for rank in (0..8).rev() {
			write!(f, "{} ", rank + 1)?;
			for file in 0..8 {
				let Square(sq) = Square::from_rank_file(rank, file);
				write!(f, "{} ", if (self.0 & (1u64 << sq)) == 0 { 0 } else { 1 })?;
			}
			writeln!(f, "{}", rank + 1)?;
		}
		writeln!(f, "  A B C D E F G H")?;

		Ok(())
	}
}

#[allow(unused)]
pub fn pop_lsb(bb: &mut u64) -> u64 {
	let lsb = *bb & bb.wrapping_neg();
	*bb &= *bb - 1;
	lsb
}

pub fn clear_lsb(bb: &mut u64) {
	*bb &= *bb - 1;
}

#[allow(unused)]
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Squares {
	#[default]
	A1 = 0,
	B1,
	C1,
	D1,
	E1,
	F1,
	G1,
	H1,
	A2,
	B2,
	C2,
	D2,
	E2,
	F2,
	G2,
	H2,
	A3,
	B3,
	C3,
	D3,
	E3,
	F3,
	G3,
	H3,
	A4,
	B4,
	C4,
	D4,
	E4,
	F4,
	G4,
	H4,
	A5,
	B5,
	C5,
	D5,
	E5,
	F5,
	G5,
	H5,
	A6,
	B6,
	C6,
	D6,
	E6,
	F6,
	G6,
	H6,
	A7,
	B7,
	C7,
	D7,
	E7,
	F7,
	G7,
	H7,
	A8,
	B8,
	C8,
	D8,
	E8,
	F8,
	G8,
	H8,
}
