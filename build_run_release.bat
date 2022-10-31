@echo off
cargo build --release
cd target\release
xm_player.exe
