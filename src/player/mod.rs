//! First-person player controller.

pub mod health;
pub mod physics;

use glam::{Mat4, Vec3};

use crate::world::World;
pub use health::{Body, BodyPart};

pub const HALF_WIDTH: f32 = 0.3;
pub const STAND_HEIGHT: f32 = 1.8;
pub const CROUCH_HEIGHT: f32 = 1.3;
pub const STAND_EYE: f32 = 1.62;
pub const CROUCH_EYE: f32 = 1.15;
const GRAVITY: f32 = 24.0;
const JUMP_VEL: f32 = 7.6;

/// Per-frame movement intent, produced from raw input by the game layer.
#[derive(Clone, Copy, Debug, Default)]
pub struct MoveInput {
    /// x = strafe right, y = forward.
    pub axis: glam::Vec2,
    pub jump: bool,
    pub crouch: bool,
    pub sprint: bool,
    /// Extra movement multiplier (ADS, reloading, overweight, ...).
    pub speed_mult: f32,
}

pub struct Player {
    /// Feet position.
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub crouching: bool,
    pub sprinting: bool,
    pub stamina: f32,
    stamina_locked: bool,
    eye_height: f32,
    /// Smooths the camera after stepping up ledges.
    step_offset: f32,
    pub body: Body,
    /// Distance walked (drives view bob).
    pub walk_phase: f32,
    pub fall_start_y: f32,
    pub noclip: bool,
    /// Fall damage taken this frame (to report to the HUD).
    pub last_fall_damage: f32,
}

impl Player {
    pub fn new(pos: Vec3, yaw: f32) -> Self {
        Self {
            pos,
            vel: Vec3::ZERO,
            yaw,
            pitch: 0.0,
            on_ground: false,
            crouching: false,
            sprinting: false,
            stamina: 100.0,
            stamina_locked: false,
            eye_height: STAND_EYE,
            step_offset: 0.0,
            body: Body::new(),
            walk_phase: 0.0,
            fall_start_y: pos.y,
            noclip: false,
            last_fall_damage: 0.0,
        }
    }

    pub fn height(&self) -> f32 {
        if self.crouching {
            CROUCH_HEIGHT
        } else {
            STAND_HEIGHT
        }
    }

    pub fn eye_pos(&self) -> Vec3 {
        self.pos + Vec3::Y * (self.eye_height - self.step_offset)
    }

    pub fn look_dir(&self) -> Vec3 {
        let cp = self.pitch.cos();
        Vec3::new(-self.yaw.sin() * cp, self.pitch.sin(), -self.yaw.cos() * cp)
    }

    pub fn forward_flat(&self) -> Vec3 {
        Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos())
    }

    pub fn right_flat(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    pub fn view_matrix(&self) -> Mat4 {
        crate::render::look_to(self.eye_pos(), self.look_dir(), Vec3::Y)
    }

    /// Apply mouse look (radians).
    pub fn look(&mut self, dyaw: f32, dpitch: f32) {
        self.yaw = (self.yaw + dyaw).rem_euclid(std::f32::consts::TAU);
        self.pitch = (self.pitch + dpitch).clamp(-1.54, 1.54);
    }

    pub fn horizontal_speed(&self) -> f32 {
        Vec3::new(self.vel.x, 0.0, self.vel.z).length()
    }

    pub fn update(&mut self, world: &World, input: &MoveInput, dt: f32) {
        self.last_fall_damage = 0.0;
        if self.noclip {
            let dir = self.look_dir() * input.axis.y + self.right_flat() * input.axis.x;
            let speed = if input.sprint { 30.0 } else { 12.0 };
            self.pos += dir * speed * dt;
            if input.jump {
                self.pos.y += speed * dt;
            }
            if input.crouch {
                self.pos.y -= speed * dt;
            }
            self.vel = Vec3::ZERO;
            return;
        }

        // Crouch (only stand up if there is headroom).
        if input.crouch {
            self.crouching = true;
        } else if self.crouching
            && !physics::collides(world, self.pos, HALF_WIDTH, STAND_HEIGHT)
        {
            self.crouching = false;
        }
        let target_eye = if self.crouching { CROUCH_EYE } else { STAND_EYE };
        self.eye_height += (target_eye - self.eye_height) * (1.0 - (-14.0 * dt).exp());
        self.step_offset *= (-12.0 * dt).exp();

        // Stamina & sprint.
        let moving_forward = input.axis.y > 0.3;
        let want_sprint = input.sprint && moving_forward && !self.crouching;
        if self.stamina <= 1.0 {
            self.stamina_locked = true;
        }
        if self.stamina_locked && self.stamina > 25.0 {
            self.stamina_locked = false;
        }
        self.sprinting = want_sprint && !self.stamina_locked && self.on_ground;
        if self.sprinting {
            self.stamina = (self.stamina - 11.0 * dt).max(0.0);
        } else {
            self.stamina = (self.stamina + 9.0 * dt).min(100.0);
        }

        let mut speed = if self.crouching {
            2.0
        } else if self.sprinting {
            6.4
        } else {
            4.2
        };
        speed *= input.speed_mult.max(0.1) * self.body.move_mult();

        let axis = if input.axis.length_squared() > 1.0 {
            input.axis.normalize()
        } else {
            input.axis
        };
        let wish = self.forward_flat() * axis.y + self.right_flat() * axis.x;
        let target = wish * speed;
        let accel = if self.on_ground { 14.0 } else { 2.5 };
        let k = 1.0 - (-accel * dt).exp();
        self.vel.x += (target.x - self.vel.x) * k;
        self.vel.z += (target.z - self.vel.z) * k;

        if input.jump && self.on_ground && self.stamina > 6.0 {
            self.vel.y = JUMP_VEL;
            self.stamina -= 6.0;
            self.on_ground = false;
        }
        self.vel.y = (self.vel.y - GRAVITY * dt).max(-50.0);

        let was_on_ground = self.on_ground;
        let height = self.height();
        let mut pos = self.pos;
        let mut vel = self.vel;
        let res = physics::move_body(world, &mut pos, &mut vel, HALF_WIDTH, height, dt, was_on_ground);
        self.pos = pos;
        self.vel = vel;
        self.on_ground = res.on_ground;
        if res.stepped > 0.0 {
            self.step_offset += res.stepped;
        }

        // Fall damage to legs.
        if was_on_ground || self.vel.y > 0.0 {
            self.fall_start_y = self.pos.y;
        } else {
            self.fall_start_y = self.fall_start_y.max(self.pos.y);
        }
        if self.on_ground && !was_on_ground {
            let fall = self.fall_start_y - self.pos.y;
            if fall > 4.5 {
                let dmg = (fall - 4.5) * 12.0;
                self.body.damage(BodyPart::LeftLeg, dmg * 0.5);
                self.body.damage(BodyPart::RightLeg, dmg * 0.5);
                self.last_fall_damage = dmg;
            }
            self.fall_start_y = self.pos.y;
        }

        if self.on_ground {
            self.walk_phase += self.horizontal_speed() * dt;
        }
        if self.pos.y < -20.0 {
            // Fell out of the world somehow: put back on top.
            let x = self.pos.x.floor() as i32;
            let z = self.pos.z.floor() as i32;
            self.pos.y = world.top_solid_y(x, z) as f32 + 1.0;
            self.vel = Vec3::ZERO;
        }
    }
}
