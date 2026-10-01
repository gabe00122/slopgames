//! The player's side: the third-person camera, aiming, and turning keys and
//! mouse into the agent's [`Intent`].

use crate::{
    Args,
    agent::{Agent, Intent},
    decals::Decals,
    fx::Fx,
    game::{Match, SLOTS, Screen, Slot, Targets, Team},
    spells::{Spell, clamp_aim, wall_ends},
    terrain::{HALF, Terrain},
    units::{Order, summon_point},
    util::{damp, damp_v, dir_of, xz},
};
use bevy::{
    anti_alias::fxaa::Fxaa,
    camera::Hdr,
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    post_process::bloom::Bloom,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

#[derive(Component)]
pub struct MainCam {
    pub pos: Vec3,
    pub look: Vec3,
    pub orbit: f32,
    pub live: bool,
}

/// Mouse look state and settings.
#[derive(Resource)]
pub struct Look {
    pub yaw: f32,
    pub pitch: f32,
    pub sensitivity: f32,
    pub invert: bool,
    /// The point under the crosshair.
    pub aim: Vec3,
    /// Whether the aim is on an enemy.
    pub aim_enemy: bool,
    /// A short notice for the player (settings changes), and for how long.
    pub notice: Option<(String, f32)>,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            yaw: 0.0,
            pitch: 0.2,
            sensitivity: 1.0,
            invert: false,
            aim: Vec3::ZERO,
            aim_enemy: false,
            notice: None,
        }
    }
}

pub fn spawn_camera(mut commands: Commands, args: Res<Args>) {
    let mut e = commands.spawn((
        Camera3d::default(),
        Hdr,
        Tonemapping::TonyMcMapface,
        Bloom {
            intensity: if args.low { 0.0 } else { 0.16 },
            ..Bloom::NATURAL
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 62f32.to_radians(),
            near: 0.3,
            far: 3000.0,
            ..default()
        }),
        DistanceFog {
            color: Color::srgba(0.36, 0.3, 0.52, 1.0),
            directional_light_color: Color::srgba(1.0, 0.75, 0.55, 0.4),
            directional_light_exponent: 18.0,
            falloff: FogFalloff::Linear {
                start: 110.0,
                end: 420.0,
            },
        },
        Transform::from_xyz(0.0, 90.0, 140.0).looking_at(Vec3::ZERO, Vec3::Y),
        MainCam {
            pos: Vec3::new(0.0, 90.0, 140.0),
            look: Vec3::ZERO,
            orbit: 0.0,
            live: false,
        },
        IsDefaultUiCamera,
    ));
    if args.low {
        e.insert((Msaa::Off, Fxaa::default()));
    } else {
        e.insert(Msaa::Sample4);
    }
}

/// Whether the player is steering right now (not paused, not on a menu).
#[derive(Resource, Default)]
pub struct Control {
    pub paused: bool,
}

