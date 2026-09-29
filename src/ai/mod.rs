//! Hostile AI ("scavs"): patrol, detect the player by line of sight, fight back.

pub mod nav;

use glam::Vec3;

use crate::player::physics;
use crate::player::Body;
use crate::rng::Rng;
use crate::weapons::armor::{ArmorKind, ArmorState};
use crate::weapons::ballistics::{crouch_scale, humanoid_hitboxes, Hitbox};
use crate::weapons::{AmmoType, AttachmentId, ReceiverId, Slot, Weapon};
use crate::world::World;

const HALF_W: f32 = 0.3;
const HEIGHT: f32 = 1.8;
const WALK_SPEED: f32 = 2.3;
const RUN_SPEED: f32 = 4.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScavState {
    Patrol,
    Investigate,
    Engage,
    Dead,
}

/// What the AI knows about the player this frame.
#[derive(Clone, Copy, Debug)]
pub struct PlayerInfo {
    pub feet: Vec3,
    pub eye: Vec3,
    pub chest: Vec3,
    pub crouching: bool,
    pub alive: bool,
    pub speed: f32,
}

/// A shot requested by an AI, resolved by the raid.
#[derive(Clone, Copy, Debug)]
pub struct ScavShot {
    pub scav: usize,
    pub origin: Vec3,
    pub dir: Vec3,
    pub ammo: AmmoType,
    pub loudness: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ScavLook {
    pub jacket: [u8; 4],
    pub pants: [u8; 4],
    pub skin: [u8; 4],
    pub hat: Option<[u8; 4]>,
}

pub struct Scav {
    pub id: usize,
    pub name: String,
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub aim_pitch: f32,
    pub on_ground: bool,
    pub crouch: f32,
    pub body: Body,
    pub helmet: Option<ArmorState>,
    pub armor: Option<ArmorState>,
    pub weapon: Weapon,
    pub state: ScavState,
    pub awareness: f32,
    pub sees_player: bool,
    pub last_known: Option<Vec3>,
    pub since_seen: f32,
    reaction: f32,
    fire_cooldown: f32,
    burst_left: u32,
    pub reload_timer: f32,
    path: Vec<Vec3>,
    path_i: usize,
    goal: Option<Vec3>,
    need_path: bool,
    wait: f32,
    sense_timer: f32,
    stuck_timer: f32,
    progress_pos: Vec3,
    strafe_dir: f32,
    strafe_timer: f32,
    repath_timer: f32,
    engage_time: f32,
    pub skill: f32,
    pub hit_flash: f32,
    pub muzzle_flash: f32,
    pub look: ScavLook,
    pub home: Vec3,
    pub death_time: f32,
    pub walk_phase: f32,
    /// Has the corpse been searched (loot is generated lazily by the raid).
    pub searched: bool,
}

const NAMES: [&str; 14] = [
    "Vitya", "Sanya", "Kolyan", "Borya", "Dimon", "Serega", "Tolyan", "Zhenya", "Grisha", "Lyokha", "Pashka",
    "Vovan", "Styopa", "Mishka",
];

fn pick_color(rng: &mut Rng, palette: &[[u8; 3]]) -> [u8; 4] {
    let c = rng.pick(palette);
    let j = |v: u8, rng: &mut Rng| (v as i32 + rng.range_i32(-12, 12)).clamp(0, 255) as u8;
    [j(c[0], rng), j(c[1], rng), j(c[2], rng), 255]
}

impl Scav {
    pub fn spawn(id: usize, pos: Vec3, rng: &mut Rng) -> Self {
        let receiver = match rng.weighted(&[15, 25, 35, 25]) {
            0 => ReceiverId::Grach,
            1 => ReceiverId::Vityaz,
            2 => ReceiverId::Ak74n,
            _ => ReceiverId::Akm,
        };
        let mut weapon = Weapon::new(receiver);
        let types = receiver.def().caliber.ammo_types();
        let ammo = match rng.weighted(&[60, 25, 15]) {
            0 => types[0],
            1 => types[1],
            _ => types[2],
        };
        if !receiver.def().pistol {
            if rng.chance(0.25) {
                weapon.set_attachment(Slot::Sight, Some(AttachmentId::CobraRedDot));
            }
            if rng.chance(0.15) {
                weapon.set_attachment(Slot::Grip, Some(AttachmentId::Rk2Grip));
            }
        }
        let cap = weapon.capacity();
        weapon = weapon.loaded_with(ammo, cap);

        let armor = if rng.chance(0.35) {
            Some(ArmorState::new(match rng.weighted(&[60, 30, 10]) {
                0 => ArmorKind::Paca,
                1 => ArmorKind::Zhuk3,
                _ => ArmorKind::Kora,
            }))
        } else {
            None
        };
        let helmet = if rng.chance(0.3) {
            Some(ArmorState::new(match rng.weighted(&[50, 40, 10]) {
                0 => ArmorKind::Ssh68,
                1 => ArmorKind::Kiver,
                _ => ArmorKind::Beanie6b47Lite,
            }))
        } else {
            None
        };
        let look = ScavLook {
            jacket: pick_color(rng, &[[40, 52, 92], [30, 30, 34], [74, 84, 52], [96, 96, 100], [110, 40, 36]]),
            pants: pick_color(rng, &[[34, 36, 60], [40, 40, 44], [66, 70, 46], [80, 66, 50]]),
            skin: pick_color(rng, &[[224, 180, 150], [196, 150, 118], [150, 110, 84]]),
            hat: if rng.chance(0.6) {
                Some(pick_color(rng, &[[30, 30, 30], [60, 70, 50], [120, 30, 30], [40, 40, 90]]))
            } else {
                None
            },
        };
        Self {
            id,
            name: format!("{} (Scav)", rng.pick(&NAMES)),
            pos,
            vel: Vec3::ZERO,
            yaw: rng.range_f32(0.0, std::f32::consts::TAU),
            aim_pitch: 0.0,
            on_ground: false,
            crouch: 0.0,
            body: Body::new(),
            helmet,
            armor,
            weapon,
            state: ScavState::Patrol,
            awareness: 0.0,
            sees_player: false,
            last_known: None,
            since_seen: 99.0,
            reaction: 0.0,
            fire_cooldown: 0.0,
            burst_left: 0,
            reload_timer: 0.0,
            path: Vec::new(),
            path_i: 0,
            goal: None,
            need_path: false,
            wait: rng.range_f32(0.0, 4.0),
            sense_timer: rng.range_f32(0.0, 0.2),
            stuck_timer: 0.0,
            progress_pos: pos,
            strafe_dir: 0.0,
            strafe_timer: 0.0,
            repath_timer: 0.0,
            engage_time: 0.0,
            skill: rng.range_f32(0.7, 1.2),
            hit_flash: 0.0,
            muzzle_flash: 0.0,
            look,
            home: pos,
            death_time: 0.0,
            walk_phase: 0.0,
            searched: false,
        }
    }

