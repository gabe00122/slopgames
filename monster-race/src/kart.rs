//! Karts: the model, the driving physics and the CPU drivers.
//!
//! A kart's state is `(u, d)` on the track ribbon plus a heading relative to
//! the track direction. Because the ribbon is re-posed from the beast's
//! skeleton every frame, karts are carried along when the beast moves, slide
//! when it tilts, and get thrown when the ground drops away.

use crate::{
    audio::{Sfx, Sound},
    beast::Ground,
    course::{Course, Mats},
    fauna::Scuttler,
    fx::{Fx, Puff},
    game::{CLASSES, COLORS, Roster, Settings},
    input::{Device, Drive, Inputs, rumble},
    items::Item,
    meshkit::MeshBuilder,
    race::{Phase, Race},
    track::{Edge, Track},
    util::{damp, lerp, lin, shade, wrap_angle},
};
use bevy::{
    camera::visibility::RenderLayers, input::gamepad::GamepadRumbleRequest, light::NotShadowCaster, prelude::*,
};
use std::f32::consts::TAU;

pub const GRAVITY: f32 = 38.0;
/// While grounded, karts follow the road down as if pulled this hard, so
/// only real drop-offs send them airborne.
const STICK: f32 = 95.0;
const HOP: f32 = 6.2;
pub const VMAX: f32 = 33.0;
const RESPAWN: f32 = 2.2;
pub const KART_RADIUS: f32 = 1.45;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Driver {
    /// Index into the roster.
    Human(usize),
    Cpu,
}

#[derive(Clone, Default)]
pub struct Ai {
    /// Preferred lane, -1..1 across the road.
    pub lane: f32,
    pub lane_timer: f32,
    /// Top-speed multiplier.
    pub skill: f32,
    pub item_wait: f32,
    pub seed: f32,
    /// Rubber-band multiplier set by the race each frame.
    pub band: f32,
    /// How long the CPU has been sliding through the current bend, and which way.
    pub bend_time: f32,
    pub bend_dir: f32,
}

#[derive(Component)]
pub struct Kart {
    pub index: usize,
    pub driver: Driver,
    pub color: usize,
    pub name: String,

    // Position on the track.
    pub u: f32,
    pub d: f32,
    /// Heading relative to the track direction; positive is to the right.
    pub yaw: f32,
    pub speed: f32,
    /// Sideways slide (banked road, shoves).
    pub lat: f32,

    // Vertical motion, measured against the ground directly below. Heights
    // and speeds are relative to the road, so a jump is the same jump whether
    // the beast is holding still or heaving the road upward.
    pub h: f32,
    pub vy: f32,
    pub grounded: bool,
    pub ground_y: f32,
    pub air_time: f32,
    pub from_ramp: bool,
    /// Upward kick to apply on the next step (hops, being tossed).
    pub launch: f32,
    pub gap_h0: f32,
    pub in_gap: bool,

    pub fallen: bool,
    pub fall_t: f32,
    pub splashed: bool,
    pub respawn: f32,

    pub drift: i8,
    pub drift_charge: f32,
    pub drift_held: bool,
    pub boost: f32,
    pub spin: f32,
    pub tumble: bool,
    pub star: f32,
    pub invuln: f32,

    pub lap: i32,
    pub place: usize,
    pub finished: Option<f32>,
    pub wrong_way: f32,
    /// Seconds to show the lap announcement.
    pub lap_flash: f32,

    pub item: Option<Item>,
    pub item_uses: u8,
    pub roulette: f32,

    pub input: Drive,
    pub ai: Ai,
    /// How long the gas has been held during the countdown.
    pub gas_held: f32,

    // World-space results of the last step.
    pub pos: Vec3,
    pub fwd: Vec3,
    pub up: Vec3,
    pub steer_vis: f32,
    /// Depth of water over the road here.
    pub depth: f32,
    fx_timer: f32,
}

impl Kart {
    pub fn new(index: usize, driver: Driver, color: usize, name: String, u: f32, d: f32) -> Self {
        Kart {
            index,
            driver,
            color,
            name,
            u,
            d,
            yaw: 0.0,
            speed: 0.0,
            lat: 0.0,
            h: 0.0,
            vy: 0.0,
            grounded: true,
            ground_y: 0.0,
            air_time: 0.0,
            from_ramp: false,
            launch: 0.0,
            gap_h0: 0.0,
            in_gap: false,
            fallen: false,
            fall_t: 0.0,
            splashed: false,
            respawn: 0.0,
            drift: 0,
            drift_charge: 0.0,
            drift_held: false,
            boost: 0.0,
            spin: 0.0,
            tumble: false,
            star: 0.0,
            invuln: 0.0,
            lap: 0,
            place: index + 1,
            finished: None,
            wrong_way: 0.0,
            lap_flash: 0.0,
            item: None,
            item_uses: 0,
            roulette: 0.0,
            input: Drive::default(),
            ai: Ai {
                skill: 1.0,
                band: 1.0,
                seed: index as f32 * 1.37,
                ..default()
            },
            gas_held: 0.0,
            pos: Vec3::ZERO,
            fwd: Vec3::Z,
            up: Vec3::Y,
            steer_vis: 0.0,
            depth: 0.0,
            fx_timer: 0.0,
        }
    }

    pub fn is_human(&self) -> bool {
        matches!(self.driver, Driver::Human(_))
    }

    /// Mini-turbo level reached by the current drift: 0..=3.
    pub fn drift_level(&self) -> u8 {
        match self.drift_charge {
            c if c >= 3.4 => 3,
            c if c >= 2.0 => 2,
            c if c >= 0.9 => 1,
            _ => 0,
        }
    }

    /// Which way the kart is sliding, for its pose and sparks: -1, 0 or 1.
    /// Players slide by drifting; CPUs are shown sliding through tight bends.
    pub fn slide(&self) -> f32 {
        if self.drift != 0 {
            self.drift as f32
        } else if self.ai.bend_time > 0.3 {
            self.ai.bend_dir
        } else {
            0.0
        }
    }

    pub fn give_boost(&mut self, secs: f32) {
        self.boost = self.boost.max(secs);
    }