pub fn cursor(
    screen: Res<State<Screen>>,
    control: Res<Control>,
    args: Res<Args>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let want = *screen.get() == Screen::Playing && !control.paused && args.autopilot == 0 && args.shot.is_none();
    let locked = cursor.grab_mode != CursorGrabMode::None;
    if want && !locked && (mouse.just_pressed(MouseButton::Left) || window.focused) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    } else if !want && locked {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn stick(v: Vec2) -> Vec2 {
    const DEAD: f32 = 0.15;
    let l = v.length();
    if l < DEAD {
        return Vec2::ZERO;
    }
    v / l * ((l - DEAD) / (1.0 - DEAD)).min(1.0).powf(1.4)
}

/// Keys, mouse and gamepad to intent, and the aim point.
///
/// Gamepad: left stick moves, right stick looks, RT casts, LT sends the
/// subagents, LB/RB (or the d-pad) pick a slot, A jumps, X interacts,
/// Y regroups.
pub fn input(
    (keys, mouse, motion, scroll): (
        Res<ButtonInput<KeyCode>>,
        Res<ButtonInput<MouseButton>>,
        Res<AccumulatedMouseMotion>,
        Res<AccumulatedMouseScroll>,
    ),
    real: Res<Time<Real>>,
    pads: Query<&Gamepad>,
    control: Res<Control>,
    args: Res<Args>,
    terrain: Option<Res<Terrain>>,
    targets: Res<Targets>,
    game: Option<Res<Match>>,
    mut look: ResMut<Look>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    cam: Query<&Transform, With<MainCam>>,
    mut agents: Query<(&mut Agent, &mut Intent)>,
    mut sfx: MessageWriter<crate::audio::Sfx>,
) {
    let Some(terrain) = terrain else { return };
    let Some(game) = game else { return };
    let Some((mut agent, mut intent)) = agents.iter_mut().find(|(a, _)| a.player) else {
        return;
    };
    let dt = real.delta_secs().min(0.05);
    let pad = pads.iter().next();
    // Look settings: [ and ] for sensitivity, I to invert.
    let mut changed = false;
    if keys.just_pressed(KeyCode::BracketLeft) {
        look.sensitivity = (look.sensitivity / 1.2).max(0.2);
        changed = true;
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        look.sensitivity = (look.sensitivity * 1.2).min(5.0);
        changed = true;
    }
    if keys.just_pressed(KeyCode::KeyI) {
        look.invert = !look.invert;
        changed = true;
    }
    if changed {
        let msg = format!(
            "look sensitivity {:.1}{}",
            look.sensitivity,
            if look.invert { "  (inverted)" } else { "" }
        );
        look.notice = Some((msg, 1.5));
    }
    if let Some((_, t)) = look.notice.as_mut() {
        *t -= dt;
        if *t <= 0.0 {
            look.notice = None;
        }
    }
    let steering = !control.paused && game.live() && args.autopilot == 0;
    if steering && cursor.grab_mode != CursorGrabMode::None {
        let d = motion.delta * 0.0024 * look.sensitivity;
        look.yaw -= d.x;
        let dy = if look.invert { -d.y } else { d.y };
        look.pitch = (look.pitch + dy).clamp(-0.45, 1.15);
    }
    if steering && let Some(pad) = pad {
        let r = stick(pad.right_stick());
        look.yaw -= r.x * 2.7 * dt * look.sensitivity;
        let dy = if look.invert { r.y } else { -r.y };
        look.pitch = (look.pitch + dy * 1.7 * dt * look.sensitivity).clamp(-0.45, 1.15);
    }
    // Aim: along the view ray, stopping at the ground or an enemy.
    if let Ok(ctf) = cam.single() {
        let origin = ctf.translation;
        let dir = ctf.forward().as_vec3();
        let ground = terrain.raycast(origin, dir, 400.0);
        let ground_t = ground.map_or(400.0, |g| g.distance(origin));
        let mut best: Option<(f32, Vec3)> = None;
        for t in &targets.0 {
            if t.team == agent.team {
                continue;
            }
            let c = t.pos + Vec3::Y * if t.flying { 0.0 } else { 1.0 };
            let along = (c - origin).dot(dir);
            if along <= 0.0 || along > ground_t {
                continue;
            }
            let off = (origin + dir * along).distance(c);
            // A little more forgiving with a gamepad.
            let slack = if pad.is_some() { 1.2 } else { 0.6 };
            if off < t.radius + slack && best.is_none_or(|b| along < b.0) {
                best = Some((along, c));
            }
        }
        look.aim_enemy = best.is_some();
        look.aim = match (best, ground) {
            (Some((_, c)), _) => c,
            (None, Some(g)) => g,
            // At the sky: the ground far along the view direction.
            (None, None) => {
                let p = xz(origin) + xz(dir).normalize_or(Vec2::Y) * 120.0;
                terrain.ground(Terrain::clamp_inside(p, 6.0))
            }
        };
    }
    intent.aim = look.aim;
    if !steering {
        intent.mv = Vec2::ZERO;
        intent.jump = false;
        return;
    }
    // Movement relative to the view.
    let fwd = dir_of(look.yaw);
    let right = Vec2::new(-fwd.y, fwd.x);
    let mut mv = Vec2::ZERO;
    let key = |k: KeyCode| keys.pressed(k);
    if key(KeyCode::KeyW) || key(KeyCode::ArrowUp) {
        mv += fwd;
    }
    if key(KeyCode::KeyS) || key(KeyCode::ArrowDown) {
        mv -= fwd;
    }
    if key(KeyCode::KeyD) || key(KeyCode::ArrowRight) {
        mv += right;
    }
    if key(KeyCode::KeyA) || key(KeyCode::ArrowLeft) {
        mv -= right;
    }
    let pad_pressed = |b: GamepadButton| pad.is_some_and(|p| p.pressed(b));
    let pad_just = |b: GamepadButton| pad.is_some_and(|p| p.just_pressed(b));
    if let Some(pad) = pad {
        let l = stick(pad.left_stick());
        mv += fwd * l.y + right * l.x;
    }
    intent.mv = mv.clamp_length_max(1.0);
    intent.face = Some(look.yaw);
    intent.snap = true;
    intent.jump = keys.pressed(KeyCode::Space) || pad_pressed(GamepadButton::South);
    if agent.dead.is_some() {
        return;
    }
    // Choosing a slot.
    let digits = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
        KeyCode::Digit0,
    ];
    let before = agent.selected;
    for (i, k) in digits.iter().enumerate() {
        if keys.just_pressed(*k) {
            agent.selected = i;
        }
    }
    let wheel = scroll.delta.y;
    let next = wheel < -0.01
        || keys.just_pressed(KeyCode::KeyE)
        || pad_just(GamepadButton::RightTrigger)
        || pad_just(GamepadButton::DPadRight);
    let prev = wheel > 0.01
        || keys.just_pressed(KeyCode::KeyQ)
        || pad_just(GamepadButton::LeftTrigger)
        || pad_just(GamepadButton::DPadLeft);
    if next {
        agent.selected = (agent.selected + 1) % SLOTS.len();
    } else if prev {
        agent.selected = (agent.selected + SLOTS.len() - 1) % SLOTS.len();
    }
    // D-pad up and down jump between spells and summons.
    if pad_just(GamepadButton::DPadUp) || pad_just(GamepadButton::DPadDown) {
        agent.selected = (agent.selected + 5) % SLOTS.len();
    }
    if agent.selected != before {
        sfx.write(crate::audio::Sfx::ui(crate::audio::Sound::Tick));
    }
    // Casting: SEGFAULT repeats while held, everything else needs a fresh press.
    let slot = SLOTS[agent.selected];
    let mouse_ok = cursor.grab_mode != CursorGrabMode::None;
    let fire = match slot {
        Slot::Spell(Spell::Segfault) => {
            (mouse_ok && mouse.pressed(MouseButton::Left)) || pad_pressed(GamepadButton::RightTrigger2)
        }
        _ => (mouse_ok && mouse.just_pressed(MouseButton::Left)) || pad_just(GamepadButton::RightTrigger2),
    };
    if fire {
        intent.cast = Some((agent.selected, look.aim));
    }
    if (mouse_ok && mouse.just_pressed(MouseButton::Right)) || pad_just(GamepadButton::LeftTrigger2) {
        intent.order = Some(Order::Move(look.aim));
    }
    if keys.just_pressed(KeyCode::KeyR) || pad_just(GamepadButton::North) {
        intent.order = Some(Order::Follow);
    }
    if keys.just_pressed(KeyCode::KeyF) || pad_just(GamepadButton::West) {
        intent.interact = true;
    }
}

