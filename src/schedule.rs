//! 闹钟何时该响。
//!
//! 判定是纯函数,时间由调用方传进来;守护每 20 秒轮询一次,靠 [`Scheduler`] 记住
//! 哪条闹钟在这一分钟已经响过了。

use std::collections::HashMap;

use chrono::{Datelike, NaiveDateTime, Timelike};

use crate::config::Alarm;

/// 这一分钟这条闹钟该不该响。只认当前这一分钟:错过的时刻不补响。
pub fn is_due(alarm: &Alarm, now: NaiveDateTime) -> bool {
    alarm.enabled
        && alarm.days.contains(&now.weekday())
        && now.hour() == alarm.time.hour()
        && now.minute() == alarm.time.minute()
}

/// 下一次响铃的时刻,给界面显示用。没有生效的星期就没有下一次。
pub fn next_fire(
    alarm: &Alarm,
    now: NaiveDateTime,
) -> Option<NaiveDateTime> {
    if !alarm.enabled {
        return None;
    }

    // 最多往后找七天:再远也是同一个星期几,答案不会变。
    (0..=7).find_map(|offset| {
        let candidate = (now.date()
            + chrono::Duration::days(offset))
        .and_time(alarm.time);
        (alarm.days.contains(&candidate.weekday())
            && candidate > now)
            .then_some(candidate)
    })
}

/// 记住每条闹钟最近响过的那一分钟,免得 20 秒一轮的轮询让它一分钟里响三次。
#[derive(Debug, Default)]
pub struct Scheduler {
    fired: HashMap<String, NaiveDateTime>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// 轮询一次,返回这一轮该响的闹钟在 `alarms` 里的下标。
    pub fn poll(
        &mut self,
        alarms: &[Alarm],
        now: NaiveDateTime,
    ) -> Vec<usize> {
        // 记的是"哪一分钟响过",不是"响过没有":同一条闹钟明天还要再响一次。
        let minute = now
            .with_second(0)
            .and_then(|t| t.with_nanosecond(0))
            .unwrap_or(now);

        alarms
            .iter()
            .enumerate()
            .filter(|(_, alarm)| is_due(alarm, now))
            .filter(|(_, alarm)| {
                let key = alarm_key(alarm);
                if self.fired.get(&key) == Some(&minute) {
                    return false;
                }
                self.fired.insert(key, minute);
                true
            })
            .map(|(index, _)| index)
            .collect()
    }
}

/// 闹钟的身份:配置改过之后下标会变,名字加时间不会。
fn alarm_key(alarm: &Alarm) -> String {
    format!("{}@{}", alarm.label, alarm.time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveTime, Weekday};

    // 2026-09-07 是星期一,同周的 09-12 是星期六。
    fn monday(hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 7)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    fn saturday(hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 12)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    fn weekday_nap() -> Alarm {
        Alarm {
            label: "午休结束".into(),
            time: NaiveTime::from_hms_opt(13, 30, 0)
                .unwrap(),
            days: vec![
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
            ],
            enabled: true,
            require_headset: true,
        }
    }

    #[test]
    fn alarm_fires_on_a_listed_weekday_at_its_exact_minute()
    {
        // 正常路径:星期在列表里、时分正好对上,这一分钟该响。
        assert!(is_due(&weekday_nap(), monday(13, 30)));
    }

    #[test]
    fn alarm_stays_silent_on_a_weekday_it_does_not_list() {
        // 周末不午休:同样的时分,星期不在列表里就不该响。
        assert!(!is_due(&weekday_nap(), saturday(13, 30)));
    }

    #[test]
    fn disabled_alarm_never_fires() {
        // 界面上把闹钟关掉,等于这条闹钟不存在,时间再对也不响。
        let alarm = Alarm {
            enabled: false,
            ..weekday_nap()
        };

        assert!(!is_due(&alarm, monday(13, 30)));
    }

    #[test]
    fn alarm_fires_only_once_within_the_same_minute() {
        // 守护每 20 秒轮询一次,同一分钟会被查到三次,但闹钟只该响一次。
        let alarms = vec![weekday_nap()];
        let mut scheduler = Scheduler::new();

        let first = scheduler.poll(
            &alarms,
            monday(13, 30).with_second(0).unwrap(),
        );
        let second = scheduler.poll(
            &alarms,
            monday(13, 30).with_second(20).unwrap(),
        );
        let third = scheduler.poll(
            &alarms,
            monday(13, 30).with_second(40).unwrap(),
        );

        assert_eq!(first, vec![0]);
        assert!(
            second.is_empty(),
            "同一分钟第二次轮询不该再响"
        );
        assert!(
            third.is_empty(),
            "同一分钟第三次轮询不该再响"
        );
    }

    #[test]
    fn next_fire_skips_to_the_following_week_when_today_is_already_past(
    ) {
        // 只设了周一、而现在是周一下午:下一次应当是下周一,不是今天。
        let alarm = Alarm {
            days: vec![Weekday::Mon],
            ..weekday_nap()
        };

        let next =
            next_fire(&alarm, monday(18, 0)).unwrap();

        assert_eq!(
            next,
            monday(13, 30) + chrono::Duration::days(7)
        );
    }

    #[test]
    fn a_minute_missed_while_suspended_does_not_fire_late()
    {
        // 合盖睡眠跨过了触发时刻,醒来已是几小时后:不该补响,那只会莫名其妙。
        let alarms = vec![weekday_nap()];
        let mut scheduler = Scheduler::new();

        let before = scheduler.poll(
            &alarms,
            monday(13, 29).with_second(50).unwrap(),
        );
        let after_resume =
            scheduler.poll(&alarms, monday(15, 0));

        assert!(before.is_empty());
        assert!(
            after_resume.is_empty(),
            "睡过去的那一分钟不该醒来补响"
        );
    }
}