    pub fn alive(&self) -> bool {
        self.state != ScavState::Dead
    }

    pub fn eye(&self) -> Vec3 {
        self.pos + Vec3::Y * (1.62 * crouch_scale(self.crouch))
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos())
    }

    pub fn aim_dir(&self) -> Vec3 {
        let cp = self.aim_pitch.cos();
        Vec3::new(-self.yaw.sin() * cp, self.aim_pitch.sin(), -self.yaw.cos() * cp)
    }

    pub fn hitboxes(&self) -> [Hitbox; 7] {
        humanoid_hitboxes(self.pos, self.yaw, self.crouch)
    }

    fn enter_engage(&mut self, rng: &mut Rng, fast: bool) {
        if self.state != ScavState::Engage {
            self.state = ScavState::Engage;
            self.reaction = if fast {
                rng.range_f32(0.2, 0.4)
            } else {
                rng.range_f32(0.45, 1.0) / self.skill
            };
            self.engage_time = 0.0;
            self.path.clear();
            self.goal = None;
        }
    }

    /// A gunshot was heard at `pos` with the given audible radius.
    pub fn hear(&mut self, pos: Vec3, radius: f32, rng: &mut Rng) {
        if !self.alive() || self.state == ScavState::Engage {
            return;
        }
        if self.pos.distance(pos) < radius {
            self.state = ScavState::Investigate;
            let off = Vec3::new(rng.range_f32(-3.0, 3.0), 0.0, rng.range_f32(-3.0, 3.0));
            self.goal = Some(pos + off);
            self.need_path = true;
            self.awareness = self.awareness.max(0.55);
            self.wait = 0.0;
        }
    }