    /// Knocks the kart out of control. Returns false if it shrugged it off.
    pub fn hit(&mut self, tumble: bool) -> bool {
        if self.star > 0.0 || self.invuln > 0.0 || self.respawn > 0.0 || self.fallen {
            return false;
        }
        self.spin = if tumble { 1.5 } else { 1.1 };
        self.tumble = tumble;
        if tumble {
            self.launch = 12.0;
        }
        self.drift = 0;
        self.drift_charge = 0.0;
        self.boost = 0.0;
        self.invuln = self.spin + 1.3;
        true
    }

    /// Total distance covered, in samples, for ranking.
    pub fn progress(&self, track: &Track) -> f32 {
        self.lap as f32 * track.n() + self.u
    }

    pub fn device(&self, roster: &Roster) -> Option<Device> {
        match self.driver {
            Driver::Human(slot) => roster.players.get(slot).map(|p| p.device),
            Driver::Cpu => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct BoostFlame;
#[derive(Component)]
pub struct StarAura;
#[derive(Component)]
pub struct RescueBird;
#[derive(Component)]
pub struct KartBody;
/// The marker floating over a player's kart, seen in everyone else's view.
#[derive(Component)]
pub struct PlayerMarker;

/// Render layer for player `slot`'s marker. Each view leaves out its own player's layer.
pub fn marker_layer(slot: usize) -> usize {
    1 + slot
}

fn kart_mesh(color: u32) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let body = lin(color);
    let dark = lin(0x26262e);
    let metal = lin(0x9aa3ad);
    // Forward is -Z.
    let center = Vec3::new(0.0, 0.45, 0.0);
    b.boxy(Vec3::new(0.0, 0.42, 0.1), Vec3::new(0.62, 0.17, 1.1), body);
    // Tapered nose.
    let (z0, z1) = (-1.0, -1.85);
    let r0 = [(-0.62, 0.25), (0.62, 0.25), (0.62, 0.59), (-0.62, 0.59)];
    let r1 = [(-0.3, 0.27), (0.3, 0.27), (0.3, 0.42), (-0.3, 0.42)];
    for i in 0..4 {
        let j = (i + 1) % 4;
        b.quad_facing(
            [
                Vec3::new(r0[i].0, r0[i].1, z0),
                Vec3::new(r0[j].0, r0[j].1, z0),
                Vec3::new(r1[j].0, r1[j].1, z1),
                Vec3::new(r1[i].0, r1[i].1, z1),
            ],
            shade(body, if i == 2 { 1.1 } else { 0.9 }),
            center,
        );
    }
    b.quad_facing(
        [
            Vec3::new(r1[0].0, r1[0].1, z1),
            Vec3::new(r1[1].0, r1[1].1, z1),
            Vec3::new(r1[2].0, r1[2].1, z1),
            Vec3::new(r1[3].0, r1[3].1, z1),
        ],
        lin(0xf4f4f0),
        center,
    );
    // Seat, engine, exhausts, spoiler.
    b.boxy(Vec3::new(0.0, 0.85, 0.62), Vec3::new(0.42, 0.34, 0.1), dark);
    b.boxy(Vec3::new(0.0, 0.68, 0.98), Vec3::new(0.44, 0.2, 0.24), metal);
    for side in [-1.0f32, 1.0] {
        b.cyl(
            Vec3::new(side * 0.26, 0.74, 1.1),
            Vec3::new(side * 0.32, 1.0, 1.55),
            0.1,
            0.12,
            6,
            dark,
            true,
        );
        b.boxy(Vec3::new(side * 0.55, 0.98, 1.22), Vec3::new(0.05, 0.22, 0.12), dark);
    }
    b.boxy(Vec3::new(0.0, 1.22, 1.28), Vec3::new(0.8, 0.045, 0.22), body);
    // Wheels.
    for (x, y, z, r, w) in [(0.74, 0.34, -0.95, 0.34, 0.3), (0.8, 0.42, 0.78, 0.42, 0.4)] {
        for side in [-1.0f32, 1.0] {
            let c = Vec3::new(side * x, y, z);
            let half = Vec3::X * w * 0.5;
            b.cyl(c - half, c + half, r, r, 9, dark, true);
            b.cyl(
                c + half * side * 0.9,
                c + half * side * 1.12,
                r * 0.5,
                r * 0.4,
                6,
                lin(0xe9e4d8),
                true,
            );
        }
    }
    // Driver: suit, head, helmet and visor.
    b.ico(Vec3::new(0.0, 0.98, 0.2), Vec3::new(0.3, 0.36, 0.26), 1, lin(0xf6f1e6));
    b.ico_with(Vec3::new(0.0, 1.52, 0.16), Vec3::splat(0.32), 2, |d| {
        let c = if d.z < -0.35 && d.y.abs() < 0.32 {
            lin(0x1c1c24)
        } else if d.y > -0.25 {
            body
        } else {
            lin(0xffd9b3)
        };
        (1.0, c)
    });
    b.cone(
        Vec3::new(0.0, 1.8, 0.16),
        Vec3::new(0.0, 2.0, 0.34),
        0.1,
        5,
        lin(0xf4f4f0),
    );
    // Arms and steering wheel.
    for side in [-1.0f32, 1.0] {
        b.cyl(
            Vec3::new(side * 0.27, 1.12, 0.12),
            Vec3::new(side * 0.16, 0.92, -0.38),
            0.085,
            0.07,
            5,
            lin(0xf6f1e6),
            false,
        );
    }
    b.cyl(
        Vec3::new(0.0, 0.86, -0.36),
        Vec3::new(0.0, 0.96, -0.46),
        0.22,
        0.22,
        8,
        dark,
        true,
    );
    b
}

/// Meshes shared by every kart's attachments.
#[derive(Resource)]
pub struct KartAssets {
    pub bodies: Vec<Handle<Mesh>>,
    pub flame: Handle<Mesh>,
    pub aura: Handle<Mesh>,
    pub bird: Handle<Mesh>,
    pub markers: Vec<Handle<Mesh>>,
}

pub fn setup_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let bodies = COLORS
        .iter()
        .map(|&(_, c)| meshes.add(kart_mesh(c).build(true, false)))
        .collect();
    let mut flame = MeshBuilder::new();
    for side in [-1.0f32, 1.0] {
        flame.cone(
            Vec3::new(side * 0.32, 1.0, 1.55),
            Vec3::new(side * 0.4, 1.25, 3.4),
            0.28,
            6,
            lin(0xffb347),
        );
        flame.cone(
            Vec3::new(side * 0.32, 1.0, 1.55),
            Vec3::new(side * 0.38, 1.15, 2.6),
            0.17,
            6,
            lin(0x9fe8ff),
        );
    }
    let mut aura = MeshBuilder::new();
    aura.ico_with(Vec3::new(0.0, 0.9, 0.0), Vec3::new(1.9, 1.5, 2.4), 1, |d| {
        (
            1.0,
            if (d.y * 6.0).sin() > 0.0 {
                lin(0xfff06a)
            } else {
                lin(0xffb703)
            },
        )
    });
    let mut bird = MeshBuilder::new();
    bird.ico(Vec3::new(0.0, 4.4, 0.0), Vec3::new(0.7, 0.6, 1.5), 1, lin(0xf8f8f4));
    bird.cone(
        Vec3::new(0.0, 4.5, -1.3),
        Vec3::new(0.0, 4.4, -2.2),
        0.25,
        4,
        lin(0xf2a23a),
    );
    for side in [-1.0f32, 1.0] {
        bird.tri_2(
            [
                Vec3::new(0.0, 4.6, -0.6),
                Vec3::new(0.0, 4.6, 0.8),
                Vec3::new(side * 4.2, 5.6, 0.3),
            ],
            lin(0xe6ebef),
        );
        bird.cyl(
            Vec3::new(side * 0.3, 4.0, 0.0),
            Vec3::new(side * 0.5, 2.3, 0.2),
            0.06,
            0.06,
            4,
            lin(0xf2a23a),
            false,
        );
    }
    // An upside-down pyramid in the kart's color, with a white rim.
    let markers = COLORS
        .iter()
        .map(|&(_, c)| {
            let mut m = MeshBuilder::new();
            let tip = Vec3::new(0.0, -0.7, 0.0);
            let corners: Vec<Vec3> = (0..4)
                .map(|k| {
                    let a = k as f32 * std::f32::consts::FRAC_PI_2;
                    Vec3::new(a.cos() * 0.62, 0.3, a.sin() * 0.62)
                })
                .collect();
            for k in 0..4 {
                let (a, b) = (corners[k], corners[(k + 1) % 4]);
                m.tri_2([a, b, tip], shade(lin(c), 1.0 + 0.15 * (k % 2) as f32));
                m.tri_2([a, b, Vec3::Y * 0.42], lin(0xffffff));
            }
            meshes.add(m.build(true, false))
        })
        .collect();
    commands.insert_resource(KartAssets {
        markers,
        bodies,
        flame: meshes.add(flame.build(true, false)),
        aura: meshes.add(aura.build(true, false)),
        bird: meshes.add(bird.build(true, false)),
    });
}

pub fn spawn_kart(commands: &mut Commands, assets: &KartAssets, mats: &Mats, kart: Kart, tag: impl Bundle) -> Entity {
    let color = kart.color;
    let player = match kart.driver {
        Driver::Human(slot) => Some(slot),
        Driver::Cpu => None,
    };
    commands
        .spawn((Transform::default(), Visibility::default(), kart, tag))
        .with_children(|p| {
            p.spawn((
                Mesh3d(assets.bodies[color].clone()),
                MeshMaterial3d(mats.gloss.clone()),
                Transform::from_scale(Vec3::splat(1.25)),
                KartBody,
            ));
            p.spawn((
                Mesh3d(assets.flame.clone()),
                MeshMaterial3d(mats.glow.clone()),
                Transform::from_scale(Vec3::splat(1.25)),
                Visibility::Hidden,
                BoostFlame,
                NotShadowCaster,
            ));
            p.spawn((
                Mesh3d(assets.aura.clone()),
                MeshMaterial3d(mats.ghost.clone()),
                Transform::default(),
                Visibility::Hidden,
                StarAura,
                NotShadowCaster,
            ));
            p.spawn((
                Mesh3d(assets.bird.clone()),
                MeshMaterial3d(mats.matte.clone()),
                Transform::default(),
                Visibility::Hidden,
                RescueBird,
            ));
            if let Some(slot) = player {
                p.spawn((
                    Mesh3d(assets.markers[color].clone()),
                    MeshMaterial3d(mats.unlit.clone()),
                    Transform::from_xyz(0.0, 4.2, 0.0),
                    RenderLayers::layer(marker_layer(slot)),
                    PlayerMarker,
                    NotShadowCaster,
                ));
            }
        })
        .id()
}

// ---------------------------------------------------------------------------
// Control
// ---------------------------------------------------------------------------

/// Reads each kart's controls: a player's device, or the CPU driver.
pub fn control(
    time: Res<Time>,
    inputs: Res<Inputs>,
    roster: Res<Roster>,
    course: Res<Course>,
    race: Res<Race>,
    scuttlers: Query<&Scuttler>,
    mut karts: Query<&mut Kart>,
) {
    let dt = time.delta_secs();
    let track = &course.track;
    let hazards: Vec<(f32, f32)> = scuttlers.iter().map(|s| (s.u, s.d)).collect();
    for mut k in &mut karts {
        let human = match (k.driver, k.finished) {
            (Driver::Human(slot), None) => roster.players.get(slot).map(|p| inputs.drive(p.device)),
            _ => None,
        };
        let to_go = if race.phase == Phase::Countdown {
            crate::race::COUNTDOWN - race.timer
        } else {
            0.0
        };
        let input = match human {
            Some(drive) if race.autopilot == 0 => drive,
            Some(_) => ai_drive(&mut k, track, &hazards, race.phase, to_go, race.autopilot == 2, dt),
            None => ai_drive(&mut k, track, &hazards, race.phase, to_go, false, dt),
        };
        k.input = input;
    }
}

/// The CPU driver. `to_go` is the time left on the countdown. With `drifter`
/// it uses the drift button as a player would instead of the CPUs' stand-in.
fn ai_drive(
    k: &mut Kart,
    track: &Track,
    hazards: &[(f32, f32)],
    phase: Phase,
    to_go: f32,
    drifter: bool,
    dt: f32,
) -> Drive {
    let loc = track.at(k.u);
    k.ai.lane_timer -= dt;
    if k.ai.lane_timer <= 0.0 {
        k.ai.seed += 1.618;
        k.ai.lane = (k.ai.seed * 12.9898).sin() * 0.7;
        k.ai.lane_timer = 2.5 + (k.ai.seed * 3.3).sin().abs() * 4.0;
    }
    let look_m = 9.0 + k.speed.abs() * 0.5;
    let ahead = track.at(k.u + look_m / loc.len);
    let far = track.at(k.u + (look_m * 2.4) / loc.len);
    // Hug the inside of the coming bend.
    let bend = (ahead.k + far.k) * 0.5;
    let mut target = k.ai.lane * (ahead.half - 2.2) + (bend * 55.0).clamp(-0.55, 0.55) * ahead.half;
    // Steer around critters on the road just ahead.
    for &(hu, hd) in hazards {
        let du = track.delta(k.u, hu) * loc.len;
        if du > 2.0 && du < 30.0 && (hd - target).abs() < 3.2 {
            target += if hd > target { -3.6 } else { 3.6 };
        }
    }
    target = target.clamp(-(ahead.half - 1.2), ahead.half - 1.2);
    let want = ((target - k.d) / look_m).atan();
    let turn_rate = lerp(2.3, 1.45, (k.speed / VMAX).clamp(0.0, 1.0));
    let feed = loc.k * k.speed / turn_rate;
    let steer = ((want - k.yaw) * 3.0 + feed).clamp(-1.0, 1.0);
    // Ease off for bends too tight to take flat out.
    let tight = far.k.abs().max(ahead.k.abs());
    let safe = if tight > 0.001 {
        (turn_rate / tight) * 1.15
    } else {
        f32::MAX
    };
    let racing = matches!(phase, Phase::Racing | Phase::Finished);
    let wants_start_boost = (k.ai.seed * 7.7).sin() > 0.2;
    let gas = if racing {
        if k.speed > safe { 0.0 } else { 1.0 }
    } else if let Phase::Countdown = phase {
        // Some CPUs time the rocket start; the rest are caught napping.
        (wants_start_boost && to_go < 0.55) as u8 as f32
    } else {
        0.0
    };
    // CPUs don't work the drift button, but they get the same reward for a
    // well-taken bend: a slide on the way round and a mini-turbo on the way out.
    let mut steer = steer;
    let mut drift = false;
    if drifter {
        // Hold the drift button through bends and steer by drift rules: the
        // stick sets how tight the slide is, from 0.5 to 2.05 rad/s.
        let bend = if loc.k.abs() > 0.012 { loc.k } else { ahead.k };
        if racing && bend.abs() > 0.014 && k.speed > 0.5 * VMAX {
            drift = true;
            let dir = if k.drift != 0 { k.drift as f32 } else { bend.signum() };
            if k.drift != 0 {
                let rate = (want - k.yaw) * 3.0 + loc.k * k.speed;
                let along = ((rate * dir - 0.5) / 1.55).clamp(0.0, 1.0);
                steer = dir * (2.0 * along - 1.0);
                // Wrong way round for this bend: let go and start again.
                if bend.signum() != dir && bend.abs() > 0.02 {
                    drift = false;
                }
            } else {
                steer = dir;
            }
        }
    } else if racing && k.spin <= 0.0 && k.grounded && loc.k.abs().max(ahead.k.abs()) > 0.02 && k.speed > 0.55 * VMAX {
        k.ai.bend_time += dt;
        k.ai.bend_dir = if loc.k.abs() > 0.008 {
            loc.k.signum()
        } else {
            ahead.k.signum()
        };
        k.drift_charge = k.ai.bend_time * 1.25;
    } else {
        if k.ai.bend_time > 0.8 && racing && k.spin <= 0.0 {
            let level = k.drift_level().max(1);
            k.give_boost(0.4 + 0.35 * level as f32);
        }
        k.ai.bend_time = 0.0;
        if k.drift == 0 {
            k.drift_charge = 0.0;
        }
    }
    // Use items after a short think.
    let mut item = false;
    if k.item.is_some() && k.roulette <= 0.0 && racing {
        k.ai.item_wait -= dt;
        if k.ai.item_wait <= 0.0 {
            item = true;
            k.ai.item_wait = 1.0 + (k.ai.seed * 5.1).sin().abs() * 4.0;
        }
    }
    Drive {
        steer,
        gas,
        brake: if racing && k.speed > safe * 1.25 { 1.0 } else { 0.0 },
        drift,
        item,
    }
}

// ---------------------------------------------------------------------------
// Physics
// ---------------------------------------------------------------------------

enum KEvent {
    Bump(f32),
    Land(f32),
    Boost,
    MiniTurbo(u8),
    Fall,
    Splash,
    Hop,
    Rescued,
}

pub fn drive(
    time: Res<Time>,
    course: Res<Course>,
    race: Res<Race>,
    settings: Res<Settings>,
    roster: Res<Roster>,
    mut fx: ResMut<Fx>,
    mut sfx: MessageWriter<Sfx>,
    mut rumbles: MessageWriter<GamepadRumbleRequest>,
    mut karts: Query<(&mut Kart, &mut Transform)>,
) {
    let dt = time.delta_secs().min(1.0 / 20.0);
    if dt <= 0.0 {
        return;
    }
    let track = &course.track;
    let sea = match course.env.ground {
        Ground::Sea { level, .. } => Some(level),
        Ground::Plain { .. } | Ground::Sky { .. } => None,
    };
    let class = CLASSES[settings.class].1;
    let mut events: Vec<KEvent> = Vec::new();
    for (mut k, mut tf) in &mut karts {
        events.clear();
        let vmax = VMAX
            * class
            * if k.is_human() && k.finished.is_none() {
                1.0
            } else {
                k.ai.skill * k.ai.band
            };
        step(
            &mut k,
            track,
            sea,
            course.out.spout,
            vmax,
            race.phase,
            dt,
            &mut fx,
            &mut events,
        );
        tf.translation = k.pos;
        tf.rotation = visual_rotation(&k, track);
        tf.scale = Vec3::ONE;

        let device = k.device(&roster).filter(|_| k.finished.is_none());
        let audible = k.is_human();
        for e in &events {
            let (sound, strong, weak, secs) = match *e {
                KEvent::Bump(s) => (Sound::Bump, 0.5 * s, 0.3, 0.15),
                KEvent::Land(t) => (Sound::Land, (t * 0.7).min(0.8), 0.2, 0.18),
                KEvent::Boost => (Sound::Boost, 0.2, 0.7, 0.35),
                KEvent::MiniTurbo(l) => (Sound::MiniTurbo, 0.1, 0.35 + 0.15 * l as f32, 0.25),
                KEvent::Fall => (Sound::Fall, 0.6, 0.6, 0.4),
                KEvent::Splash => (Sound::Splash, 0.7, 0.3, 0.3),
                KEvent::Hop => (Sound::Hop, 0.0, 0.2, 0.06),
                KEvent::Rescued => (Sound::Land, 0.2, 0.1, 0.1),
            };
            if audible {
                sfx.write(Sfx(sound));
            }
            if let Some(device) = device {
                rumble(&mut rumbles, device, strong, weak, secs);
            }
        }
    }
}

fn step(
    k: &mut Kart,
    track: &Track,
    sea: Option<f32>,
    spout: bool,
    vmax_base: f32,
    phase: Phase,
    dt: f32,
    fx: &mut Fx,
    events: &mut Vec<KEvent>,
) {
    let n = track.n();
    k.boost = (k.boost - dt).max(0.0);
    k.star = (k.star - dt).max(0.0);
    k.invuln = (k.invuln - dt).max(0.0);
    k.spin = (k.spin - dt).max(0.0);
    k.lap_flash = (k.lap_flash - dt).max(0.0);
    k.fx_timer -= dt;
    let loc = track.at(k.u);
    let right = loc.r.normalize_or(Vec3::X);

    // Being carried back to the road.
    if k.respawn > 0.0 {
        k.respawn -= dt;
        k.h = 16.0 * (k.respawn / RESPAWN).clamp(0.0, 1.0).powf(1.4);
        k.speed = 0.0;
        k.lat = 0.0;
        k.yaw = 0.0;
        k.vy = 0.0;
        k.grounded = false;
        k.ground_y = loc.at(k.d).y;
        if k.respawn <= 0.0 {
            k.h = 0.0;
            k.grounded = true;
            k.invuln = 1.6;
            events.push(KEvent::Rescued);
        }
        finish_step(k, track);
        return;
    }

    let racing = matches!(phase, Phase::Racing | Phase::Finished);
    let inp = if !racing || k.spin > 0.0 || k.fallen {
        Drive::default()
    } else {
        k.input
    };
    if let Phase::Countdown = phase {
        k.gas_held = if k.input.gas > 0.5 { k.gas_held + dt } else { 0.0 };
    }
    let near_ground = k.grounded || k.h < 0.35;
    let offroad = k.d.abs() > loc.half;

    // ------------------------------------------------------------ steering
    let speed_f = (k.speed.abs() / 7.0).min(1.0);
    let hi = (k.speed / vmax_base).clamp(0.0, 1.0);
    let mut turn = inp.steer * lerp(2.3, 1.45, hi) * speed_f * k.speed.signum();
    let mut hop = false;
    if inp.drift && !k.drift_held && k.grounded && k.drift == 0 {
        hop = true;
    }
    if inp.drift && k.drift == 0 && near_ground && inp.steer.abs() > 0.3 && k.speed > 0.45 * vmax_base {
        k.drift = inp.steer.signum() as i8;
        k.drift_charge = 0.0;
    }
    if k.drift != 0 {
        if !inp.drift || k.speed < 0.3 * vmax_base {
            let level = k.drift_level();
            if level > 0 && inp.steer.abs() < 2.0 && k.spin <= 0.0 {
                k.give_boost(0.45 + 0.45 * level as f32);
                debug!("{} mini-turbo level {level}", k.name);
                events.push(KEvent::MiniTurbo(level));
            }
            k.drift = 0;
            k.drift_charge = 0.0;
        } else {
            let dir = k.drift as f32;
            let along = ((inp.steer * dir) + 1.0) * 0.5;
            turn = dir * lerp(0.5, 2.05, along);
            k.drift_charge += dt * lerp(0.55, 1.5, along);
        }
    }
    k.drift_held = inp.drift;
    if !near_ground {
        turn *= 0.3;
    }
    k.yaw += turn * dt;
    k.steer_vis = damp(k.steer_vis, inp.steer, 10.0, dt);

    // ------------------------------------------------------------ speed
    let mut vmax = vmax_base;
    let mut accel = 17.0;
    if k.depth > 0.25 && k.star <= 0.0 {
        vmax *= lerp(0.82, 0.5, ((k.depth - 0.25) / 2.5).clamp(0.0, 1.0));
    }
    if offroad && k.boost <= 0.0 && k.star <= 0.0 {
        vmax *= 0.56;
    }
    if k.star > 0.0 {
        vmax *= 1.16;
        accel = 26.0;
    }
    if k.boost > 0.0 {
        vmax *= 1.36;
        accel = 48.0;
    }
    if near_ground {
        let gas = if k.boost > 0.0 { 1.0 } else { inp.gas };
        if gas > 0.05 && k.speed < vmax {
            k.speed += accel * gas * (1.06 - k.speed / vmax).clamp(0.0, 1.0) * dt;
        }
        if inp.brake > 0.05 {
            if k.speed > 0.5 {
                k.speed -= 36.0 * inp.brake * dt;
            } else if gas < 0.05 {
                k.speed = (k.speed - 14.0 * inp.brake * dt).max(-11.0);
            }
        }
        if gas < 0.05 && inp.brake < 0.05 {
            k.speed -= k.speed * 0.6 * dt;
        }
        if k.speed > vmax {
            k.speed -= (k.speed - vmax) * (if offroad { 3.5 } else { 1.8 }) * dt;
        }
        k.speed -= 9.0 * loc.t.y * k.yaw.cos() * dt;
        k.speed -= k.speed * turn.abs() * 0.03 * dt;
        if k.spin > 0.0 {
            k.speed -= k.speed * 2.4 * dt;
        }
        // Banked or tilting ground drags the kart downhill.
        let slide = -right.y * 12.0 - k.drift as f32 * 2.2;
        k.lat = damp(k.lat, slide, 5.0, dt);
    }
    if !racing {
        k.speed = 0.0;
        k.lat = 0.0;
    }

    // ------------------------------------------------------------ move
    let scale = (1.0 - k.d * loc.k).clamp(0.4, 2.5);
    let ds = k.speed * k.yaw.cos() * dt;
    let mut u = k.u + ds / (loc.len * scale);
    k.d += (k.speed * k.yaw.sin() + k.lat) * dt;
    k.yaw = wrap_angle(k.yaw - loc.k * ds / scale);
    if u >= n {
        u -= n;
        k.lap += 1;
        k.lap_flash = 2.2;
    } else if u < 0.0 {
        u += n;
        k.lap -= 1;
    }
    k.u = u;
    let heading_back = k.yaw.cos() < -0.3 && k.speed > 4.0;
    k.wrong_way = if heading_back { k.wrong_way + dt } else { 0.0 };

    // ------------------------------------------------------------ edges
    let loc = track.at(k.u);
    let gap = track.gap_at(k.u);
    let limit = loc.limit(k.d);
    if k.d.abs() > limit && !k.fallen && gap.is_none() {
        let side = k.d.signum();
        match loc.edge_at(k.d) {
            Edge::Fall => {
                if k.h < 1.5 {
                    k.fallen = true;
                    k.fall_t = 0.0;
                    k.splashed = false;
                    k.grounded = false;
                    k.vy = k.vy.min(2.0);
                    k.drift = 0;
                    events.push(KEvent::Fall);
                    debug!("{} fell off at u={:.0} d={:.1} speed={:.0}", k.name, k.u, k.d, k.speed);
                }
            }
            edge => {
                k.d = side * limit;
                let outward = (k.yaw.sin() * side).max(0.0);
                if k.lat * side > 0.0 {
                    k.lat = 0.0;
                }
                if edge == Edge::Wall {
                    if outward > 0.04 {
                        if outward * k.speed.abs() > 4.0 {
                            events.push(KEvent::Bump((outward * k.speed.abs() / 20.0).min(1.0)));
                            fx.burst(
                                Puff::Hit,
                                k.pos + right * side * 1.2 + Vec3::Y * 0.6,
                                Vec3::ZERO,
                                5,
                                5.0,
                                0.25,
                                0.3,
                            );
                        }
                        k.speed *= 1.0 - 0.45 * outward;
                        k.yaw = -k.yaw * 0.3;
                    }
                } else {
                    // Undergrowth: soft, but it saps speed and turns you back.
                    k.speed -= k.speed * 1.6 * dt;
                    k.yaw -= side * outward * 3.0 * dt;
                }
            }
        }
    }

    // ------------------------------------------------------------ vertical
    let ground = loc.at(k.d);
    // How fast the road rises under the kart because of where it drove, with
    // the beast's own motion taken out.
    let ground_vy = if racing {
        ((track.carried_from(k.u, k.d).y - k.ground_y) / dt).clamp(-45.0, 45.0)
    } else {
        0.0
    };
    if let Some((a, b, p)) = gap {
        // The leap: a scripted arc that always lands on the far side.
        let span = (b - a).rem_euclid(n) * loc.len;
        let peak = 6.0 + span * 0.1;
        if !k.in_gap {
            debug!("{} leaps the gap ({span:.0} m)", k.name);
            k.in_gap = true;
            k.gap_h0 = k.h;
            k.air_time = 0.0;
            events.push(KEvent::Hop);
        }
        let h = 4.0 * peak * p * (1.0 - p) + k.gap_h0 * (1.0 - p);
        k.vy = ground_vy + (h - k.h) / dt;
        k.h = h;
        k.grounded = false;
        k.from_ramp = true;
        k.air_time += dt;
        k.speed = k.speed.max(27.0);
        k.yaw = damp(k.yaw, 0.0, 3.0, dt);
        let keep = loc.half - 1.0;
        k.d = damp(k.d, k.d.clamp(-keep, keep), 6.0, dt);
        k.drift = 0;
    } else if k.fallen {
        k.in_gap = false;
        k.fall_t += dt;
        k.vy -= GRAVITY * dt;
        k.h += (k.vy - ground_vy) * dt;
        k.speed -= k.speed * 0.8 * dt;
        let world_y = ground.y + k.h;
        if let Some(level) = sea
            && world_y < level
            && !k.splashed
        {
            k.splashed = true;
            k.fall_t = k.fall_t.max(0.9);
            events.push(KEvent::Splash);
            let at = Vec3::new(k.pos.x, level, k.pos.z);
            fx.burst(Puff::Water, at, Vec3::Y * 9.0, 26, 11.0, 0.9, 1.1);
        }
        if k.fall_t > 1.4 {
            // Set back down a little way before the place it left the road.
            k.fallen = false;
            k.respawn = RESPAWN;
            let mut back = k.u - 5.0;
            while track.gap_at(back).is_some() {
                back -= 2.0;
            }
            if back < 0.0 {
                back += n;
                k.lap -= 1;
            }
            k.u = back;
            k.d = 0.0;
            k.h = 16.0;
            k.item_uses = k.item_uses.min(1);
        }
    } else if k.grounded {
        k.in_gap = false;
        // An erupting blowhole throws the kart high; it gets a boost when it lands.
        if spout && racing && track.geysers.iter().any(|&g| track.within(k.u, g - 1.6, g + 1.6)) && k.launch <= 0.0 {
            k.launch = 21.0;
            events.push(KEvent::Hop);
        }
        if hop || k.launch > 0.0 {
            k.vy = ground_vy.max(0.0) + if k.launch > 0.0 { k.launch } else { HOP };
            k.launch = 0.0;
            k.grounded = false;
            k.h = 0.02;
            k.air_time = 0.0;
            k.from_ramp = k.vy > 18.0;
            if hop {
                events.push(KEvent::Hop);
            }
        } else if ground_vy < k.vy - STICK * dt && k.speed.abs() > 6.0 {
            // The road dropped away faster than the kart can follow.
            k.vy -= GRAVITY * dt;
            k.h = ((k.vy - ground_vy) * dt).max(0.0);
            k.grounded = false;
            k.air_time = 0.0;
            k.from_ramp = track.ramps.iter().any(|r| track.within(k.u, r.0 - 1.0, r.1 + 3.0));
        } else {
            k.vy = ground_vy;
            k.h = 0.0;
        }
    } else {
        k.in_gap = false;
        if k.launch > 0.0 {
            k.vy = k.vy.max(0.0) + k.launch;
            k.launch = 0.0;
        }
        k.vy -= GRAVITY * dt;
        k.h += (k.vy - ground_vy) * dt;
        k.air_time += dt;
        if k.h <= 0.0 {
            k.h = 0.0;
            k.grounded = true;
            let hard = k.air_time;
            if hard > 0.3 {
                events.push(KEvent::Land(hard));
                fx.burst(Puff::Dust, k.pos, Vec3::ZERO, 8, 5.0, 0.5, 0.7);
                if k.from_ramp && k.spin <= 0.0 {
                    k.give_boost(0.7);
                    events.push(KEvent::Boost);
                }
            }
            k.from_ramp = false;
            k.vy = ground_vy;
        }
    }
    k.ground_y = ground.y;

    // ------------------------------------------------------------ road effects
    k.depth = match sea {
        Some(level) if k.grounded => (level - ground.y).max(0.0),
        _ => 0.0,
    };
    if k.grounded && racing {
        for pad in &track.boosts {
            if track.within(k.u, pad.u0, pad.u1) && (k.d - pad.d).abs() < pad.half + 0.6 && k.boost < 0.9 {
                k.give_boost(1.25);
                events.push(KEvent::Boost);
            }
        }
    }

    finish_step(k, track);

    // Trails.
    if k.fx_timer <= 0.0 && k.grounded && k.speed.abs() > 5.0 {
        k.fx_timer = 0.035;
        let back = k.pos - k.fwd * 1.3;
        let side = k.fwd.cross(k.up).normalize_or(Vec3::X);
        if k.depth > 0.25 {
            let at = back + Vec3::Y * k.depth.min(1.5);
            fx.burst(Puff::Water, at, k.up * 5.0 - k.fwd * 3.0, 2, 5.0, 0.5, 0.6);
        } else if k.slide() != 0.0 {
            let level = k.drift_level();
            let kind = if level == 0 { Puff::Dust } else { Puff::Spark(level) };
            let wheel = back - side * k.slide() * 1.0 + k.up * 0.25;
            fx.burst(
                kind,
                wheel,
                k.up * 3.0 - k.fwd * 2.0,
                2,
                3.5,
                0.3,
                if level == 0 { 0.5 } else { 0.3 },
            );
        } else if offroad {
            fx.burst(Puff::Dust, back + k.up * 0.2, k.up * 2.0, 1, 2.5, 0.4, 0.35);
        }
        if k.boost > 0.0 {
            let j = fx.jitter(0.3);
            fx.puff(Puff::Flame, back + k.up * 1.1 + j, -k.fwd * 6.0, 0.25, 0.45);
        }
        if k.star > 0.0 {
            let j = fx.jitter(1.4);
            fx.puff(Puff::Star, k.pos + k.up * 1.2 + j, k.up * 3.0, 0.5, 0.45);
        }
    }
}

/// Converts track coordinates into the world pose used by everything else.
fn finish_step(k: &mut Kart, track: &Track) {
    let loc = track.at(k.u);
    let right = loc.r.normalize_or(Vec3::X);
    let float = if k.grounded {
        (k.depth - 0.9).clamp(0.0, 3.0)
    } else {
        0.0
    };
    k.pos = loc.at(k.d) + Vec3::Y * (k.h + float);
    k.fwd = (loc.t * k.yaw.cos() + right * k.yaw.sin()).normalize_or(Vec3::Z);
    k.up = loc.n;
}

fn visual_rotation(k: &Kart, track: &Track) -> Quat {
    let loc = track.at(k.u);
    let mut yaw = k.yaw + k.slide() * if k.drift != 0 { 0.42 } else { 0.3 };
    let mut tilt = Quat::IDENTITY;
    if k.spin > 0.0 {
        let total = if k.tumble { 1.5 } else { 1.1 };
        let f = 1.0 - k.spin / total;
        if k.tumble {
            tilt = Quat::from_rotation_x(-f * TAU * 1.0);
        } else {
            yaw += f * TAU * 2.0;
        }
    }
    if k.fallen {
        tilt = Quat::from_rotation_x(-(k.fall_t * 1.2).min(1.2)) * Quat::from_rotation_z(k.fall_t * 2.0);
    } else if !k.grounded && k.respawn <= 0.0 {
        // Nose follows the arc a little.
        tilt *= Quat::from_rotation_x((k.vy * 0.018).clamp(-0.35, 0.35));
    }
    let lean = Quat::from_rotation_z(-k.steer_vis * 0.07 * (k.speed / VMAX).clamp(0.0, 1.0) - k.slide() * 0.1);
    loc.rotation(yaw) * tilt * lean
}

// ---------------------------------------------------------------------------
// Collisions
// ---------------------------------------------------------------------------

pub fn collide(
    course: Res<Course>,
    race: Res<Race>,
    roster: Res<Roster>,
    mut fx: ResMut<Fx>,
    mut sfx: MessageWriter<Sfx>,
    mut rumbles: MessageWriter<GamepadRumbleRequest>,
    mut karts: Query<&mut Kart>,
    mut scuttlers: Query<&mut Scuttler>,
) {
    if !matches!(race.phase, Phase::Racing | Phase::Finished) {
        return;
    }
    let track = &course.track;
    let mut all: Vec<Mut<Kart>> = karts.iter_mut().collect();
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            let (left, rest) = all.split_at_mut(j);
            let (a, b) = (&mut left[i], &mut rest[0]);
            if a.respawn > 0.0 || b.respawn > 0.0 || a.fallen || b.fallen {
                continue;
            }
            let delta = b.pos - a.pos;
            let dist = delta.length();
            if dist > KART_RADIUS * 2.0 || dist < 1e-4 {
                continue;
            }
            // Work in the road plane: along-track and across-track.
            let loc = track.at(a.u);
            let (ds, dd) = (track.delta(a.u, b.u) * loc.len, b.d - a.d);
            let len = (ds * ds + dd * dd).sqrt().max(1e-3);
            let (ns, nd) = (ds / len, dd / len);
            let overlap = (KART_RADIUS * 2.0 - len).max(0.0);
            let (wa, wb) = match (a.star > 0.0, b.star > 0.0) {
                (true, false) => (0.0, 1.0),
                (false, true) => (1.0, 0.0),
                _ => (0.5, 0.5),
            };
            a.d -= nd * overlap * wa;
            b.d += nd * overlap * wb;
            a.u = track.wrap(a.u - ns * overlap * wa / loc.len);
            b.u = track.wrap(b.u + ns * overlap * wb / loc.len);
            // Being shoved is not driving: keep it out of the vertical motion.
            a.ground_y = track.at(a.u).at(a.d).y;
            b.ground_y = track.at(b.u).at(b.d).y;
            let closing = (a.speed - b.speed) * ns;
            a.lat -= nd * 5.0 * wa * 2.0;
            b.lat += nd * 5.0 * wb * 2.0;
            if closing > 0.0 {
                // Trade some speed along the line between them.
                a.speed -= closing * 0.4 * ns;
                b.speed += closing * 0.4 * ns;
            }
            let mut struck = false;
            if a.star > 0.0 && b.star <= 0.0 {
                struck |= b.hit(true);
            }
            if b.star > 0.0 && a.star <= 0.0 {
                struck |= a.hit(true);
            }
            let mid = (a.pos + b.pos) * 0.5 + Vec3::Y * 0.7;
            if struck {
                fx.burst(Puff::Hit, mid, Vec3::Y * 4.0, 14, 8.0, 0.5, 0.45);
            }
            for k in [&**a, &**b] {
                if k.is_human() {
                    sfx.write(Sfx(if struck { Sound::Hit } else { Sound::Bump }));
                    if let Some(device) = k.device(&roster) {
                        rumble(&mut rumbles, device, 0.5, 0.4, 0.15);
                    }
                }
            }
        }
    }

    // Critters on the road.
    for mut s in &mut scuttlers {
        if s.squashed > 0.0 {
            continue;
        }
        for k in all.iter_mut() {
            if !k.grounded || k.respawn > 0.0 {
                continue;
            }
            let loc = track.at(k.u);
            let ds = track.delta(k.u, s.u) * loc.len;
            if ds.abs() < 2.0 && (k.d - s.d).abs() < 2.1 {
                s.squashed = 3.0;
                if k.hit(false) {
                    fx.burst(Puff::Hit, k.pos + Vec3::Y, Vec3::Y * 4.0, 12, 7.0, 0.5, 0.4);
                    if k.is_human() {
                        sfx.write(Sfx(Sound::Hit));
                        if let Some(device) = k.device(&roster) {
                            rumble(&mut rumbles, device, 0.8, 0.5, 0.35);
                        }
                    }
                }
                break;
            }
        }
    }
}

