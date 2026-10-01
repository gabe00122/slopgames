//! Keyboard, mouse and gamepad folded into one set of intents. Presses are
//! latched until the fixed-rate player systems consume them, so a tap is
//! never lost between physics steps.
//!
//! Keyboard: WASD / arrows move, Space jumps (in water: a burst), J or left
//! mouse claws, K or right mouse grabs and throws, Shift bursts.
//! Gamepad: left stick / d-pad move, A jumps and bursts, X claws, B grabs,
//! RT or RB bursts.

use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct Controls {
    pub mv: Vec2,
    pub jump_held: bool,
    pub jump: bool,
    pub attack: bool,
    pub grab: bool,
    pub dash: bool,
    /// The last input came from a gamepad (for button prompts).
    pub pad: bool,
}

impl Controls {
    pub fn clear_presses(&mut self) {
        self.jump = false;
        self.attack = false;
        self.grab = false;
        self.dash = false;
    }
}

fn deadzone(v: Vec2) -> Vec2 {
    let l = v.length();
    if l < 0.2 {
        Vec2::ZERO
    } else {
        v / l * ((l - 0.2) / 0.8).min(1.0)
    }
}

pub fn read(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    mut c: ResMut<Controls>,
) {
    let key = |k: KeyCode| keys.pressed(k);
    let mut mv = Vec2::ZERO;
    if key(KeyCode::KeyA) || key(KeyCode::ArrowLeft) {
        mv.x -= 1.0;
    }
    if key(KeyCode::KeyD) || key(KeyCode::ArrowRight) {
        mv.x += 1.0;
    }
    if key(KeyCode::KeyW) || key(KeyCode::ArrowUp) {
        mv.y += 1.0;
    }
    if key(KeyCode::KeyS) || key(KeyCode::ArrowDown) {
        mv.y -= 1.0;
    }
    let mut jump_held = key(KeyCode::Space);
    let mut jump = keys.just_pressed(KeyCode::Space);
    let mut attack = keys.just_pressed(KeyCode::KeyJ) || mouse.just_pressed(MouseButton::Left);
    let mut grab = keys.just_pressed(KeyCode::KeyK) || mouse.just_pressed(MouseButton::Right);
    let mut dash = keys.any_just_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight, KeyCode::KeyL]);
    if keys.get_just_pressed().next().is_some() || mouse.get_just_pressed().next().is_some() {
        c.pad = false;
    }
    if mv != Vec2::ZERO {
        mv = mv.normalize();
    }
    for pad in &pads {
        let mut s = deadzone(pad.left_stick());
        if pad.pressed(GamepadButton::DPadLeft) {
            s.x -= 1.0;
        }
        if pad.pressed(GamepadButton::DPadRight) {
            s.x += 1.0;
        }
        if pad.pressed(GamepadButton::DPadUp) {
            s.y += 1.0;
        }
        if pad.pressed(GamepadButton::DPadDown) {
            s.y -= 1.0;
        }
        if s.length() > 1.0 {
            s = s.normalize();
        }
        if s.length() > mv.length() {
            mv = s;
        }
        jump_held |= pad.pressed(GamepadButton::South);
        jump |= pad.just_pressed(GamepadButton::South);
        attack |= pad.just_pressed(GamepadButton::West);
        grab |= pad.just_pressed(GamepadButton::East);
        dash |= pad.any_just_pressed([GamepadButton::RightTrigger2, GamepadButton::RightTrigger]);
        if pad.get_just_pressed().next().is_some() || s != Vec2::ZERO {
            c.pad = true;
        }
    }
    c.mv = mv;
    c.jump_held = jump_held;
    c.jump |= jump;
    c.attack |= attack;
    c.grab |= grab;
    c.dash |= dash;
}