/// The aiming marks on the ground for the selected slot.
pub fn aim_marks(
    look: Res<Look>,
    terrain: Option<Res<Terrain>>,
    control: Res<Control>,
    game: Option<Res<Match>>,
    mut decals: ResMut<Decals>,
    agents: Query<(&Agent, &Transform)>,
) {
    let (Some(terrain), Some(game)) = (terrain, game) else {
        return;
    };
    if control.paused || !game.live() {
        return;
    }
    let Some((a, tf)) = agents.iter().find(|(a, _)| a.player) else {
        return;
    };
    if a.dead.is_some() {
        return;
    }
    let color = if crate::agent::can_afford(a, a.selected) && a.cooldowns[a.selected] <= 0.0 {
        a.team.color()
    } else {
        0x8b95a7
    };
    match SLOTS[a.selected] {
        Slot::Spell(s) => {
            let def = s.def();
            match s {
                Spell::Hotfix => decals.ring_now(xz(tf.translation), def.radius, 0x7ee787),
                Spell::Firewall => {
                    let at = clamp_aim(&terrain, tf.translation, look.aim, def.range);
                    let (p, q) = wall_ends(tf.translation, at);
                    decals.line_now(p, q, 0.35, color);
                    decals.ring_now(xz(at), 0.8, color);
                }
                _ => {
                    let at = clamp_aim(&terrain, tf.translation, look.aim, def.range);
                    decals.ring_now(xz(at), def.radius, color);
                    decals.ring_now(xz(at), 0.35, color);
                }
            }
        }
        Slot::Summon(_) => {
            let at = summon_point(&terrain, tf.translation, a.yaw);
            decals.ring_now(xz(at), 1.6, color);
        }
    }
}

