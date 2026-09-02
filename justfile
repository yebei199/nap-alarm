# cargo 的活都得在 dev shell 里跑:slint 编译期要 pkg-config 找到 fontconfig,
# 运行期还要 dlopen wayland/libxkbcommon/libGL(见 flake.nix)。
shell := "nix develop -c"

default:
    @just --list

# 设置窗口。不带参数跑的就是它,托盘图标不在这个模式里。
run:
    {{shell}} cargo run

# 常驻守护:调度、响铃、托盘图标都在这个模式下,想看托盘就跑这个。
daemon:
    # 先清掉上一个实例,否则两个守护会同时响。
    -pkill -x nap-alarm
    {{shell}} cargo run -- daemon

test:
    {{shell}} cargo test

check:
    {{shell}} cargo fmt --check
    {{shell}} cargo clippy --all-targets -- -D warnings
    {{shell}} cargo test

# 截图到 /tmp/nap-alarm-shot.png
shot:
    #!/usr/bin/env bash
    # 两个坑:合成器不给不可见窗口发 frame callback,不先 focus 抓到的是过期缓冲;
    # niri 异步落盘,得按 marker 文件的时间过滤,否则挑到上一张旧图。
    set -euo pipefail
    pkill -x nap-alarm || true
    {{shell}} cargo build
    setsid ./target/debug/nap-alarm &
    for _ in $(seq 30); do
        id=$(niri msg --json windows | jq -r '.[] | select(.title=="闹钟设置") | .id' | head -1)
        [ -n "$id" ] && break
        sleep 1
    done
    niri msg action focus-window --id "$id"
    sleep 2
    marker=$(mktemp)
    niri msg action screenshot-window --id "$id"
    for _ in $(seq 20); do
        shot=$(find ~/Pictures/Screenshots -name '*.png' -newer "$marker" | head -1)
        [ -n "$shot" ] && break
        sleep 0.5
    done
    mv "$shot" /tmp/nap-alarm-shot.png
    echo "/tmp/nap-alarm-shot.png"
