//! 单实例判定。
//!
//! 跑这个程序总是既起后台又开设置窗口,所以第二次跑必须认出"守护已经有了",
//! 否则两个调度器到点会一起响。认领靠 pid 文件,判活靠 /proc。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// 进程名。/proc/<pid>/comm 会截到 15 个字符,这个名字没那么长。
const PROCESS_NAME: &str = "nap-alarm";

/// 守护的 pid 文件。放在 XDG_RUNTIME_DIR:重启即清,不会留下隔夜的陈旧文件。
pub fn pid_file() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("nap-alarm.pid")
}

/// 这个 pid 是不是一个还活着的本程序。
///
/// 只看 /proc/<pid> 在不在是不够的:pid 会被系统回收给别的程序,那样会把一个陌生
/// 进程当成守护,于是永远起不来后台。
pub fn daemon_is_running(
    proc_root: &Path,
    pid: u32,
) -> bool {
    std::fs::read_to_string(
        proc_root.join(pid.to_string()).join("comm"),
    )
    .map(|comm| comm.trim() == PROCESS_NAME)
    .unwrap_or(false)
}

/// 试着当守护:抢到返回 true,已经有人占着返回 false。
///
/// 用 create_new 而不是"先读再写":两个实例同时起来时,内核保证只有一个能建出文件。
pub fn claim(
    pid_file: &Path,
    my_pid: u32,
    alive: impl Fn(u32) -> bool,
) -> bool {
    // 两轮:第一轮撞上别人的文件,判定是陈旧的就删掉再抢一次。
    for _ in 0..2 {
        if let Some(dir) = pid_file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }

        match OpenOptions::new().write(true).create_new(true).open(pid_file) {
            Ok(mut file) => return write!(file, "{my_pid}").is_ok(),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return false,
        }

        let holder = std::fs::read_to_string(pid_file)
            .ok()
            .and_then(|text| {
                text.trim().parse::<u32>().ok()
            });
        match holder {
            Some(pid) if alive(pid) => return false,
            // 上次被 kill -9 或者机器断电留下的文件:清掉再抢。
            _ => {
                let _ = std::fs::remove_file(pid_file);
            }
        }
    }
    false
}

/// pid 文件里记的那个守护进程号。第二次运行要靠它把窗口叫出来。
pub fn holder_pid(pid_file: &Path) -> Option<u32> {
    std::fs::read_to_string(pid_file)
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()
}

/// 守护退出时把 pid 文件收掉。留着也不致命(下次会当陈旧文件清掉),但别留。
pub fn release(pid_file: &Path) {
    let _ = std::fs::remove_file(pid_file);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nobody_is_alive(_pid: u32) -> bool {
        false
    }

    #[test]
    fn the_first_instance_claims_the_daemon_role() {
        // 没有别人占着的时候,这一次运行就是守护。
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nap-alarm.pid");

        assert!(claim(&path, 4242, nobody_is_alive));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "4242"
        );
    }

    #[test]
    fn a_second_instance_does_not_claim_it_while_the_first_is_alive(
    ) {
        // 守护还活着时再跑一次,只该开设置窗口,不该再起一个调度器。
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nap-alarm.pid");
        assert!(claim(&path, 4242, nobody_is_alive));

        assert!(!claim(&path, 4243, |pid| pid == 4242));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "4242",
            "第二个实例不该顶掉守护写的 pid"
        );
    }

    #[test]
    fn a_stale_pid_file_is_taken_over() {
        // 上次被 kill -9 或者机器断电,pid 文件还在但进程早没了:不能就此再也起不来。
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nap-alarm.pid");
        std::fs::write(&path, "4242").unwrap();

        assert!(claim(&path, 4243, nobody_is_alive));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "4243"
        );
    }

    #[test]
    fn the_holder_pid_is_read_back_from_the_file() {
        // 第二次运行靠这个号找到守护;读不出来就只能干瞪眼,窗口永远叫不出来。
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nap-alarm.pid");

        assert_eq!(
            holder_pid(&path),
            None,
            "文件不存在时不该编一个号出来"
        );

        assert!(claim(&path, 4242, |_| false));
        assert_eq!(holder_pid(&path), Some(4242));
    }

    #[test]
    fn a_pid_reused_by_another_program_is_not_mistaken_for_the_daemon(
    ) {
        // pid 会被系统回收给别的程序,只看 /proc/<pid> 在不在会把陌生进程当成守护。
        let proc_root = tempfile::tempdir().unwrap();
        std::fs::create_dir(proc_root.path().join("4242"))
            .unwrap();
        std::fs::write(
            proc_root.path().join("4242").join("comm"),
            "sshd\n",
        )
        .unwrap();
        std::fs::create_dir(proc_root.path().join("4243"))
            .unwrap();
        std::fs::write(
            proc_root.path().join("4243").join("comm"),
            "nap-alarm\n",
        )
        .unwrap();

        assert!(!daemon_is_running(proc_root.path(), 4242));
        assert!(daemon_is_running(proc_root.path(), 4243));
    }
}
