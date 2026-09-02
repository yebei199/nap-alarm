//! 闹钟配置:~/.config/nap-alarm/alarms.toml 的读写与校验。

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "骨架待确认"]
    fn config_round_trips_through_toml() {
        // 界面存盘再读回来,闹钟内容必须一模一样,否则改一次设置丢一次数据。
        todo!()
    }

    #[test]
    #[ignore = "骨架待确认"]
    fn a_missing_config_file_yields_an_empty_alarm_list() {
        // 头一次运行没有配置文件:该当作"一个闹钟都没有",而不是报错退出。
        todo!()
    }

    #[test]
    #[ignore = "骨架待确认"]
    fn an_unparsable_time_is_rejected_with_the_offending_value(
    ) {
        // 手写配置写错时间:错误信息里要带上那个写错的值,否则无从改起。
        todo!()
    }

    #[test]
    #[ignore = "骨架待确认"]
    fn an_unknown_weekday_name_is_rejected() {
        // 手写配置把星期拼错:宁可报错,也不要静悄悄当成"这天不响"。
        todo!()
    }
}
