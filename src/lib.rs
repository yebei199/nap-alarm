//! 一个只在蓝牙耳机连着时才响的小闹钟。
//!
//! 调度、配置、耳机判定都是纯逻辑,放在库里供测试直接调用;界面与响铃在 main.rs。
slint::include_modules!();

pub mod config;
pub mod headset;
pub mod schedule;
pub mod tray;