    /// Took a hit from something at `from`.
    pub fn on_hit(&mut self, from: Vec3, rng: &mut Rng) {
        if !self.alive() {
            return;
        }
        self.hit_flash = 0.15;
        self.awareness = 1.0;
        self.last_known = Some(from);
        self.enter_engage(rng, true);
        // Snap roughly towards the attacker.
        let d = from - self.pos;
        self.yaw = (-d.x).atan2(-d.z) + rng.range_f32(-0.3, 0.3);
    }

    pub fn kill(&mut self, time: f32) {
        self.state = ScavState::Dead;
        self.death_time = time;
        self.vel = Vec3::ZERO;
        self.path.clear();
    }

    fn perceive(&mut self, world: &World, player: &PlayerInfo, sense_dt: f32, rng: &mut Rng) {
        if !player.alive {
            self.sees_player = false;
            if self.state == ScavState::Engage {
                self.state = ScavState::Patrol;
                self.goal = None;
            }
            return;
        }
        let eye = self.eye();
        let to = player.eye - eye;
        let dist = to.length();
        let flat = Vec3::new(to.x, 0.0, to.z).normalize_or_zero();
        let angle = self.forward().dot(flat).clamp(-1.0, 1.0).acos().to_degrees();
        let engaged = self.state == ScavState::Engage;
        let fov = if engaged { 110.0 } else { 65.0 };
        let max_range = if engaged { 120.0 } else { 80.0 };
        let in_view = dist < max_range && (angle < fov || dist < 5.0);
        let visible =
            in_view && (world.line_of_sight(eye, player.eye) || world.line_of_sight(eye, player.chest));
        self.sees_player = visible;
        if visible {
            self.last_known = Some(player.feet);
            self.since_seen = 0.0;
            let mut rate = 0.5 + 22.0 / dist.max(1.0);
            if player.crouching {
                rate *= 0.55;
            }
            if player.speed > 5.0 {
                rate *= 1.4;
            }
            if self.state == ScavState::Investigate {
                rate *= 1.8;
            }
            self.awareness += rate * sense_dt;
            if self.awareness >= 1.0 {
                self.awareness = 1.0;
                self.enter_engage(rng, false);
            }
        } else {
            self.awareness = (self.awareness - 0.12 * sense_dt).max(0.0);
        }
    }

    fn set_goal(&mut self, goal: Vec3) {
        self.goal = Some(goal);
        self.need_path = true;
    }

    fn ensure_path(&mut self, world: &World) {
        if !self.need_path {
            return;
        }
        self.need_path = false;
        self.path.clear();
        self.path_i = 0;
        if let Some(goal) = self.goal {
            if let Some(p) = nav::find_path(world, self.pos, goal, 4000) {
                self.path = p;
            }
        }
    }

    /// Desired horizontal velocity to follow the current path.
    fn follow_path(&mut self, speed: f32) -> Vec3 {
        while self.path_i < self.path.len() {
            let t = self.path[self.path_i];
            let d = Vec3::new(t.x - self.pos.x, 0.0, t.z - self.pos.z);
            if d.length() < 0.35 {
                self.path_i += 1;
            } else {
                break;
            }
        }
        if self.path_i >= self.path.len() {
            return Vec3::ZERO;
        }
        let t = self.path[self.path_i];
        let d = Vec3::new(t.x - self.pos.x, 0.0, t.z - self.pos.z).normalize_or_zero();
        d * speed
    }

    fn path_done(&self) -> bool {
        self.path_i >= self.path.len()
    }