/// Shows and animates each kart's attachments.
pub fn dress(
    time: Res<Time>,
    karts: Query<(&Kart, &Children)>,
    mut bodies: Query<
        &mut Visibility,
        (
            With<KartBody>,
            Without<BoostFlame>,
            Without<StarAura>,
            Without<RescueBird>,
        ),
    >,
    mut flames: Query<(&mut Visibility, &mut Transform), (With<BoostFlame>, Without<StarAura>, Without<RescueBird>)>,
    mut auras: Query<(&mut Visibility, &mut Transform), (With<StarAura>, Without<BoostFlame>, Without<RescueBird>)>,
    mut birds: Query<(&mut Visibility, &mut Transform), (With<RescueBird>, Without<BoostFlame>, Without<StarAura>)>,
    mut markers: Query<
        &mut Transform,
        (
            With<PlayerMarker>,
            Without<BoostFlame>,
            Without<StarAura>,
            Without<RescueBird>,
        ),
    >,
) {
    let t = time.elapsed_secs();
    let show = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
    for (k, children) in &karts {
        for child in children.iter() {
            if let Ok(mut vis) = bodies.get_mut(child) {
                // Blink while invulnerable after a hit.
                let blink = k.invuln > 0.0 && k.spin <= 0.0 && k.star <= 0.0 && (t * 18.0).sin() > 0.3;
                *vis = show(!blink);
            }
            if let Ok((mut vis, mut tf)) = flames.get_mut(child) {
                *vis = show(k.boost > 0.0);
                let flick = 1.0 + 0.25 * (t * 47.0 + k.index as f32).sin();
                tf.scale = Vec3::new(1.25, 1.25, 1.25 * flick);
            }
            if let Ok((mut vis, mut tf)) = auras.get_mut(child) {
                *vis = show(k.star > 0.0);
                tf.rotation = Quat::from_rotation_y(t * 7.0);
                tf.scale = Vec3::splat(1.0 + 0.08 * (t * 20.0).sin());
            }
            if let Ok(mut tf) = markers.get_mut(child) {
                tf.translation.y = 4.2 + 0.35 * (t * 3.0 + k.index as f32).sin();
                tf.rotation = Quat::from_rotation_y(t * 2.0);
            }
            if let Ok((mut vis, mut tf)) = birds.get_mut(child) {
                *vis = show(k.respawn > 0.0);
                tf.translation.y = 0.4 * (t * 9.0).sin();
                tf.scale = Vec3::new(1.0, 1.0 + 0.12 * (t * 14.0).sin(), 1.0);
            }
        }
    }
}
