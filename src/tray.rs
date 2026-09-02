//! 托盘图标。守护没有常驻窗口,不给个图标的话它在系统里就是隐形的:跑没跑、下次
//! 什么时候响、想改设置从哪进,全都无从知道。
//!
//! 走 StatusNotifierItem(D-Bus),这是 Wayland 上唯一还活着的托盘协议,DMS 的
//! Quickshell 托盘就是它的 host。

use ksni::{
    blocking::TrayMethods, menu::StandardItem, MenuItem,
    ToolTip, Tray,
};

/// 托盘里那个图标。它只显示状态,真正的状态在守护那边。
pub struct AlarmTray {
    /// 下一次响铃的说法,例如"周三 13:30";没有生效的闹钟时是"没有闹钟"。
    pub next_fire: String,
    /// 把设置窗口叫出来。窗口全程只有一个,归守护自己拿着,这里只是请它露面。
    pub open_settings: fn(),
}

impl Tray for AlarmTray {
    fn id(&self) -> String {
        "nap-alarm".into()
    }

    fn title(&self) -> String {
        "闹钟".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![clock_icon()]
    }

    /// 左键点图标就开设置。托盘图标点了没反应,谁都会以为程序卡住了 —— 而"设置在
    /// 右键菜单里"这件事,只有写它的人知道。
    fn activate(&mut self, _x: i32, _y: i32) {
        (self.open_settings)();
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "闹钟".into(),
            description: self.next_fire.clone(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: self.next_fire.clone(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "设置".into(),
                activate: Box::new(|tray: &mut Self| {
                    (tray.open_settings)()
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "退出".into(),
                activate: Box::new(|_| quit_daemon()),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// 托盘图标边长。host 会按自己的栏高缩放,32 够清楚也不占地方。
const ICON_SIZE: usize = 32;
/// 表盘外圈与内圈半径,单位像素,以图标中心为原点。
const RIM_OUTER: f32 = 15.0;
const RIM_INNER: f32 = 12.5;
/// 指针粗细(半宽)。
const HAND_HALF_WIDTH: f32 = 1.3;
/// 每个像素在两个方向上各取几个子采样点:边缘不做这一步会全是锯齿。
const SUBSAMPLES: usize = 3;

/// 画一个表盘 + 两根指针的托盘图标。
///
/// 自己画像素而不是报一个图标名,是因为图标名由 host 在它自己的主题里查:这台机器
/// 只有 hicolor 和 locolor,`alarm-symbolic` 谁都找不到,托盘上就是一块空白。
pub fn clock_icon() -> ksni::Icon {
    let center = ICON_SIZE as f32 / 2.0;
    let mut data = vec![0u8; ICON_SIZE * ICON_SIZE * 4];

    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let coverage =
                coverage_at(x as f32, y as f32, center);
            if coverage <= 0.0 {
                continue;
            }
            // SNI 要的是 ARGB32,字节序就是 A、R、G、B。白色配深色托盘栏。
            let pixel = (y * ICON_SIZE + x) * 4;
            data[pixel] = (coverage * 255.0) as u8;
            data[pixel + 1] = 255;
            data[pixel + 2] = 255;
            data[pixel + 3] = 255;
        }
    }

    ksni::Icon {
        width: ICON_SIZE as i32,
        height: ICON_SIZE as i32,
        data,
    }
}

/// 这个像素被图案盖住多少,0 到 1。
fn coverage_at(x: f32, y: f32, center: f32) -> f32 {
    let mut hits = 0;
    for sub_y in 0..SUBSAMPLES {
        for sub_x in 0..SUBSAMPLES {
            let step = 1.0 / (SUBSAMPLES as f32 + 1.0);
            let sample_x = x + step * (sub_x as f32 + 1.0);
            let sample_y = y + step * (sub_y as f32 + 1.0);
            if inside_clock(sample_x, sample_y, center) {
                hits += 1;
            }
        }
    }
    hits as f32 / (SUBSAMPLES * SUBSAMPLES) as f32
}

/// 表盘外圈,加上指向 12 点的长针与指向 2 点的短针。
fn inside_clock(x: f32, y: f32, center: f32) -> bool {
    let distance = ((x - center).powi(2)
        + (y - center).powi(2))
    .sqrt();
    if (RIM_INNER..=RIM_OUTER).contains(&distance) {
        return true;
    }

    let minute_hand = distance_to_segment(
        x,
        y,
        center,
        center,
        center,
        center - 9.0,
    );
    let hour_hand = distance_to_segment(
        x,
        y,
        center,
        center,
        center + 5.0,
        center - 5.0,
    );
    minute_hand <= HAND_HALF_WIDTH
        || hour_hand <= HAND_HALF_WIDTH
}

/// 点到线段的距离。指针就是有粗细的线段,靠它决定像素在不在指针上。
fn distance_to_segment(
    x: f32,
    y: f32,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
) -> f32 {
    let (dx, dy) = (bx - ax, by - ay);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared == 0.0 {
        0.0
    } else {
        (((x - ax) * dx + (y - ay) * dy) / length_squared)
            .clamp(0.0, 1.0)
    };
    ((x - (ax + t * dx)).powi(2)
        + (y - (ay + t * dy)).powi(2))
    .sqrt()
}

/// 挂上托盘图标。没有 SNI host 时只报一声,闹钟照常响。
pub fn spawn(
    next_fire: String,
    open_settings: fn(),
) -> Option<ksni::blocking::Handle<AlarmTray>> {
    match (AlarmTray {
        next_fire,
        open_settings,
    })
    .spawn()
    {
        Ok(handle) => Some(handle),
        Err(error) => {
            eprintln!(
                "nap-alarm: 托盘挂不上({error}),闹钟照常响"
            );
            None
        }
    }
}

/// 菜单回调跑在 ksni 自己的线程上,退出得回到 Slint 的事件循环里做。
fn quit_daemon() {
    let _ = slint::invoke_from_event_loop(|| {
        let _ = slint::quit_event_loop();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(
        icon: &ksni::Icon,
        x: usize,
        y: usize,
    ) -> u8 {
        icon.data[(y * ICON_SIZE + x) * 4]
    }

    #[test]
    fn the_icon_is_a_full_argb_buffer_of_its_declared_size()
    {
        // SNI 按 width/height 读那块内存,尺寸对不上时 host 要么画花要么整个不画。
        let icon = clock_icon();

        assert_eq!(icon.width, ICON_SIZE as i32);
        assert_eq!(icon.height, ICON_SIZE as i32);
        assert_eq!(
            icon.data.len(),
            ICON_SIZE * ICON_SIZE * 4
        );
    }

    #[test]
    fn the_icon_is_transparent_outside_the_clock_face() {
        // 托盘背景色由 host 定,四角不透明会变成一个白方块糊在栏上。
        let icon = clock_icon();

        assert_eq!(alpha_at(&icon, 0, 0), 0);
        assert_eq!(alpha_at(&icon, ICON_SIZE - 1, 0), 0);
        assert_eq!(alpha_at(&icon, 0, ICON_SIZE - 1), 0);
        assert_eq!(
            alpha_at(&icon, ICON_SIZE - 1, ICON_SIZE - 1),
            0
        );
    }

    #[test]
    fn the_clock_rim_is_actually_drawn() {
        // 全透明的图标在托盘上就是一块空白,和没挂上分不出来。
        let icon = clock_icon();

        let painted = icon
            .data
            .chunks(4)
            .filter(|pixel| pixel[0] > 0)
            .count();
        assert!(
            painted > 100,
            "画出来的像素只有 {painted} 个,基本等于空白"
        );
    }
}
