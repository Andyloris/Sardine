use crate::board::sliding_attack_table::SLIDING_ATTACKS;

static ROOK_BITS: [u8; 64] = [
    12, 11, 11, 11, 11, 11, 11, 12, 11, 10, 10, 10, 10, 10, 10, 11, 11, 10, 10, 10, 10, 10, 10, 11,
    11, 10, 10, 10, 10, 10, 10, 11, 11, 10, 10, 10, 10, 10, 10, 11, 11, 10, 10, 10, 10, 10, 10, 11,
    11, 10, 10, 10, 10, 10, 10, 11, 12, 11, 11, 11, 11, 11, 11, 12,
];

static BISHOP_BITS: [u8; 64] = [
    6, 5, 5, 5, 5, 5, 5, 6, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 7, 7, 7, 7, 5, 5, 5, 5, 7, 9, 9, 7, 5, 5,
    5, 5, 7, 9, 9, 7, 5, 5, 5, 5, 7, 7, 7, 7, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 5, 5, 5, 5, 5, 5, 6,
];
// Code for generating magics
/*
// Was found to be one of the fastest methods for generating random numbers
const fn fast_hash(state: u64) -> u64 {
    let mut h = state;
    h ^= h >> 23;
    h = h.wrapping_mul(0x2127599bf4325c37);
    h ^= h >> 47;
    h
}

struct RandomFewbits {
    state: u64,
}

impl RandomFewbits {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub const fn get_rand(&mut self) -> u64 {
        let res = fast_hash(self.state) & fast_hash(self.state + 1) & fast_hash(self.state + 2);
        self.state += 3;
        res
    }
}

fn generate_rook_mask(rook_sq: u8) -> u64 {
    let mut result: u64 = 0;
    let (rank, file) = Square(rook_sq).to_rank_file();

    for r in rank + 1..7 {
        let Square(sq) = Square::from_rank_file(r, file);
        result |= 1 << sq;
    }

    for r in 1..rank {
        let Square(sq) = Square::from_rank_file(r, file);
        result |= 1 << sq;
    }

    for f in file + 1..7 {
        let Square(sq) = Square::from_rank_file(rank, f);
        result |= 1 << sq;
    }

    for f in 1..file {
        let Square(sq) = Square::from_rank_file(rank, f);
        result |= 1 << sq;
    }

    result
}

fn generate_rook_attacks(rook_sq: u8, occ: u64) -> u64 {
    let mut result: u64 = 0;
    let (rank, file) = Square(rook_sq).to_rank_file();

    for r in rank + 1..8 {
        let Square(sq) = Square::from_rank_file(r, file);
        let mask = 1u64 << sq;
        if (mask & occ) != 0 {
            break;
        }
        result |= mask;
    }

    for r in (0..rank).rev() {
        let Square(sq) = Square::from_rank_file(r, file);
        let mask = 1u64 << sq;
        if (mask & occ) != 0 {
            break;
        }
        result |= mask;
    }

    for f in file + 1..8 {
        let Square(sq) = Square::from_rank_file(rank, f);
        let mask = 1u64 << sq;
        if (mask & occ) != 0 {
            break;
        }
        result |= mask;
    }

    for f in (0..file).rev() {
        let Square(sq) = Square::from_rank_file(rank, f);
        let mask = 1u64 << sq;
        if (mask & occ) != 0 {
            break;
        }
        result |= mask;
    }

    result
}

fn generate_bishop_mask(bishop_sq: u8) -> u64 {
    let mut result: u64 = 0;
    let (rank, file) = Square(bishop_sq).to_rank_file();

    for (r, f) in (rank + 1..7).zip(file + 1..7) {
        let Square(sq) = Square::from_rank_file(r, f);
        result |= 1 << sq;
    }

    for (r, f) in (1..rank).rev().zip(file + 1..7) {
        let Square(sq) = Square::from_rank_file(r, f);
        result |= 1 << sq;
    }

    for (r, f) in (rank + 1..7).zip((1..file).rev()) {
        let Square(sq) = Square::from_rank_file(r, f);
        result |= 1 << sq;
    }

    for (r, f) in (1..rank).rev().zip((1..file).rev()) {
        let Square(sq) = Square::from_rank_file(r, f);
        result |= 1 << sq;
    }

    result
}

fn generate_bishop_attacks(bishop_sq: u8, occ: u64) -> u64 {
    let mut result: u64 = 0;
    let (rank, file) = Square(bishop_sq).to_rank_file();

    for (r, f) in (rank + 1..8).zip(file + 1..8) {
        let Square(sq) = Square::from_rank_file(r, f);
        let mask = 1u64 << sq;
        if (mask & occ) != 0 {
            break;
        }
        result |= mask;
    }

    for (r, f) in (0..rank).rev().zip(file + 1..8) {
        let Square(sq) = Square::from_rank_file(r, f);
        let mask = 1u64 << sq;
        if (mask & occ) != 0 {
            break;
        }
        result |= mask;
    }

    for (r, f) in (rank + 1..8).zip((0..file).rev()) {
        let Square(sq) = Square::from_rank_file(r, f);
        let mask = 1u64 << sq;
        if (mask & occ) != 0 {
            break;
        }
        result |= mask;
    }

    for (r, f) in (0..rank).rev().zip((0..file).rev()) {
        let Square(sq) = Square::from_rank_file(r, f);
        let mask = 1u64 << sq;
        if (mask & occ) != 0 {
            break;
        }
        result |= mask;
    }

    result
}

fn find_magics(sq: u8, is_bishop: bool, random: &mut RandomFewbits) -> Magic {
    let (mut attacks, mut occs, mut used) = ([0u64; 4096], [0u64; 4096], [0u64; 4096]);
    let mut epochs = [0usize; 4096];

    let mut magic: u64 = 0;
    let mask = if is_bishop {
        generate_bishop_mask(sq)
    } else {
        generate_rook_mask(sq)
    };

    let shift = 64
        - if is_bishop {
            BISHOP_BITS[sq as usize]
        } else {
            ROOK_BITS[sq as usize]
        };
    // Carry-Rippler trick
    let mut b = 0;
    let mut num_occs = 0;
    loop {
        occs[num_occs] = b;
        attacks[num_occs] = if is_bishop {
            generate_bishop_attacks(sq, b)
        } else {
            generate_rook_attacks(sq, b)
        };
        num_occs += 1;

        b = (b.wrapping_sub(mask)) & mask;
        if b == 0 {
            break;
        }
    }

    let mut num_tries: usize = 0;
    let mut i: usize = 0;
    while i < num_occs {
        magic = 0;
        while ((magic * mask) >> 56).count_ones() < 5 {
            num_tries += 1;
            magic = random.get_rand();
        }

        num_tries += 1;
        i = 0;
        loop {
            if i >= num_occs {
                break;
            }
            let hash = magic_hash(occs[i], magic, mask, shift);

            if epochs[hash] < num_tries {
                epochs[hash] = num_tries;
                used[hash] = attacks[i];
            } else if used[hash] != attacks[i] {
                break;
            }
            i += 1;
        }
    }

    Magic {
        magic,
        mask,
        offset: 0,
        shift,
    }
}*/
