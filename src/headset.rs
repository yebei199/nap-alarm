//! 蓝牙耳机判定。
//!
//! 找的是 PipeWire 里的 bluez 输出节点:它存在就说明耳机连着且已经能出声,比问
//! bluetoothctl 更贴近"闹钟能不能送进耳朵"——蓝牙鼠标键盘不会有 bluez_output。

/// 从 pw-dump 的输出里认出第一个 bluez 输出节点名。
pub fn parse_bluez_sink(
    pw_dump_output: &str,
) -> Option<String> {
    let start = pw_dump_output.find("bluez_output.")?;
    Some(
        pw_dump_output[start..]
            .chars()
            .take_while(|c| {
                c.is_ascii_alphanumeric()
                    || matches!(c, '_' | '.' | '-')
            })
            .collect(),
    )
}

/// 跑一次 pw-dump,问现在有没有连着的蓝牙耳机。
pub fn connected_headset() -> Option<String> {
    let dump = std::process::Command::new("pw-dump")
        .output()
        .ok()?;
    parse_bluez_sink(&String::from_utf8_lossy(&dump.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bluez_sink_name_is_extracted_from_pw_dump_output(
    ) {
        // 从真实 pw-dump 输出里认出耳机节点名,响铃要靠它把声音钉到耳机上。
        let dump = r#"
        { "info": { "props": { "node.name": "alsa_output.pci-0000_00_1f.3.iec958-stereo" } } },
        { "info": { "props": { "node.name": "bluez_output.80_99_E7_FC_F7_9C.1", "media.class": "Audio/Sink" } } }
        "#;

        assert_eq!(
            parse_bluez_sink(dump).as_deref(),
            Some("bluez_output.80_99_E7_FC_F7_9C.1")
        );
    }

    #[test]
    fn output_without_any_bluez_node_means_no_headset() {
        // 只有板载声卡:判定为耳机没连,这一轮闹钟整个跳过。
        let dump = r#"
        { "info": { "props": { "node.name": "alsa_output.pci-0000_00_1f.3.iec958-stereo" } } }
        "#;

        assert_eq!(parse_bluez_sink(dump), None);
    }
}
