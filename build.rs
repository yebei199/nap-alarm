//! 编译期把 ui/alarm.slint 编译成 Rust 代码。
fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_style("material".into());
    slint_build::compile_with_config(
        "ui/alarm.slint",
        config,
    )
    .expect("ui/alarm.slint compiles");
}
