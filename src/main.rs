//! 入口:`nap-alarm daemon` 跑常驻调度,不带参数打开设置窗口。
//!
//! 守护自己算时间,不依赖 systemd timer:每 20 秒轮询一次,读一遍配置再问 Scheduler
//! 这一分钟有没有闹钟该响。轮询而不是"睡到下一次触发",是因为挂起唤醒和改系统时间
//! 都会让一个长长的 sleep 醒错时候,而重新读一遍当前时间永远不会错。

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{
    Datelike, Local, NaiveTime, Timelike, Weekday,
};
use slint::{ComponentHandle, ModelRc, VecModel};

use nap_alarm::config::{self, Alarm, Config};
use nap_alarm::headset;
use nap_alarm::schedule::{self, Scheduler};
use nap_alarm::tray;
use nap_alarm::{AlarmRow, RingWindow, SettingsWindow};

/// 轮询间隔。闹钟按分钟触发,20 秒保证每一分钟至少被查到一次。
const POLL_INTERVAL: Duration = Duration::from_secs(20);
/// 界面里星期的排列顺序,周一在最前。
const WEEKDAYS: [Weekday; 7] = [
    Weekday::Mon,
    Weekday::Tue,
    Weekday::Wed,
    Weekday::Thu,
    Weekday::Fri,
    Weekday::Sat,
    Weekday::Sun,
];

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("daemon") => daemon(),
        None => settings(),
        Some(other) => {
            eprintln!("nap-alarm: 不认识的参数 {other:?};用法:nap-alarm [daemon]");
            std::process::exit(2);
        }
    }
}

/// 常驻调度。窗口关掉也不退出,所以走 run_event_loop_until_quit。
fn daemon() {
    let path = config::default_path();
    let scheduler = Rc::new(RefCell::new(Scheduler::new()));

    // 托盘先起:守护没有常驻窗口,没有图标就等于隐形。
    let tray = tray::spawn("没有闹钟".into());
    // 正在响的那个窗口得有人拿着:句柄一丢,窗口就没了。
    let ringing: Rc<RefCell<Option<RingWindow>>> =
        Rc::new(RefCell::new(None));

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        POLL_INTERVAL,
        move || {
            let config = match config::load(&path) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("nap-alarm: {error}");
                    return;
                }
            };

            let now = Local::now().naive_local();
            if let Some(tray) = tray.as_ref() {
                let summary = next_fire_summary(&config, now);
                tray.update(move |tray| tray.next_fire = summary);
            }

            for index in scheduler
                .borrow_mut()
                .poll(&config.alarms, now)
            {
                let alarm = &config.alarms[index];
                let sink = headset::connected_headset();
                // 耳机没连就整轮跳过:外放会吵到别人,而且人没戴耳机时本来也叫不醒。
                if alarm.require_headset && sink.is_none() {
                    // 出一声日志:跳过和"闹钟坏了"在外面看起来一模一样。
                    eprintln!("nap-alarm: {} 到点了,但耳机没连,跳过", alarm.label);
                    continue;
                }
                ring(alarm, &config.sound, sink, &ringing);
            }
        },
    );

    if let Err(error) = slint::run_event_loop_until_quit() {
        eprintln!("nap-alarm: 事件循环起不来:{error}");
        std::process::exit(1);
    }
}

