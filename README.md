# nap-alarm

A small alarm clock that only rings into a connected bluetooth headset.

Every alarm on this machine had the same flaw: it rings out loud. An alarm for
a nap in a shared room has to reach exactly one person, and the headset is the
only output that does. So the condition is part of the alarm, not a setting
buried somewhere: when the headset is not connected, the alarm does not ring at
all — a nap alarm nobody is wearing headphones for cannot wake anyone anyway,
and out loud it only wakes everyone else.

"Connected" means a `bluez_output.*` node exists in PipeWire, not that
`bluetoothctl` lists a device. That node is the proof the headset is paired,
routed and ready to take audio; a bluetooth mouse would pass the device list.
Ringing then goes to that node through `pw-play --target`, so it lands in the
headset rather than whatever output happens to be default.

The alarm window is the stop button. Clicking anywhere in it ends the ringing —
a person who has just woken up should not have to find a small target.

## Running it

```
nap-alarm daemon   # the scheduler, meant for a systemd user service
nap-alarm          # the settings window
```

The daemon polls the wall clock every twenty seconds rather than sleeping until
the next alarm. A long sleep wakes at the wrong time after a suspend or a clock
change; reading the current time cannot. A minute slept through never fires
late, and an alarm fires once in its minute however often it is polled.

## The config file

`~/.config/nap-alarm/alarms.toml`. The settings window writes it on every edit,
so there is no save button, and it is plain enough to edit by hand:

```toml
sound = "/path/to/ring.ogg"

[[alarm]]
label = "午休结束"
time = "13:30"
days = ["mon", "tue", "wed", "thu", "fri"]
enabled = true
require_headset = true
```

Times and weekdays are parsed while the file is read, so a typo fails on the
spot with the offending value in the message rather than staying quiet until
the morning it was supposed to ring.

## Building

Every cargo command runs inside the dev shell: Slint needs `pkg-config` to find
fontconfig at build time and dlopens wayland, libxkbcommon and libGL at run
time.

```
just check   # fmt, clippy, tests
just run     # the settings window
```
