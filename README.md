# Sardine

Sardine is a UCI chess engine written in Rust. It implements minimal chess knowledge in its evaluation function, relying instead on a strong alpha-beta search with a number of pruning heuristics.

## Features

- Board Representation
  - Bitboards with Little-Endian Rank-File Mapping
  - Magic bitboards
  - BMI2 - PEXT Bitboards
  - 8x8 Mailbox

- Search
  - Negamax with Fail-Soft Alpha-Beta
  - Iterative Deepening
  - Move Ordering
    - TT Move
    - Captures using MVV-LVA and the Capture history heuristic
    - Killer Moves
    - Quiet moves by the Butterfly history heuristic 
  - Quiescence Search
  - Transposition table
  - Principal variation search
  - Reverse futility pruning
  - Null move pruning
  - Late move reductions
  - Futility pruning
  - Internal iterative reductions
  - Improving
  - Static exchange evaluation pruning
  - Singular extensions

- Evaluation
  - PeSTO's evaluation function using Texel tuned material values and Piece-Square Tables
  - Tempo
  - Score grain
  - Bishop pair bonus
  - Incremental updates
  - Tapered evaluation
  - 50 moves rule continuity

## Building

0. Install [Rust](https://rust-lang.org/tools/install/) and if you intend on testing patches, [fastchess](https://github.com/Disservin/fastchess)
1. Clone this repository: `git clone https://github.com/Andyloris/Sardine`
2. Enter the source directory: `cd Sardine`
3. Build: `RUSTFLAGS="-C target-cpu=native" cargo build --release` If you are **not** compiling on an older AMD cpu (pre-Zen 2), you can enable the PEXT-based magic bitboards: `RUSTFLAGS="-C target-cpu=native" cargo build --release --features pext_magics`
4. The engine binary will be at `target/release/sardine`

## Usage

Sardine is a chess engine implementing the UCI protocol; not a graphical user interface (or GUI). To use the engine, consider installing a chess GUI that supports UCI engines (e.g. [Cutechess](https://github.com/cutechess/cutechess))
For a list of UCI commands, see the [UCI wikipedia article](https://en.wikipedia.org/wiki/Universal_Chess_Interface)

## Contributing

Each PR must be statistically tested using [SPRT](https://chessprogramming.org/Sequential_Probability_Ratio_Test). The provided `test_patch.sh` script can be used to test a patch automatically by comparing the current committed changes against the previous committed version. **Running the script may DISCARD LOCAL CHANGES that haven't been committed, YOU HAVE BEEN WARNED**

## License

Sardine is distributed under the [GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.en.html)