/// 弹响铃窗口并开始放声,直到窗口被点掉或关掉。
fn ring(
    alarm: &Alarm,
    sound: &Path,
    sink: Option<String>,
    ringing: &Rc<RefCell<Option<RingWindow>>>,
) {
    let window = match RingWindow::new() {
        Ok(window) => window,
        Err(error) => {
            eprintln!(
                "nap-alarm: 响铃窗口建不出来:{error}"
            );
            return;
        }
    };
    window.set_label(alarm.label.clone().into());
    window.set_time(
        alarm.time.format("%H:%M").to_string().into(),
    );

    let stop = Arc::new(AtomicBool::new(false));
    if let Some(sound) = ring_sound(sound) {
        let stop = stop.clone();
        std::thread::spawn(move || {
            play_until_stopped(
                &sound,
                sink.as_deref(),
                &stop,
            )
        });
    } else {
        eprintln!("nap-alarm: 没有配铃声,只弹窗不响");
    }

    // 点窗口任何地方、或者用窗口管理器关掉,都算把闹钟按掉。
    let silence = {
        let stop = stop.clone();
        let ringing = ringing.clone();
        let handle = window.as_weak();
        move || {
            stop.store(true, Ordering::Relaxed);
            if let Some(window) = handle.upgrade() {
                let _ = window.hide();
            }
            ringing.replace(None);
        }
    };
    window.on_stop(silence.clone());
    window.window().on_close_requested(move || {
        silence();
        slint::CloseRequestResponse::HideWindow
    });

    if let Err(error) = window.show() {
        eprintln!("nap-alarm: 响铃窗口显示不出来:{error}");
        stop.store(true, Ordering::Relaxed);
        return;
    }
    ringing.replace(Some(window));
}

/// 铃声取配置里的路径,配置没写就退回环境变量 NAP_ALARM_SOUND(打包时塞的默认音)。
fn ring_sound(configured: &Path) -> Option<PathBuf> {
    if !configured.as_os_str().is_empty() {
        return Some(configured.to_path_buf());
    }
    std::env::var_os("NAP_ALARM_SOUND").map(PathBuf::from)
}

