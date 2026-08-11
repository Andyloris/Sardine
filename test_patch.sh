#!/bin/sh

BOOK="$1"

mkdir -p test/
RUSTFLAGS="-C target-cpu=native" cargo build --release
cp target/release/final_sardine test/eng1

git checkout HEAD^1
RUSTFLAGS="-C target-cpu=native" cargo build --release
cp target/release/final_sardine test/eng2

git switch -
fastchess -engine cmd=./test/eng1 name=NewSardine \
	-engine cmd=./test/eng2 name=OldSardine \
	-each tc=8+0.08 \
	-rounds 1000000 \
	-repeat \
	-concurrency 15 \
	-recover \
	-openings file="$BOOK" format=pgn order=random \
	-sprt elo0=0 elo1=5 alpha=0.05 beta=0.05 model=logistic
