# cargo 的活都得在 dev shell 里跑:slint 编译期要 pkg-config 找到 fontconfig,
# 运行期还要 dlopen wayland/libxkbcommon/libGL(见 flake.nix)。
shell := "nix develop -c"

default:
    @just --list

run:
    {{shell}} cargo run

test:
    {{shell}} cargo test

check:
    {{shell}} cargo fmt --check
    {{shell}} cargo clippy --all-targets -- -D warnings
    {{shell}} cargo test