/// 一遍一遍地放,直到被按掉。--target 钉住耳机,默认输出可能还在音箱上。
fn play_until_stopped(
    sound: &Path,
    sink: Option<&str>,
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::Relaxed) {
        let mut command = Command::new("pw-play");
        if let Some(sink) = sink {
            command.arg(format!("--target={sink}"));
        }
        let Ok(mut child) = command.arg(sound).spawn()
        else {
            eprintln!("nap-alarm: pw-play 起不来,放不了声");
            return;
        };

        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {}
                Err(_) => return,
            }
            // 按掉的时候正在放的那一遍也要立刻掐掉,否则还要再响六秒。
            if stop.load(Ordering::Relaxed) {
                let _ = child.kill();
                // 杀完还得收尸:守护是常驻的,不 wait 就每响一次攒一个僵尸进程。
                let _ = child.wait();
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// 设置窗口之外的全部状态。改动即时存盘,所以界面上没有保存按钮。
struct Editor {
    path: PathBuf,
    config: RefCell<Config>,
    rows: Rc<VecModel<AlarmRow>>,
}

impl Editor {
    /// 改一处配置:落盘,再把界面刷成配置的样子。
    fn edit(&self, mutate: impl FnOnce(&mut Config)) {
        let mut config = self.config.borrow_mut();
        mutate(&mut config);
        if let Err(error) =
            config::save(&self.path, &config)
        {
            eprintln!("nap-alarm: {error}");
        }
        self.rows.set_vec(rows(&config));
    }
}

fn settings() {
    let path = config::default_path();
    let config =
        config::load(&path).unwrap_or_else(|error| {
            eprintln!("nap-alarm: {error};先当空配置打开");
            Config::default()
        });

    let window = match SettingsWindow::new() {
        Ok(window) => window,
        Err(error) => {
            eprintln!(
                "nap-alarm: 设置窗口建不出来:{error}"
            );
            std::process::exit(1);
        }
    };

    let editor = Rc::new(Editor {
        path,
        rows: Rc::new(VecModel::from(rows(&config))),
        config: RefCell::new(config),
    });
    window.set_alarms(ModelRc::from(editor.rows.clone()));
    window.set_sound(
        editor
            .config
            .borrow()
            .sound
            .display()
            .to_string()
            .into(),
    );

    window.on_add_alarm({
        let editor = editor.clone();
        move || {
            editor.edit(|config| {
                config.alarms.push(default_alarm())
            })
        }
    });
    window.on_delete_alarm({
        let editor = editor.clone();
        move |index| {
            editor.edit(|config| {
                config.alarms.remove(index as usize);
            })
        }
    });
    window.on_toggle_enabled({
        let editor = editor.clone();
        move |index| {
            editor.edit(|config| {
                let alarm =
                    &mut config.alarms[index as usize];
                alarm.enabled = !alarm.enabled;
            })
        }
    });
    window.on_toggle_headset({
        let editor = editor.clone();
        move |index| {
            editor.edit(|config| {
                let alarm =
                    &mut config.alarms[index as usize];
                alarm.require_headset =
                    !alarm.require_headset;
            })
        }
    });
    window.on_toggle_day({
        let editor = editor.clone();
        move |index, weekday| {
            editor.edit(|config| {
                let day = WEEKDAYS[weekday as usize];
                let days =
                    &mut config.alarms[index as usize].days;
                match days
                    .iter()
                    .position(|listed| *listed == day)
                {
                    Some(at) => {
                        days.remove(at);
                    }
                    None => days.push(day),
                }
                days.sort_by_key(|day| {
                    day.num_days_from_monday()
                });
            })
        }
    });
    window.on_set_label({
        let editor = editor.clone();
        move |index, label| {
            editor.edit(|config| {
                config.alarms[index as usize].label =
                    label.to_string()
            })
        }
    });
    window.on_set_time({
        let editor = editor.clone();
        move |index, text| {
            // 写错的时间不落盘:刷新会把输入框弹回原值,一眼看得出没被接受。
            match NaiveTime::parse_from_str(&text, "%H:%M") {
                Ok(time) => editor.edit(|config| config.alarms[index as usize].time = time),
                Err(_) => {
                    eprintln!("nap-alarm: 时间 {text:?} 认不出来,应当形如 13:30");
                    editor.edit(|_| {});
                }
            }
        }
    });
    window.on_set_sound({
        let editor = editor.clone();
        move |sound| {
            editor.edit(|config| {
                config.sound =
                    PathBuf::from(sound.to_string())
            })
        }
    });

    if let Err(error) = window.run() {
        eprintln!("nap-alarm: 设置窗口跑不起来:{error}");
        std::process::exit(1);
    }
}

/// 新闹钟的默认样子:工作日午休结束,且只在耳机连着时响。
fn default_alarm() -> Alarm {
    Alarm {
        label: "闹钟".into(),
        time: NaiveTime::from_hms_opt(13, 30, 0)
            .unwrap_or_default(),
        days: WEEKDAYS[..5].to_vec(),
        enabled: true,
        require_headset: true,
    }
}

/// 把配置铺成界面要的行。
fn rows(config: &Config) -> Vec<AlarmRow> {
    let now = Local::now().naive_local();
    config
        .alarms
        .iter()
        .map(|alarm| AlarmRow {
            label: alarm.label.clone().into(),
            time: alarm
                .time
                .format("%H:%M")
                .to_string()
                .into(),
            days: ModelRc::new(VecModel::from(
                WEEKDAYS
                    .iter()
                    .map(|day| alarm.days.contains(day))
                    .collect::<Vec<_>>(),
            )),
            enabled: alarm.enabled,
            require_headset: alarm.require_headset,
            next_fire: next_fire_text(alarm, now).into(),
        })
        .collect()
}

/// 所有闹钟里最早的那一次,给托盘显示。
fn next_fire_summary(
    config: &Config,
    now: chrono::NaiveDateTime,
) -> String {
    match config
        .alarms
        .iter()
        .filter_map(|alarm| schedule::next_fire(alarm, now))
        .min()
    {
        Some(next) => format!(
            "下次 {} {:02}:{:02}",
            weekday_name(next.date().weekday()),
            next.hour(),
            next.minute()
        ),
        None => "没有闹钟".into(),
    }
}

fn next_fire_text(
    alarm: &Alarm,
    now: chrono::NaiveDateTime,
) -> String {
    match schedule::next_fire(alarm, now) {
        Some(next) => format!(
            "下次 {} {:02}:{:02}",
            weekday_name(next.date().weekday()),
            next.hour(),
            next.minute()
        ),
        None => "不会响".into(),
    }
}

fn weekday_name(day: Weekday) -> &'static str {
    ["周一", "周二", "周三", "周四", "周五", "周六", "周日"]
        [day.num_days_from_monday() as usize]
}
