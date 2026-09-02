//! 闹钟何时该响:纯函数,不碰时钟也不碰文件,时间一律由调用方传进来。

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "骨架待确认"]
    fn alarm_fires_on_a_listed_weekday_at_its_exact_minute()
    {
        // 正常路径:星期在列表里、时分正好对上,这一分钟该响。
        todo!()
    }

    #[test]
    #[ignore = "骨架待确认"]
    fn alarm_stays_silent_on_a_weekday_it_does_not_list() {
        // 周末不午休:同样的时分,星期不在列表里就不该响。
        todo!()
    }

    #[test]
    #[ignore = "骨架待确认"]
    fn disabled_alarm_never_fires() {
        // 界面上把闹钟关掉,等于这条闹钟不存在,时间再对也不响。
        todo!()
    }

    #[test]
    #[ignore = "骨架待确认"]
    fn alarm_fires_only_once_within_the_same_minute() {
        // 守护每 20 秒轮询一次,同一分钟会被查到三次,但闹钟只该响一次。
        todo!()
    }

    #[test]
    #[ignore = "骨架待确认"]
    fn next_fire_skips_to_the_following_week_when_today_is_already_past(
    ) {
        // 只设了周一、而现在是周一下午:下一次应当是下周一,不是今天。
        todo!()
    }

    #[test]
    #[ignore = "骨架待确认"]
    fn a_minute_missed_while_suspended_does_not_fire_late()
    {
        // 合盖睡眠跨过了触发时刻,醒来已是几小时后:不该补响,那只会莫名其妙。
        todo!()
    }
}
