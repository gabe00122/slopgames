//! Gamepads and keyboards. Every connected gamepad is a device, and the
//! keyboard provides two more (WASD and the arrow keys), so four people can
//! play with two pads, or one person can test split-screen alone.

use bevy::{
    input::gamepad::{GamepadRumbleIntensity, GamepadRumbleRequest},
    platform::collections::HashMap,
    prelude::*,
};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Device {
    Pad(Entity),
    /// W A S D, Space, E.
    KeysA,
    /// Arrow keys, Right Shift, Enter.
    KeysB,
}

impl Device {
    pub fn label(&self) -> &'static str {
        match self {
            Device::Pad(_) => "GAMEPAD",
            Device::KeysA => "KEYS: WASD",
            Device::KeysB => "KEYS: ARROWS",
        }
    }
}

/// Driving controls for one frame.
#[derive(Clone, Copy, Default, Debug)]
pub struct Drive {
    /// -1 (left) to 1 (right).
    pub steer: f32,
    pub gas: f32,
    pub brake: f32,
    pub drift: bool,
    /// The item button went down this frame.
    pub item: bool,
}

/// Menu controls for one frame (edges, not levels).
#[derive(Clone, Copy, Default, Debug)]
pub struct Nav {
    pub dx: i32,
    pub dy: i32,
    pub confirm: bool,
    pub back: bool,
    pub pause: bool,
}

impl Nav {
    pub fn any(&self) -> bool {
        self.dx != 0 || self.dy != 0 || self.confirm || self.back || self.pause
    }
}

#[derive(Resource, Default)]
pub struct Inputs {
    pub nav: Vec<(Device, Nav)>,
    pub drive: HashMap<Device, Drive>,
    /// Last digital stick direction and time until it repeats, per device.
    held: HashMap<Device, (i32, i32, f32)>,
}

impl Inputs {
    pub fn drive(&self, device: Device) -> Drive {
        self.drive.get(&device).copied().unwrap_or_default()
    }

    pub fn nav(&self, device: Device) -> Nav {
        self.nav
            .iter()
            .find(|(d, _)| *d == device)
            .map(|(_, n)| *n)
            .unwrap_or_default()
    }

    /// Menu input merged across every device.
    pub fn any_nav(&self) -> Nav {
        let mut out = Nav::default();
        for (_, n) in &self.nav {
            if out.dx == 0 {
                out.dx = n.dx;
            }
            if out.dy == 0 {
                out.dy = n.dy;
            }
            out.confirm |= n.confirm;
            out.back |= n.back;
            out.pause |= n.pause;
        }
        out
    }
}

fn curve(x: f32) -> f32 {
    const DEAD: f32 = 0.14;
    if x.abs() < DEAD {
        return 0.0;
    }
    let t = (x.abs() - DEAD) / (1.0 - DEAD);
    x.signum() * t.powf(1.35)
}