    fn turn_towards(&mut self, target_yaw: f32, rate: f32, dt: f32) {
        let mut diff = (target_yaw - self.yaw).rem_euclid(std::f32::consts::TAU);
        if diff > std::f32::consts::PI {
            diff -= std::f32::consts::TAU;
        }
        let step = rate * dt;
        self.yaw += diff.clamp(-step, step);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        dt: f32,
        world: &World,
        player: &PlayerInfo,
        patrol_points: &[Vec3],
        rng: &mut Rng,
        shots: &mut Vec<ScavShot>,
    ) {
        self.hit_flash = (self.hit_flash - dt).max(0.0);
        self.muzzle_flash = (self.muzzle_flash - dt).max(0.0);
        if !self.alive() {
            return;
        }

        self.sense_timer -= dt;
        if self.sense_timer <= 0.0 {
            let sdt = 0.15 + rng.f32() * 0.05;
            self.sense_timer = sdt;
            self.perceive(world, player, sdt, rng);
        }
        if !self.sees_player {
            self.since_seen += dt;
        }
        self.fire_cooldown -= dt;
        if self.reload_timer > 0.0 {
            self.reload_timer -= dt;
            if self.reload_timer <= 0.0 {
                self.weapon.rounds = self.weapon.capacity();
            }
        }

        let mut desired = Vec3::ZERO;
        let mut want_crouch = false;
        match self.state {
            ScavState::Patrol => {
                if self.goal.is_none() || self.path_done() {
                    self.wait -= dt;
                    if self.wait <= 0.0 && !patrol_points.is_empty() {
                        // Prefer points near home so scavs stay spread out.
                        let mut choice = None;
                        for _ in 0..8 {
                            let p = *rng.pick(patrol_points);
                            if p.distance(self.home) < 45.0 && p.distance(self.pos) > 4.0 {
                                choice = Some(p);
                                break;
                            }
                        }
                        let goal = choice.unwrap_or(self.home);
                        self.set_goal(goal);
                        self.wait = rng.range_f32(2.0, 7.0);
                    } else if self.wait > 0.0 {
                        // Idle: look around slowly.
                        self.yaw += (rng.f32() - 0.5) * dt * 1.5;
                    }
                }
                self.ensure_path(world);
                desired = self.follow_path(WALK_SPEED);
                if desired.length_squared() > 0.01 {
                    let ty = (-desired.x).atan2(-desired.z);
                    self.turn_towards(ty, 4.0, dt);
                }
                self.aim_pitch *= 1.0 - dt * 3.0;
            }
            ScavState::Investigate => {
                self.ensure_path(world);
                desired = self.follow_path(RUN_SPEED * 0.8);
                if desired.length_squared() > 0.01 {
                    let ty = (-desired.x).atan2(-desired.z);
                    self.turn_towards(ty, 5.0, dt);
                } else {
                    // Arrived (or no path): search around, then give up.
                    self.yaw += dt * 1.2;
                    self.wait -= dt;
                    if self.wait <= -5.0 {
                        self.state = ScavState::Patrol;
                        self.goal = None;
                        self.wait = rng.range_f32(1.0, 3.0);
                    }
                }
            }
            ScavState::Engage => {
                self.engage_time += dt;
                let target_pos = if self.sees_player {
                    Some(player.chest)
                } else {
                    self.last_known.map(|p| p + Vec3::Y * 1.3)
                };
                if let Some(tp) = target_pos {
                    let d = tp - self.eye();
                    let ty = (-d.x).atan2(-d.z);
                    self.turn_towards(ty, 5.0 * self.skill, dt);
                    let tp_pitch = (d.y / Vec3::new(d.x, 0.0, d.z).length().max(0.01)).atan();
                    self.aim_pitch += (tp_pitch - self.aim_pitch) * (1.0 - (-8.0 * dt).exp());
                }
                if self.sees_player {
                    let dist = player.eye.distance(self.eye());
                    // Movement: close in from far away, otherwise strafe.
                    self.repath_timer -= dt;
                    if dist > 32.0 {
                        if self.repath_timer <= 0.0 {
                            self.set_goal(player.feet);
                            self.repath_timer = 1.5;
                        }
                        self.ensure_path(world);
                        desired = self.follow_path(RUN_SPEED * 0.75);
                    } else {
                        self.strafe_timer -= dt;
                        if self.strafe_timer <= 0.0 {
                            self.strafe_timer = rng.range_f32(0.7, 1.8);
                            self.strafe_dir = *rng.pick(&[-1.0, 0.0, 0.0, 1.0]);
                            self.crouch = if rng.chance(0.25) { 1.0 } else { 0.0 };
                        }
                        let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
                        desired = right * self.strafe_dir * 1.7;
                        want_crouch = self.crouch > 0.5;
                    }
                    // Shooting.
                    self.reaction -= dt;
                    let aim_err = {
                        let want = (player.chest - self.eye()).normalize_or_zero();
                        self.aim_dir().angle_between(want).to_degrees()
                    };
                    if self.reaction <= 0.0 && self.reload_timer <= 0.0 && aim_err < 12.0 && self.fire_cooldown <= 0.0 {
                        self.fire(dist, player, rng, shots, desired.length() > 0.5);
                    }
                } else {
                    // Lost sight: push to the last known position.
                    if self.since_seen > 1.5 {
                        if let Some(lk) = self.last_known {
                            self.state = ScavState::Investigate;
                            self.set_goal(lk);
                            self.wait = 0.0;
                        } else {
                            self.state = ScavState::Patrol;
                        }
                    }
                }
            }
            ScavState::Dead => {}
        }
        if !want_crouch && self.state != ScavState::Engage {
            self.crouch = 0.0;
        }

        // Physics.
        let accel = if self.on_ground { 10.0 } else { 2.0 };
        let k = 1.0 - (-accel * dt).exp();
        self.vel.x += (desired.x - self.vel.x) * k;
        self.vel.z += (desired.z - self.vel.z) * k;
        self.vel.y = (self.vel.y - 24.0 * dt).max(-50.0);
        let height = if self.crouch > 0.5 { 1.3 } else { HEIGHT };
        let res = physics::move_body(world, &mut self.pos, &mut self.vel, HALF_W, height, dt, self.on_ground);
        self.on_ground = res.on_ground;
        if res.hit_wall && self.on_ground && desired.length() > 0.5 {
            // Blocked by something two high: hop in case it's a gap we can clear.
            self.vel.y = 7.0;
        }
        self.walk_phase += Vec3::new(self.vel.x, 0.0, self.vel.z).length() * dt;

        // Stuck detection.
        if desired.length() > 0.5 {
            self.stuck_timer += dt;
            if self.stuck_timer > 1.5 {
                if self.pos.distance(self.progress_pos) < 0.5 {
                    // Give up on this goal.
                    self.path.clear();
                    self.goal = None;
                    self.wait = 0.5;
                    if self.state == ScavState::Investigate {
                        self.state = ScavState::Patrol;
                    }
                }
                self.stuck_timer = 0.0;
                self.progress_pos = self.pos;
            }
        } else {
            self.stuck_timer = 0.0;
            self.progress_pos = self.pos;
        }
    }