/// Moves the camera: behind the player, orbiting the island on the title
/// screen, or circling the fallen altar when the match is over.
pub fn camera(
    time: Res<Time>,
    real: Res<Time<Real>>,
    args: Res<Args>,
    screen: Res<State<Screen>>,
    terrain: Option<Res<Terrain>>,
    game: Option<Res<Match>>,
    mut look: ResMut<Look>,
    mut fx: ResMut<Fx>,
    agents: Query<(&Agent, &Transform)>,
    mut cams: Query<(&mut MainCam, &mut Transform), Without<Agent>>,
    buildings: Query<
        (&Transform, Has<crate::structures::Altar>),
        (
            Without<MainCam>,
            Without<Agent>,
            Or<(With<crate::structures::Altar>, With<crate::structures::Datacenter>)>,
        ),
    >,
) {
    let Some(terrain) = terrain else { return };
    // What the camera must stay above: the ground, and the tops of buildings.
    let roofs: Vec<(Vec2, f32, f32)> = buildings
        .iter()
        .map(|(t, altar)| {
            let (r, h) = if altar { (8.2, 5.4) } else { (2.8, 7.8) };
            (xz(t.translation), r, t.translation.y + h)
        })
        .collect();
    let floor = |p: Vec3| {
        let mut f = terrain.height(p.x, p.z).max(crate::terrain::WATER) + 0.9;
        for (c, r, top) in &roofs {
            if xz(p).distance(*c) < *r {
                f = f.max(*top + 0.5);
            }
        }
        f
    };
    let Ok((mut cam, mut tf)) = cams.single_mut() else {
        return;
    };
    let dt = real.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let player = agents.iter().find(|(a, _)| a.player).or_else(|| {
        // Autopilot and the demo follow Blue.
        agents.iter().find(|(a, _)| a.team == Team::Blue)
    });
    let over = game.as_ref().is_some_and(|g| !g.live());
    let (pos, target, rate) = if *screen.get() == Screen::Title {
        cam.orbit += dt * 0.05;
        let a = cam.orbit + 0.8;
        let r = 150.0;
        let p = Vec3::new(a.cos() * r, 85.0 + (t * 0.1).sin() * 10.0, a.sin() * r);
        (p, Vec3::new(0.0, 4.0, 0.0), 2.0)
    } else if over {
        let loser = game
            .as_ref()
            .and_then(|g| g.winner)
            .map(|w| w.other())
            .unwrap_or(Team::Red);
        let home = agents
            .iter()
            .find(|(a, _)| a.team == loser)
            .map_or(Vec3::ZERO, |(a, _)| a.home);
        cam.orbit += dt * 0.12;
        let p = home + Vec3::new(cam.orbit.cos() * 46.0, 30.0, cam.orbit.sin() * 46.0);
        (p, home + Vec3::Y * 4.0, 1.5)
    } else if let Some((a, atf)) = player {
        if args.autopilot > 0 || !a.player {
            look.yaw = damp(look.yaw, look.yaw + crate::util::wrap_angle(a.yaw - look.yaw), 3.0, dt);
            look.pitch = 0.32;
        }
        if a.dead.is_some() {
            let back = dir_of(look.yaw) * -30.0;
            let p = a.home + Vec3::new(back.x, 30.0, back.y);
            (p, a.home + Vec3::Y * 4.0, 2.0)
        } else {
            let pivot = atf.translation + Vec3::Y * 2.7;
            let fwd = dir_of(look.yaw);
            let right = Vec2::new(-fwd.y, fwd.x);
            let dist = 8.5;
            let back = Vec3::new(fwd.x, 0.0, fwd.y) * -(look.pitch.cos() * dist);
            let up = Vec3::Y * (look.pitch.sin() * dist);
            let side = Vec3::new(right.x, 0.0, right.y) * 1.1;
            let mut p = pivot + back + up + side;
            // A spring arm: pull in toward the agent if anything is in the way.
            let steps = 16;
            for k in 1..=steps {
                let q = pivot.lerp(p, k as f32 / steps as f32);
                if q.y < floor(q) {
                    p = pivot.lerp(p, ((k - 1) as f32 / steps as f32).max(0.25));
                    break;
                }
            }
            p.y = p.y.max(floor(p));
            p.x = p.x.clamp(-HALF + 2.0, HALF - 2.0);
            p.z = p.z.clamp(-HALF + 2.0, HALF - 2.0);
            let aim_dir = Vec3::new(fwd.x * look.pitch.cos(), -look.pitch.sin(), fwd.y * look.pitch.cos());
            let target = p + aim_dir * 30.0;
            (p, target, 22.0)
        }
    } else {
        (cam.pos, cam.look, 2.0)
    };
    if !cam.live {
        cam.pos = pos;
        cam.look = target;
        cam.live = true;
    }
    cam.pos = damp_v(cam.pos, pos, rate, dt);
    // Never inside the ground or a building, whatever the shot.
    let f = floor(cam.pos);
    if cam.pos.y < f {
        cam.pos.y = f;
    }
    cam.look = damp_v(cam.look, target, rate, dt);
    fx.listener = cam.pos;
    let shake = fx.shake;
    let jitter = if shake > 0.01 {
        let rt = real.elapsed_secs();
        Vec3::new((rt * 53.0).sin(), (rt * 71.0 + 1.0).sin(), (rt * 61.0 + 2.0).sin()) * shake * 0.3
    } else {
        Vec3::ZERO
    };
    *tf = Transform::from_translation(cam.pos + jitter).looking_at(cam.look + jitter * 0.5, Vec3::Y);
    if let Some((pos, yaw, pitch)) = args.pose {
        *tf = Transform::from_translation(pos).with_rotation(Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0));
    }
}
