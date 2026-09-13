#!/bin/bash
export DISPLAY=:0
cd /home/iostream/projects/kraken-rust
exec ./target/release/kraken-rs "$@"