pub fn gather(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<(Entity, &Gamepad)>,
    mut inputs: ResMut<Inputs>,
) {
    let dt = time.delta_secs();
    let mut raw: Vec<(Device, Drive, (i32, i32), Nav)> = Vec::new();

    for (entity, pad) in &pads {
        let stick = pad.left_stick();
        let dpad = pad.dpad();
        let steer = (curve(stick.x) + dpad.x).clamp(-1.0, 1.0);
        let trigger = |b: GamepadButton| pad.get(b).unwrap_or(0.0).max(if pad.pressed(b) { 1.0 } else { 0.0 });
        let drive = Drive {
            steer,
            gas: trigger(GamepadButton::RightTrigger2).max(pad.pressed(GamepadButton::South) as u8 as f32),
            brake: trigger(GamepadButton::LeftTrigger2).max(pad.pressed(GamepadButton::East) as u8 as f32),
            drift: pad.any_pressed([GamepadButton::RightTrigger, GamepadButton::West]),
            item: pad.any_just_pressed([GamepadButton::LeftTrigger, GamepadButton::North]),
        };
        let dir = (
            (stick.x + dpad.x > 0.55) as i32 - (stick.x + dpad.x < -0.55) as i32,
            (stick.y + dpad.y < -0.55) as i32 - (stick.y + dpad.y > 0.55) as i32,
        );
        let nav = Nav {
            confirm: pad.just_pressed(GamepadButton::South),
            back: pad.just_pressed(GamepadButton::East),
            pause: pad.just_pressed(GamepadButton::Start),
            ..default()
        };
        raw.push((Device::Pad(entity), drive, dir, nav));
    }

    let axis = |neg: KeyCode, pos: KeyCode| keys.pressed(pos) as i32 - keys.pressed(neg) as i32;
    {
        let dir = (axis(KeyCode::KeyA, KeyCode::KeyD), axis(KeyCode::KeyW, KeyCode::KeyS));
        let drive = Drive {
            steer: dir.0 as f32,
            gas: keys.pressed(KeyCode::KeyW) as u8 as f32,
            brake: keys.pressed(KeyCode::KeyS) as u8 as f32,
            drift: keys.any_pressed([KeyCode::Space, KeyCode::ShiftLeft]),
            item: keys.any_just_pressed([KeyCode::KeyE, KeyCode::KeyQ]),
        };
        let nav = Nav {
            confirm: keys.any_just_pressed([KeyCode::Space, KeyCode::KeyE]),
            back: keys.any_just_pressed([KeyCode::KeyQ, KeyCode::Escape]),
            pause: keys.just_pressed(KeyCode::Escape),
            ..default()
        };
        raw.push((Device::KeysA, drive, dir, nav));
    }
    {
        let dir = (
            axis(KeyCode::ArrowLeft, KeyCode::ArrowRight),
            axis(KeyCode::ArrowUp, KeyCode::ArrowDown),
        );
        let drive = Drive {
            steer: dir.0 as f32,
            gas: keys.pressed(KeyCode::ArrowUp) as u8 as f32,
            brake: keys.pressed(KeyCode::ArrowDown) as u8 as f32,
            drift: keys.any_pressed([KeyCode::ShiftRight, KeyCode::ControlRight]),
            item: keys.any_just_pressed([KeyCode::Enter, KeyCode::Slash]),
        };
        let nav = Nav {
            confirm: keys.just_pressed(KeyCode::Enter),
            back: keys.just_pressed(KeyCode::Backspace),
            pause: keys.just_pressed(KeyCode::KeyP),
            ..default()
        };
        raw.push((Device::KeysB, drive, dir, nav));
    }

    let inputs = &mut *inputs;
    inputs.nav.clear();
    inputs.drive.clear();
    for (device, drive, dir, mut nav) in raw {
        // Direction edges, with auto-repeat while held.
        let held = inputs.held.entry(device).or_insert((0, 0, 0.0));
        if dir != (held.0, held.1) {
            *held = (dir.0, dir.1, 0.4);
            nav.dx = dir.0;
            nav.dy = dir.1;
        } else if dir != (0, 0) {
            held.2 -= dt;
            if held.2 <= 0.0 {
                held.2 = 0.14;
                nav.dx = dir.0;
                nav.dy = dir.1;
            }
        }
        inputs.nav.push((device, nav));
        inputs.drive.insert(device, drive);
    }
    let alive: Vec<Device> = inputs.nav.iter().map(|(d, _)| *d).collect();
    inputs.held.retain(|d, _| alive.contains(d));
}

/// Hands a connected gamepad to any player whose own has gone: one that was
/// unplugged and came back as a new device, or a quick-start seat that was
/// never given one. Quick-start seats fall back to the keyboard.
pub fn reattach(
    time: Res<Time<Real>>,
    args: Res<crate::Args>,
    pads: Query<Entity, With<Gamepad>>,
    mut roster: ResMut<crate::game::Roster>,
) {
    for slot in 0..roster.players.len() {
        let Device::Pad(pad) = roster.players[slot].device else {
            continue;
        };
        if pad != Entity::PLACEHOLDER && pads.contains(pad) {
            continue;
        }
        let taken = |d: Device| roster.players.iter().any(|p| p.device == d);
        let spare_pad = pads.iter().map(Device::Pad).find(|d| !taken(*d));
        // Give gamepads a moment to announce themselves before settling for keys.
        let keys = (args.race.is_some() && time.elapsed_secs() > 0.6)
            .then(|| [Device::KeysA, Device::KeysB].into_iter().find(|d| !taken(*d)))
            .flatten();
        // With nothing to give, mark the seat empty so nothing keeps addressing the lost gamepad.
        let device = spare_pad.or(keys).unwrap_or(Device::Pad(Entity::PLACEHOLDER));
        if roster.players[slot].device != device {
            roster.players[slot].device = device;
        }
    }
}

/// Shakes a player's controller, if they have one.
pub fn rumble(writer: &mut MessageWriter<GamepadRumbleRequest>, device: Device, strong: f32, weak: f32, secs: f32) {
    if let Device::Pad(gamepad) = device
        && gamepad != Entity::PLACEHOLDER
    {
        writer.write(GamepadRumbleRequest::Add {
            gamepad,
            intensity: GamepadRumbleIntensity {
                strong_motor: strong.clamp(0.0, 1.0),
                weak_motor: weak.clamp(0.0, 1.0),
            },
            duration: Duration::from_secs_f32(secs),
        });
    }
}
