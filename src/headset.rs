//! 蓝牙耳机判定:pw-dump 里存在 bluez 输出节点,就说明耳机连着且能出声。

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "骨架待确认"]
    fn the_bluez_sink_name_is_extracted_from_pw_dump_output(
    ) {
        // 从真实 pw-dump 输出里认出耳机节点名,响铃要靠它把声音钉到耳机上。
        todo!()
    }

    #[test]
    #[ignore = "骨架待确认"]
    fn output_without_any_bluez_node_means_no_headset() {
        // 只有板载声卡:判定为耳机没连,这一轮闹钟整个跳过。
        todo!()
    }
}
