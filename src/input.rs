//! Raw keyboard/mouse state with per-frame edge detection.

use std::collections::HashSet;

use glam::Vec2;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

#[derive(Default)]
pub struct Input {
    down: HashSet<KeyCode>,
    pressed: HashSet<KeyCode>,
    released: HashSet<KeyCode>,
    mouse_down: [bool; 3],
    mouse_pressed: [bool; 3],
    mouse_released: [bool; 3],
    pub mouse_delta: Vec2,
    pub scroll: f32,
}

fn button_index(b: MouseButton) -> Option<usize> {
    match b {
        MouseButton::Left => Some(0),
        MouseButton::Right => Some(1),
        MouseButton::Middle => Some(2),
        _ => None,
    }
}

impl Input {
    pub fn key_event(&mut self, code: KeyCode, pressed: bool, repeat: bool) {
        if pressed {
            if !repeat && self.down.insert(code) {
                self.pressed.insert(code);
            }
        } else if self.down.remove(&code) {
            self.released.insert(code);
        }
    }

    pub fn mouse_button(&mut self, b: MouseButton, pressed: bool) {
        if let Some(i) = button_index(b) {
            if pressed && !self.mouse_down[i] {
                self.mouse_pressed[i] = true;
            }
            if !pressed && self.mouse_down[i] {
                self.mouse_released[i] = true;
            }
            self.mouse_down[i] = pressed;
        }
    }

    pub fn mouse_motion(&mut self, dx: f64, dy: f64) {
        self.mouse_delta += Vec2::new(dx as f32, dy as f32);
    }

    pub fn down(&self, k: KeyCode) -> bool {
        self.down.contains(&k)
    }

    pub fn pressed(&self, k: KeyCode) -> bool {
        self.pressed.contains(&k)
    }

    pub fn released(&self, k: KeyCode) -> bool {
        self.released.contains(&k)
    }

    pub fn lmb(&self) -> bool {
        self.mouse_down[0]
    }

    pub fn rmb(&self) -> bool {
        self.mouse_down[1]
    }

    pub fn lmb_pressed(&self) -> bool {
        self.mouse_pressed[0]
    }

    pub fn rmb_pressed(&self) -> bool {
        self.mouse_pressed[1]
    }

    pub fn mmb_pressed(&self) -> bool {
        self.mouse_pressed[2]
    }

    /// Clear per-frame edges and deltas.
    pub fn end_frame(&mut self) {
        self.pressed.clear();
        self.released.clear();
        self.mouse_pressed = [false; 3];
        self.mouse_released = [false; 3];
        self.mouse_delta = Vec2::ZERO;
        self.scroll = 0.0;
    }

    /// Forget all held state (focus loss, screen change).
    pub fn reset(&mut self) {
        self.down.clear();
        self.mouse_down = [false; 3];
        self.end_frame();
    }

    /// Forget held mouse buttons only (e.g. after a UI click started a raid).
    pub fn reset_mouse(&mut self) {
        self.mouse_down = [false; 3];
        self.mouse_pressed = [false; 3];
        self.mouse_released = [false; 3];
    }
}
