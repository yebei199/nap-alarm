//! 闹钟配置:~/.config/nap-alarm/alarms.toml 的读写与校验。
//!
//! 时间与星期在反序列化时就解析成 chrono 类型,配置文件写错在读取那一刻就报错,
//! 而不是等到某天该响的时候悄悄不响。

use std::path::{Path, PathBuf};

use chrono::{NaiveTime, Weekday};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
};

/// 配置读写过程中可能出的岔子。
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("读不了配置文件 {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("写不了配置文件 {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("配置文件 {path} 有问题: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("配置存不成 TOML: {0}")]
    Encode(#[from] toml::ser::Error),
}

/// 一条闹钟。
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize,
)]
pub struct Alarm {
    /// 响铃时显示的名字。
    pub label: String,
    #[serde(
        serialize_with = "serialize_time",
        deserialize_with = "deserialize_time"
    )]
    pub time: NaiveTime,
    /// 星期几生效,空列表等于这条闹钟永远不响。
    #[serde(
        serialize_with = "serialize_days",
        deserialize_with = "deserialize_days"
    )]
    pub days: Vec<Weekday>,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// 只在蓝牙耳机连着时才响。
    #[serde(default)]
    pub require_headset: bool,
}

/// 整个配置文件。
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize,
)]
pub struct Config {
    /// 铃声文件,所有闹钟共用。
    pub sound: PathBuf,
    #[serde(default, rename = "alarm")]
    pub alarms: Vec<Alarm>,
}

fn enabled_by_default() -> bool {
    true
}

fn serialize_time<S: Serializer>(
    time: &NaiveTime,
    ser: S,
) -> Result<S::Ok, S::Error> {
    ser.serialize_str(&time.format("%H:%M").to_string())
}

fn deserialize_time<'de, D: Deserializer<'de>>(
    de: D,
) -> Result<NaiveTime, D::Error> {
    let raw = String::deserialize(de)?;
    NaiveTime::parse_from_str(&raw, "%H:%M").map_err(|_| {
        serde::de::Error::custom(format!(
            "时间 {raw:?} 认不出来,应当形如 \"13:30\""
        ))
    })
}

fn serialize_days<S: Serializer>(
    days: &[Weekday],
    ser: S,
) -> Result<S::Ok, S::Error> {
    ser.collect_seq(
        days.iter()
            .map(|day| day.to_string().to_lowercase()),
    )
}

fn deserialize_days<'de, D: Deserializer<'de>>(
    de: D,
) -> Result<Vec<Weekday>, D::Error> {
    let raw = Vec::<String>::deserialize(de)?;
    raw.into_iter()
        .map(|name| {
            name.parse::<Weekday>().map_err(|_| {
                serde::de::Error::custom(format!("星期 {name:?} 认不出来,应当形如 \"mon\""))
            })
        })
        .collect()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sound: PathBuf::new(),
            alarms: Vec::new(),
        }
    }
}

/// 配置文件的默认位置:$XDG_CONFIG_HOME/nap-alarm/alarms.toml。
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                std::env::var_os("HOME")
                    .unwrap_or_default(),
            )
            .join(".config")
        });
    base.join("nap-alarm").join("alarms.toml")
}

/// 读配置。文件不存在算"一个闹钟都没有",不是错误。
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(source)
            if source.kind()
                == std::io::ErrorKind::NotFound =>
        {
            return Ok(Config::default())
        }
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            })
        }
    };

    toml::from_str(&text).map_err(|source| {
        ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        }
    })
}

/// 写配置,目录不存在就建出来。
pub fn save(
    path: &Path,
    config: &Config,
) -> Result<(), ConfigError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| {
            ConfigError::Write {
                path: dir.to_path_buf(),
                source,
            }
        })?;
    }

    std::fs::write(path, toml::to_string_pretty(config)?)
        .map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noon_nap() -> Alarm {
        Alarm {
            label: "午休结束".into(),
            time: NaiveTime::from_hms_opt(13, 30, 0)
                .unwrap(),
            days: vec![Weekday::Mon, Weekday::Fri],
            enabled: true,
            require_headset: true,
        }
    }

    #[test]
    fn config_round_trips_through_toml() {
        // 界面存盘再读回来,闹钟内容必须一模一样,否则改一次设置丢一次数据。
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alarms.toml");
        let config = Config {
            sound: PathBuf::from("/tmp/ring.ogg"),
            alarms: vec![noon_nap()],
        };

        save(&path, &config).unwrap();

        assert_eq!(load(&path).unwrap(), config);
    }

    #[test]
    fn a_missing_config_file_yields_an_empty_alarm_list() {
        // 头一次运行没有配置文件:该当作"一个闹钟都没有",而不是报错退出。
        let dir = tempfile::tempdir().unwrap();

        let config =
            load(&dir.path().join("没有这个文件.toml"))
                .unwrap();

        assert!(config.alarms.is_empty());
    }

    #[test]
    fn an_unparsable_time_is_rejected_with_the_offending_value(
    ) {
        // 手写配置写错时间:错误信息里要带上那个写错的值,否则无从改起。
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alarms.toml");
        std::fs::write(
            &path,
            "sound = \"/tmp/ring.ogg\"\n\n[[alarm]]\nlabel = \"午休结束\"\ntime = \"25:70\"\ndays = [\"mon\"]\n",
        )
        .unwrap();

        let error = load(&path).unwrap_err().to_string();

        assert!(
            error.contains("25:70"),
            "错误信息里没有那个写错的值: {error}"
        );
    }

    #[test]
    fn an_unknown_weekday_name_is_rejected() {
        // 手写配置把星期拼错:宁可报错,也不要静悄悄当成"这天不响"。
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alarms.toml");
        std::fs::write(
            &path,
            "sound = \"/tmp/ring.ogg\"\n\n[[alarm]]\nlabel = \"午休结束\"\ntime = \"13:30\"\ndays = [\"星期一\"]\n",
        )
        .unwrap();

        let error = load(&path).unwrap_err().to_string();

        assert!(
            error.contains("星期一"),
            "错误信息里没有那个写错的值: {error}"
        );
    }
}