    fn fire(&mut self, dist: f32, player: &PlayerInfo, rng: &mut Rng, shots: &mut Vec<ScavShot>, moving: bool) {
        if self.weapon.rounds == 0 {
            self.reload_timer = self.weapon.stats().reload_time * 1.3;
            return;
        }
        let stats = self.weapon.stats();
        if self.burst_left == 0 {
            self.burst_left = if stats.full_auto && self.weapon.auto_mode {
                rng.range_i32(2, 6) as u32
            } else {
                1
            };
        }
        let ammo = self.weapon.loaded.unwrap_or(AmmoType::Ps545);
        // Aim: chest, occasionally head; error shrinks the longer the fight lasts.
        let target = if rng.chance(0.08) {
            player.eye
        } else {
            player.chest + Vec3::Y * rng.range_f32(-0.35, 0.2)
        };
        // Early shots in an engagement are wild; aim settles over ~6 seconds.
        let settle = (1.9 - (self.engage_time / 6.0).min(1.1)).max(0.8);
        let mut err_deg = 2.8 / self.skill * settle + stats.spread;
        if moving {
            err_deg *= 1.5;
        }
        if player.crouching {
            err_deg *= 1.1;
        }
        err_deg += dist * 0.012;
        let origin = self.eye() + self.forward() * 0.3;
        let dir = crate::weapons::ballistics::apply_spread((target - origin).normalize(), err_deg, rng);
        shots.push(ScavShot {
            scav: self.id,
            origin,
            dir,
            ammo,
            loudness: stats.loudness,
        });
        self.muzzle_flash = 0.05;
        self.weapon.rounds -= 1;
        self.burst_left -= 1;
        if self.burst_left == 0 {
            self.fire_cooldown = rng.range_f32(0.35, 1.1);
        } else {
            self.fire_cooldown = 60.0 / stats.fire_rate;
        }
        if self.weapon.rounds == 0 {
            self.reload_timer = stats.reload_time * 1.3;
            self.burst_left = 0;
        }
    }
}
