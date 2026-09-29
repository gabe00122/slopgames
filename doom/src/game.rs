//! Game simulation: player, weapons, monsters, projectiles, doors, pickups and
//! level progression. Rendering data is produced by `build_frame`.

use crate::levels::LEVELS;
use crate::map::{DoorState, Key, Map, ThingKind};
use crate::math::{hash2, wrap_angle, Rng, V2};
use crate::render::{Frame, Light, Quad, View, FAR};
use crate::sprites::{MonsterSprites, Sprites};
use crate::textures::T_EXIT_ON;

pub const PLAYER_RADIUS: f32 = 0.25;
pub const EYE_HEIGHT: f32 = 0.5;
const USE_RANGE: f32 = 1.3;
const WALK_SPEED: f32 = 3.6;
const RUN_SPEED: f32 = 6.2;
const MAX_LIGHTS: usize = 16;

/// Per-frame player intent, gathered from keyboard and mouse by `main`.
#[derive(Default, Clone, Debug)]
pub struct Input {
    pub forward: f32,
    pub strafe: f32,
    pub turn: f32,
    pub mouse_dx: f32,
    pub run: bool,
    pub fire: bool,
    /// Edge-triggered actions.
    pub use_: bool,
    pub confirm: bool,
    pub automap: bool,
    pub weapon: Option<usize>,
    pub cycle: i32,
}

// ---------------------------------------------------------------------------
// Weapons
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Weapon {
    Pistol,
    Shotgun,
    Chaingun,
    Launcher,
}

impl Weapon {
    pub const ALL: [Weapon; 4] = [Weapon::Pistol, Weapon::Shotgun, Weapon::Chaingun, Weapon::Launcher];

    pub fn index(self) -> usize {
        self as usize
    }

    /// 0 = bullets, 1 = shells, 2 = rockets.
    pub fn ammo_type(self) -> usize {
        match self {
            Weapon::Pistol | Weapon::Chaingun => 0,
            Weapon::Shotgun => 1,
            Weapon::Launcher => 2,
        }
    }

    pub fn cycle_time(self) -> f32 {
        match self {
            Weapon::Pistol => 0.36,
            Weapon::Shotgun => 0.95,
            Weapon::Chaingun => 0.105,
            Weapon::Launcher => 0.75,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum WeaponPhase {
    Ready,
    Firing(f32),
    Lowering(f32),
    Raising(f32),
}

const SWITCH_TIME: f32 = 0.18;

#[derive(Clone)]
struct Loadout {
    health: i32,
    armor: i32,
    armor_class: i32,
    ammo: [i32; 3],
    owned: [bool; 4],
    weapon: Weapon,
}

impl Loadout {
    fn fresh() -> Self {
        Loadout { health: 100, armor: 0, armor_class: 0, ammo: [50, 0, 0], owned: [true, false, false, false], weapon: Weapon::Pistol }
    }
}

pub struct Player {
    pub pos: V2,
    pub vel: V2,
    pub angle: f32,
    pub health: i32,
    pub armor: i32,
    pub armor_class: i32,
    pub ammo: [i32; 3],
    pub max_ammo: [i32; 3],
    pub owned: [bool; 4],
    pub weapon: Weapon,
    pub pending: Option<Weapon>,
    pub phase: WeaponPhase,
    pub refire: bool,
    pub keys: [bool; 3],
    pub bob_phase: f32,
    pub bob: f32,
    pub damage_flash: f32,
    pub bonus_flash: f32,
    pub hazard_t: f32,
    pub dead: bool,
    pub death_t: f32,
    pub face_look: i32,
    pub face_t: f32,
    pub ouch_t: f32,
    pub grin_t: f32,
    pub muzzle_t: f32,
    pub chaingun_frame: usize,
}

impl Player {
    fn new(pos: V2, angle: f32, l: &Loadout) -> Self {
        Player {
            pos,
            vel: V2::ZERO,
            angle,
            health: l.health,
            armor: l.armor,
            armor_class: l.armor_class,
            ammo: l.ammo,
            max_ammo: [200, 50, 50],
            owned: l.owned,
            weapon: l.weapon,
            pending: None,
            phase: WeaponPhase::Raising(0.0),
            refire: false,
            keys: [false; 3],
            bob_phase: 0.0,
            bob: 0.0,
            damage_flash: 0.0,
            bonus_flash: 0.0,
            hazard_t: 0.0,
            dead: false,
            death_t: 0.0,
            face_look: 0,
            face_t: 0.0,
            ouch_t: 0.0,
            grin_t: 0.0,
            muzzle_t: 0.0,
            chaingun_frame: 0,
        }
    }

    fn loadout(&self) -> Loadout {
        Loadout {
            health: self.health,
            armor: self.armor,
            armor_class: self.armor_class,
            ammo: self.ammo,
            owned: self.owned,
            weapon: self.weapon,
        }
    }

    pub fn dir(&self) -> V2 {
        V2::from_angle(self.angle)
    }
}

// ---------------------------------------------------------------------------
// Monsters, projectiles, effects, things
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MonsterKind {
    Zombie,
    Imp,
    Demon,
    Caco,
}

struct MonsterInfo {
    hp: i32,
    speed: f32,
    radius: f32,
    pain_chance: f32,
    /// Melee reach beyond touching distance; 0 = no melee.
    melee: f32,
    ranged: bool,
    reaction: f32,
    scale: f32,
    float: f32,
    attack_len: f32,
    fire_time: f32,
}

fn minfo(k: MonsterKind) -> MonsterInfo {
    match k {
        MonsterKind::Zombie => MonsterInfo {
            hp: 20,
            speed: 1.9,
            radius: 0.28,
            pain_chance: 0.78,
            melee: 0.0,
            ranged: true,
            reaction: 0.6,
            scale: 1.0,
            float: 0.0,
            attack_len: 0.6,
            fire_time: 0.32,
        },
        MonsterKind::Imp => MonsterInfo {
            hp: 60,
            speed: 2.4,
            radius: 0.28,
            pain_chance: 0.78,
            melee: 0.45,
            ranged: true,
            reaction: 0.5,
            scale: 1.0,
            float: 0.0,
            attack_len: 0.72,
            fire_time: 0.5,
        },
        MonsterKind::Demon => MonsterInfo {
            hp: 150,
            speed: 3.7,
            radius: 0.36,
            pain_chance: 0.7,
            melee: 0.5,
            ranged: false,
            reaction: 0.2,
            scale: 1.12,
            float: 0.0,
            attack_len: 0.55,
            fire_time: 0.3,
        },
        MonsterKind::Caco => MonsterInfo {
            hp: 400,
            speed: 2.0,
            radius: 0.42,
            pain_chance: 0.5,
            melee: 0.45,
            ranged: true,
            reaction: 0.6,
            scale: 1.15,
            float: 0.02,
            attack_len: 0.8,
            fire_time: 0.55,
        },
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Ai {
    Idle,
    Chase,
    Attack,
    Pain,
    Dying,
    Dead,
}

#[derive(Clone, Copy, Debug)]
pub struct Monster {
    pub kind: MonsterKind,
    pub pos: V2,
    pub hp: i32,
    pub ai: Ai,
    pub t: f32,
    pub anim: f32,
    pub cooldown: f32,
    pub reaction: f32,
    pub fired: bool,
    pub melee: bool,
    pub sees: bool,
    pub think: f32,
    pub stuck: f32,
    pub detour: Option<(V2, f32)>,
    pub facing: f32,
    pub flash: f32,
    pub seed: u32,
}

impl Monster {
    fn alive(&self) -> bool {
        !matches!(self.ai, Ai::Dying | Ai::Dead)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjKind {
    ImpBall,
    CacoBall,
    Rocket,
}

#[derive(Clone, Copy, Debug)]
pub struct Projectile {
    pub kind: ProjKind,
    pub pos: V2,
    pub vel: V2,
    pub z: f32,
    pub from_player: bool,
    pub t: f32,
    pub alive: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FxKind {
    Puff,
    Blood,
    Explosion,
    Burst,
}

#[derive(Clone, Copy, Debug)]
pub struct Effect {
    pub kind: FxKind,
    pub pos: V2,
    pub z: f32,
    pub t: f32,
}

impl FxKind {
    fn duration(self) -> f32 {
        match self {
            FxKind::Puff => 0.3,
            FxKind::Blood => 0.4,
            FxKind::Explosion => 0.55,
            FxKind::Burst => 0.35,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Decor {
    pub kind: ThingKind,
    pub pos: V2,
    pub hp: i32,
    /// Seconds until a damaged barrel blows; negative = not lit.
    pub fuse: f32,
    pub gone: bool,
}

impl Decor {
    fn radius(&self) -> f32 {
        match self.kind {
            ThingKind::Barrel => 0.26,
            ThingKind::Lamp => 0.16,
            _ => 0.12,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Pickup {
    pub kind: ThingKind,
    pub pos: V2,
    pub taken: bool,
    pub counts: bool,
    pub dropped: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Title,
    Playing,
    Intermission,
    Victory,
}

#[derive(Clone, Default, Debug)]
pub struct Stats {
    pub kills: i32,
    pub total_kills: i32,
    pub items: i32,
    pub total_items: i32,
    pub secrets: i32,
    pub total_secrets: i32,
    pub time: f32,
}

enum Hit {
    Monster(usize),
    Barrel(usize),
}

pub struct Game {
    pub phase: Phase,
    pub paused: bool,
    pub level: usize,
    pub map: Map,
    pub player: Player,
    pub monsters: Vec<Monster>,
    pub projectiles: Vec<Projectile>,
    pub effects: Vec<Effect>,
    pub decor: Vec<Decor>,
    pub pickups: Vec<Pickup>,
    pub rng: Rng,
    pub time: f32,
    pub messages: Vec<(String, f32)>,
    pub seen: Vec<bool>,
    pub show_automap: bool,
    pub stats: Stats,
    /// Set when static map data (textures) changed and must be re-uploaded.
    pub map_dirty: bool,
    pub god: bool,
    pub title_t: f32,
    flow: Vec<u16>,
    flow_cell: (i32, i32),
    flow_timer: f32,
    seen_timer: f32,
    loadout: Loadout,
    exit_timer: Option<f32>,
    noise: bool,
    start_angle: f32,
}

const FLOW_UNREACHED: u16 = u16::MAX;

fn ray_circle(o: V2, d: V2, c: V2, r: f32) -> Option<f32> {
    let oc = c - o;
    let tca = oc.dot(d);
    if tca < 0.0 {
        return None;
    }
    let d2 = oc.len2() - tca * tca;
    if d2 > r * r {
        return None;
    }
    Some((tca - (r * r - d2).sqrt()).max(0.0))
}

impl Game {
    pub fn new(level: usize) -> Self {
        let mut g = Game {
            phase: Phase::Title,
            paused: false,
            level: 0,
            map: Map { w: 1, h: 1, cells: Vec::new(), doors: Vec::new() },
            player: Player::new(V2::ZERO, 0.0, &Loadout::fresh()),
            monsters: Vec::new(),
            projectiles: Vec::new(),
            effects: Vec::new(),
            decor: Vec::new(),
            pickups: Vec::new(),
            rng: Rng::new(0x5eed),
            time: 0.0,
            messages: Vec::new(),
            seen: Vec::new(),
            show_automap: false,
            stats: Stats::default(),
            map_dirty: true,
            god: false,
            title_t: 0.0,
            flow: Vec::new(),
            flow_cell: (-1, -1),
            flow_timer: 0.0,
            seen_timer: 0.0,
            loadout: Loadout::fresh(),
            exit_timer: None,
            noise: false,
            start_angle: 0.0,
        };
        g.load_level(level.min(LEVELS.len() - 1), Loadout::fresh());
        g
    }

    pub fn level_name(&self) -> &'static str {
        LEVELS[self.level].name
    }

    pub fn start_new_game(&mut self, level: usize) {
        self.load_level(level.min(LEVELS.len() - 1), Loadout::fresh());
        self.phase = Phase::Playing;
    }

    fn load_level(&mut self, idx: usize, loadout: Loadout) {
        let def = &LEVELS[idx];
        let (map, things) = Map::parse(def);
        self.level = idx;
        self.monsters.clear();
        self.projectiles.clear();
        self.effects.clear();
        self.decor.clear();
        self.pickups.clear();
        self.messages.clear();
        self.exit_timer = None;
        self.stats = Stats::default();
        self.start_angle = def.start_angle;
        let mut start = V2::new(1.5, 1.5);
        for t in &things {
            use ThingKind::*;
            match t.kind {
                Player => start = t.pos,
                Zombie | Imp | Demon | Caco => {
                    let kind = match t.kind {
                        Zombie => MonsterKind::Zombie,
                        Imp => MonsterKind::Imp,
                        Demon => MonsterKind::Demon,
                        _ => MonsterKind::Caco,
                    };
                    let seed = self.rng.next_u32();
                    self.monsters.push(Monster {
                        kind,
                        pos: t.pos,
                        hp: minfo(kind).hp,
                        ai: Ai::Idle,
                        t: 0.0,
                        anim: 0.0,
                        cooldown: 0.0,
                        reaction: 0.0,
                        fired: false,
                        melee: false,
                        sees: false,
                        think: self.rng.f32() * 0.3,
                        stuck: 0.0,
                        detour: None,
                        facing: (start - t.pos).angle(),
                        flash: 0.0,
                        seed,
                    });
                }
                Barrel | Lamp | TorchRed | TorchBlue | SkullPole => {
                    self.decor.push(Decor { kind: t.kind, pos: t.pos, hp: 20, fuse: -1.0, gone: false })
                }
                _ => self.pickups.push(Pickup { kind: t.kind, pos: t.pos, taken: false, counts: true, dropped: false }),
            }
        }
        // Monsters face the player start by default; fix facing now that `start` is known.
        for m in self.monsters.iter_mut() {
            m.facing = (start - m.pos).angle();
        }
        self.stats.total_kills = self.monsters.len() as i32;
        self.stats.total_items = self.pickups.len() as i32;
        self.stats.total_secrets = map.doors.iter().filter(|d| d.secret).count() as i32;
        self.seen = vec![false; map.cells.len()];
        self.flow = vec![FLOW_UNREACHED; map.cells.len()];
        self.flow_cell = (-1, -1);
        self.map = map;
        self.map_dirty = true;
        self.loadout = loadout.clone();
        self.player = Player::new(start, def.start_angle, &loadout);
        self.msg(format!("Level {}: {}", idx + 1, def.name));
    }

    pub fn msg(&mut self, s: impl Into<String>) {
        self.messages.push((s.into(), 3.5));
        if self.messages.len() > 4 {
            self.messages.remove(0);
        }
    }

    /// Whether the Doom-style status bar is shown (it shrinks the 3D view).
    pub fn shows_statusbar(&self) -> bool {
        self.phase == Phase::Playing
    }

    // -----------------------------------------------------------------------
    // Top-level update
    // -----------------------------------------------------------------------

    pub fn update(&mut self, dt: f32, input: &Input) {
        let dt = dt.min(0.05);
        match self.phase {
            Phase::Title => {
                self.title_t += dt;
                self.time += dt;
                if input.confirm {
                    self.start_new_game(0);
                }
            }
            Phase::Intermission => {
                self.time += dt;
                if input.confirm {
                    if self.level + 1 < LEVELS.len() {
                        let mut l = self.player.loadout();
                        l.health = l.health.max(1);
                        self.load_level(self.level + 1, l);
                        self.phase = Phase::Playing;
                    } else {
                        self.phase = Phase::Victory;
                    }
                }
            }
            Phase::Victory => {
                self.time += dt;
                self.title_t += dt;
                if input.confirm {
                    self.load_level(0, Loadout::fresh());
                    self.phase = Phase::Title;
                }
            }
            Phase::Playing => {
                if self.paused {
                    return;
                }
                self.time += dt;
                self.stats.time += dt;
                if input.automap {
                    self.show_automap = !self.show_automap;
                }
                if self.player.dead {
                    self.player.death_t += dt;
                    if self.player.death_t > 1.2 && input.confirm {
                        let l = self.loadout.clone();
                        self.load_level(self.level, l);
                        return;
                    }
                } else {
                    self.update_player(dt, input);
                    self.update_weapon(dt, input);
                    if input.use_ {
                        self.try_use();
                    }
                }
                self.update_flow(dt);
                self.update_doors(dt);
                self.update_monsters(dt);
                self.update_projectiles(dt);
                self.update_decor(dt);
                self.update_effects(dt);
                self.update_pickups();
                self.update_seen(dt);
                for m in self.messages.iter_mut() {
                    m.1 -= dt;
                }
                self.messages.retain(|m| m.1 > 0.0);
                if let Some(t) = self.exit_timer.as_mut() {
                    *t -= dt;
                    if *t <= 0.0 {
                        self.exit_timer = None;
                        self.phase = Phase::Intermission;
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Player
    // -----------------------------------------------------------------------

    fn update_player(&mut self, dt: f32, input: &Input) {
        let p = &mut self.player;
        p.angle = wrap_angle(p.angle + input.mouse_dx + input.turn * 2.6 * dt);
        let fwd = p.dir();
        let mut wish = fwd * input.forward + fwd.right() * input.strafe;
        if wish.len2() > 1.0 {
            wish = wish.norm();
        }
        let speed = if input.run { RUN_SPEED } else { WALK_SPEED };
        let target = wish * speed;
        let k = 1.0 - (-dt * 11.0).exp();
        p.vel += (target - p.vel) * k;

        let travel = p.vel * dt;
        let steps = (travel.len() / 0.15).ceil().max(1.0) as i32;
        let mut pos = p.pos;
        for _ in 0..steps {
            pos = self.map.collide(pos + travel * (1.0 / steps as f32), PLAYER_RADIUS);
            // Solid things.
            for d in &self.decor {
                if d.gone {
                    continue;
                }
                let r = d.radius() + PLAYER_RADIUS;
                let off = pos - d.pos;
                let l2 = off.len2();
                if l2 < r * r && l2 > 1e-8 {
                    pos = d.pos + off * (r / l2.sqrt());
                }
            }
            for m in &self.monsters {
                if !m.alive() {
                    continue;
                }
                let r = minfo(m.kind).radius + PLAYER_RADIUS;
                let off = pos - m.pos;
                let l2 = off.len2();
                if l2 < r * r && l2 > 1e-8 {
                    pos = m.pos + off * (r / l2.sqrt());
                }
            }
            pos = self.map.collide(pos, PLAYER_RADIUS);
        }
        let p = &mut self.player;
        p.pos = pos;

        let moving = (p.vel.len() / RUN_SPEED).min(1.0);
        p.bob += (moving - p.bob) * (1.0 - (-dt * 8.0).exp());
        p.bob_phase += dt * (6.0 + 5.0 * moving);

        p.damage_flash = (p.damage_flash - dt * 1.2).max(0.0);
        p.bonus_flash = (p.bonus_flash - dt * 1.5).max(0.0);
        p.face_t -= dt;
        p.ouch_t -= dt;
        p.grin_t -= dt;
        p.muzzle_t = (p.muzzle_t - dt).max(0.0);
        if p.face_t <= 0.0 {
            p.face_t = 0.6 + self.rng.f32() * 1.2;
            p.face_look = self.rng.int(-1, 1);
        }

        // Hazardous floors.
        let hz = self.map.cell_at(self.player.pos).hazard;
        if hz > 0 {
            self.player.hazard_t += dt;
            if self.player.hazard_t >= 0.5 {
                self.player.hazard_t -= 0.5;
                let pos = self.player.pos;
                self.damage_player(hz / 2, pos);
            }
        } else {
            self.player.hazard_t = 0.4;
        }
    }

    pub fn damage_player(&mut self, dmg: i32, from: V2) {
        if self.player.dead || self.phase != Phase::Playing {
            return;
        }
        let p = &mut self.player;
        let mut dmg = dmg;
        if self.god {
            dmg = 0;
        }
        if p.armor > 0 {
            let saved = (if p.armor_class >= 2 { dmg / 2 } else { dmg / 3 }).min(p.armor);
            p.armor -= saved;
            dmg -= saved;
            if p.armor == 0 {
                p.armor_class = 0;
            }
        }
        p.health -= dmg;
        p.damage_flash = (p.damage_flash + dmg as f32 * 0.035 + 0.1).min(0.75);
        let rel = wrap_angle((from - p.pos).angle() - p.angle);
        p.face_look = if rel < -0.5 {
            -1
        } else if rel > 0.5 {
            1
        } else {
            0
        };
        p.face_t = 1.0;
        if dmg >= 20 {
            p.ouch_t = 0.8;
        }
        if p.health <= 0 {
            p.health = 0;
            p.dead = true;
            p.death_t = 0.0;
            p.damage_flash = 0.8;
            self.show_automap = false;
        }
    }

    fn try_use(&mut self) {
        let p = &self.player;
        let Some((x, y)) = self.map.use_trace(p.pos, p.dir(), USE_RANGE) else {
            return;
        };
        let cell = *self.map.cell(x, y);
        if let Some(di) = cell.door {
            let door = self.map.doors[di].clone();
            if let Some(k) = door.key
                && !self.player.keys[k as usize] {
                    self.msg(format!("You need a {} key to open this door", k.name()));
                    return;
                }
            if door.secret && door.state == DoorState::Closed {
                self.stats.secrets += 1;
                self.msg("You found a secret area!");
            }
            self.open_door(di);
        } else if cell.exit {
            if let Some(c) = self.map.cell_mut(x, y) {
                c.wall = T_EXIT_ON + 1;
            }
            self.map_dirty = true;
            self.exit_timer = Some(0.7);
            self.player.grin_t = 2.0;
        }
    }

    fn open_door(&mut self, di: usize) {
        let d = &mut self.map.doors[di];
        match d.state {
            DoorState::Closed | DoorState::Closing => d.state = DoorState::Opening,
            DoorState::Open(_) => {
                d.state = DoorState::Open(if d.secret { f32::INFINITY } else { 3.5 });
            }
            DoorState::Opening => {}
        }
    }

    fn door_occupied(&self, x: i32, y: i32) -> bool {
        let cmin = V2::new(x as f32, y as f32);
        let overlaps = |p: V2, r: f32| {
            let q = V2::new(p.x.clamp(cmin.x, cmin.x + 1.0), p.y.clamp(cmin.y, cmin.y + 1.0));
            (p - q).len2() < r * r
        };
        overlaps(self.player.pos, PLAYER_RADIUS)
            || self.monsters.iter().any(|m| m.alive() && overlaps(m.pos, minfo(m.kind).radius))
    }

    fn update_doors(&mut self, dt: f32) {
        for i in 0..self.map.doors.len() {
            let (x, y) = (self.map.doors[i].x, self.map.doors[i].y);
            let occupied = self.door_occupied(x, y);
            let d = &mut self.map.doors[i];
            match d.state {
                DoorState::Closed => {}
                DoorState::Opening => {
                    d.open += dt * 2.2;
                    if d.open >= 1.0 {
                        d.open = 1.0;
                        d.state = DoorState::Open(if d.secret { f32::INFINITY } else { 3.5 });
                    }
                }
                DoorState::Open(t) => {
                    let t = t - dt;
                    d.state = if t > 0.0 {
                        DoorState::Open(t)
                    } else if occupied {
                        DoorState::Open(0.5)
                    } else {
                        DoorState::Closing
                    };
                }
                DoorState::Closing => {
                    if occupied {
                        d.state = DoorState::Opening;
                    } else {
                        d.open -= dt * 2.2;
                        if d.open <= 0.0 {
                            d.open = 0.0;
                            d.state = DoorState::Closed;
                        }
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Weapons
    // -----------------------------------------------------------------------

    fn has_ammo(&self, w: Weapon) -> bool {
        self.player.ammo[w.ammo_type()] > 0
    }

    fn best_weapon(&self) -> Weapon {
        for w in [Weapon::Chaingun, Weapon::Shotgun, Weapon::Pistol, Weapon::Launcher] {
            if self.player.owned[w.index()] && self.has_ammo(w) {
                return w;
            }
        }
        Weapon::Pistol
    }

    fn update_weapon(&mut self, dt: f32, input: &Input) {
        if let Some(i) = input.weapon {
            let w = Weapon::ALL[i.min(3)];
            if self.player.owned[w.index()] && w != self.player.weapon {
                self.player.pending = Some(w);
            }
        }
        if input.cycle != 0 {
            let cur = self.player.pending.unwrap_or(self.player.weapon).index() as i32;
            for k in 1..=4 {
                let idx = (cur + input.cycle * k).rem_euclid(4) as usize;
                if self.player.owned[idx] {
                    if Weapon::ALL[idx] != self.player.weapon {
                        self.player.pending = Some(Weapon::ALL[idx]);
                    }
                    break;
                }
            }
        }

        match self.player.phase {
            WeaponPhase::Lowering(t) => {
                let t = t + dt;
                if t >= SWITCH_TIME {
                    if let Some(w) = self.player.pending.take() {
                        self.player.weapon = w;
                    }
                    self.player.phase = WeaponPhase::Raising(0.0);
                } else {
                    self.player.phase = WeaponPhase::Lowering(t);
                }
            }
            WeaponPhase::Raising(t) => {
                let t = t + dt;
                self.player.phase = if t >= SWITCH_TIME { WeaponPhase::Ready } else { WeaponPhase::Raising(t) };
            }
            WeaponPhase::Firing(t) => {
                let t = t + dt;
                if t >= self.player.weapon.cycle_time() {
                    self.player.phase = WeaponPhase::Ready;
                    self.player.refire = input.fire;
                } else {
                    self.player.phase = WeaponPhase::Firing(t);
                }
            }
            WeaponPhase::Ready => {}
        }

        if self.player.phase == WeaponPhase::Ready {
            if self.player.pending.is_some() {
                self.player.phase = WeaponPhase::Lowering(0.0);
            } else if input.fire {
                if self.has_ammo(self.player.weapon) {
                    self.fire_weapon();
                    self.player.phase = WeaponPhase::Firing(0.0);
                } else {
                    let best = self.best_weapon();
                    if best != self.player.weapon {
                        self.player.pending = Some(best);
                    }
                }
            } else {
                self.player.refire = false;
            }
        }
    }

    fn fire_weapon(&mut self) {
        let w = self.player.weapon;
        self.player.ammo[w.ammo_type()] -= 1;
        self.noise = true;
        let origin = self.player.pos;
        let angle = self.player.angle;
        let accurate = !self.player.refire;
        match w {
            Weapon::Pistol => {
                self.player.muzzle_t = 0.09;
                let spread = if accurate { 0.0 } else { self.rng.range(-0.045, 0.045) };
                let dmg = 5 * self.rng.int(1, 3);
                self.hitscan(origin, angle + spread, 32.0, dmg);
            }
            Weapon::Shotgun => {
                self.player.muzzle_t = 0.12;
                for _ in 0..7 {
                    let spread = self.rng.range(-0.1, 0.1);
                    let dmg = 5 * self.rng.int(1, 3);
                    self.hitscan(origin, angle + spread, 32.0, dmg);
                }
            }
            Weapon::Chaingun => {
                self.player.muzzle_t = 0.07;
                self.player.chaingun_frame ^= 1;
                let spread = if accurate { 0.0 } else { self.rng.range(-0.05, 0.05) };
                let dmg = 5 * self.rng.int(1, 3);
                self.hitscan(origin, angle + spread, 32.0, dmg);
            }
            Weapon::Launcher => {
                self.player.muzzle_t = 0.1;
                let dir = V2::from_angle(angle);
                self.projectiles.push(Projectile {
                    kind: ProjKind::Rocket,
                    pos: origin + dir * 0.3,
                    vel: dir * 12.0,
                    z: 0.42,
                    from_player: true,
                    t: 0.0,
                    alive: true,
                });
            }
        }
        self.player.refire = true;
    }

    fn hitscan(&mut self, origin: V2, angle: f32, range: f32, dmg: i32) {
        let dir = V2::from_angle(angle);
        let wall = self.map.cast(origin, dir, range);
        let mut best = wall.map_or(range, |h| h.dist);
        let mut target = None;
        for (i, m) in self.monsters.iter().enumerate() {
            if m.alive()
                && let Some(t) = ray_circle(origin, dir, m.pos, minfo(m.kind).radius)
                    && t < best {
                        best = t;
                        target = Some(Hit::Monster(i));
                    }
        }
        for (i, d) in self.decor.iter().enumerate() {
            if d.kind == ThingKind::Barrel && !d.gone
                && let Some(t) = ray_circle(origin, dir, d.pos, d.radius())
                    && t < best {
                        best = t;
                        target = Some(Hit::Barrel(i));
                    }
        }
        let at = origin + dir * (best - 0.06);
        let z = self.rng.range(0.32, 0.62);
        match target {
            Some(Hit::Monster(i)) => {
                let mz = 0.4 + minfo(self.monsters[i].kind).float * 10.0;
                let z = mz + self.rng.range(-0.08, 0.1);
                self.spawn_fx(FxKind::Blood, at, z);
                self.damage_monster(i, dmg, origin);
            }
            Some(Hit::Barrel(i)) => {
                self.spawn_fx(FxKind::Puff, at, 0.25);
                self.damage_barrel(i, dmg);
            }
            None if wall.is_some() => self.spawn_fx(FxKind::Puff, at, z),
            None => {}
        }
    }

    fn spawn_fx(&mut self, kind: FxKind, pos: V2, z: f32) {
        self.effects.push(Effect { kind, pos, z, t: 0.0 });
    }

    fn damage_monster(&mut self, i: usize, dmg: i32, from: V2) {
        let info = minfo(self.monsters[i].kind);
        let roll = self.rng.f32();
        let m = &mut self.monsters[i];
        if !m.alive() {
            return;
        }
        m.hp -= dmg;
        // A little knockback sells the hit.
        let push = (m.pos - from).norm() * ((dmg as f32 * 0.004).min(0.12) / info.scale);
        let np = self.map.collide(m.pos + push, info.radius);
        let m = &mut self.monsters[i];
        m.pos = np;
        if m.hp <= 0 {
            m.ai = Ai::Dying;
            m.t = 0.0;
            self.stats.kills += 1;
            if m.kind == MonsterKind::Zombie {
                let pos = m.pos;
                self.pickups.push(Pickup { kind: ThingKind::Clip, pos, taken: false, counts: false, dropped: true });
            }
        } else {
            if m.ai == Ai::Idle {
                m.ai = Ai::Chase;
                m.reaction = 0.0;
            }
            if roll < info.pain_chance && m.ai != Ai::Pain {
                m.ai = Ai::Pain;
                m.t = 0.0;
            }
        }
    }

    fn damage_barrel(&mut self, i: usize, dmg: i32) {
        let d = &mut self.decor[i];
        if d.gone || d.fuse >= 0.0 {
            return;
        }
        d.hp -= dmg;
        if d.hp <= 0 {
            d.fuse = 0.12 + self.rng.f32() * 0.1;
        }
    }

    fn splash(&mut self, pos: V2, radius: f32, max_dmg: f32) {
        for i in 0..self.monsters.len() {
            let m = self.monsters[i];
            if !m.alive() {
                continue;
            }
            let d = (m.pos.dist(pos) - minfo(m.kind).radius).max(0.0);
            if d < radius && self.map.line_of_sight(pos, m.pos) {
                self.damage_monster(i, (max_dmg * (1.0 - d / radius)) as i32, pos);
            }
        }
        let d = (self.player.pos.dist(pos) - PLAYER_RADIUS).max(0.0);
        if d < radius && self.map.line_of_sight(pos, self.player.pos) {
            self.damage_player((max_dmg * (1.0 - d / radius)) as i32, pos);
        }
        for i in 0..self.decor.len() {
            let b = self.decor[i];
            if b.kind == ThingKind::Barrel && !b.gone {
                let d = (b.pos.dist(pos) - b.radius()).max(0.0);
                if d < radius {
                    self.damage_barrel(i, (max_dmg * (1.0 - d / radius)) as i32);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Monsters
    // -----------------------------------------------------------------------

    fn passable_for_monsters(&self, x: i32, y: i32) -> bool {
        let c = self.map.cell(x, y);
        match c.door {
            Some(d) => {
                let d = &self.map.doors[d];
                (d.key.is_none() && !d.secret) || !d.blocks()
            }
            None => c.wall == 0,
        }
    }

    /// Breadth-first distance field from the player's cell, used for pathing.
    fn update_flow(&mut self, dt: f32) {
        self.flow_timer -= dt;
        let pc = (self.player.pos.x.floor() as i32, self.player.pos.y.floor() as i32);
        if pc == self.flow_cell && self.flow_timer > 0.0 {
            return;
        }
        self.flow_cell = pc;
        self.flow_timer = 0.5;
        let w = self.map.w;
        self.flow.fill(FLOW_UNREACHED);
        let mut queue = std::collections::VecDeque::new();
        if self.map.in_bounds(pc.0, pc.1) {
            self.flow[(pc.1 * w + pc.0) as usize] = 0;
            queue.push_back(pc);
        }
        while let Some((x, y)) = queue.pop_front() {
            let d = self.flow[(y * w + x) as usize];
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if !self.map.in_bounds(nx, ny) || !self.passable_for_monsters(nx, ny) {
                    continue;
                }
                let i = (ny * w + nx) as usize;
                if self.flow[i] == FLOW_UNREACHED {
                    self.flow[i] = d + 1;
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    fn flow_at(&self, x: i32, y: i32) -> u16 {
        if self.map.in_bounds(x, y) { self.flow[(y * self.map.w + x) as usize] } else { FLOW_UNREACHED }
    }

    /// Center of the neighbouring cell that is closest to the player.
    fn flow_step(&self, pos: V2) -> Option<(V2, (i32, i32))> {
        let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
        let mut best = self.flow_at(cx, cy);
        let mut out = None;
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (nx, ny) = (cx + dx, cy + dy);
            if dx != 0 && dy != 0 && (!self.passable_for_monsters(cx + dx, cy) || !self.passable_for_monsters(cx, cy + dy)) {
                continue;
            }
            let v = self.flow_at(nx, ny);
            if v < best {
                best = v;
                out = Some((V2::new(nx as f32 + 0.5, ny as f32 + 0.5), (nx, ny)));
            }
        }
        out
    }

    fn update_monsters(&mut self, dt: f32) {
        let ppos = self.player.pos;
        let pdead = self.player.dead;
        for i in 0..self.monsters.len() {
            let mut m = self.monsters[i];
            let info = minfo(m.kind);
            m.t += dt;
            m.flash = (m.flash - dt).max(0.0);
            m.cooldown -= dt;
            m.reaction -= dt;
            let to_player = ppos - m.pos;
            let dist = to_player.len();
            match m.ai {
                Ai::Dead => {}
                Ai::Dying => {
                    if m.t >= 0.6 {
                        m.ai = Ai::Dead;
                    }
                }
                Ai::Pain => {
                    if m.t >= 0.28 {
                        m.ai = Ai::Chase;
                        m.t = 0.0;
                    }
                }
                Ai::Idle => {
                    // Gunfire is heard (every frame) by anything the sound can
                    // path to; sight is checked on a slower think timer.
                    let (cx, cy) = (m.pos.x.floor() as i32, m.pos.y.floor() as i32);
                    let heard = self.noise && self.flow_at(cx, cy) <= 22;
                    m.think -= dt;
                    let mut sees = false;
                    if m.think <= 0.0 {
                        m.think = 0.25 + self.rng.f32() * 0.2;
                        let in_front = wrap_angle(to_player.angle() - m.facing).abs() < 1.9 || dist < 3.0;
                        sees = !pdead && dist < 24.0 && in_front && self.map.line_of_sight(m.pos, ppos);
                    }
                    if sees || heard {
                        m.ai = Ai::Chase;
                        m.reaction = info.reaction * (0.5 + self.rng.f32());
                        m.t = 0.0;
                        m.sees = sees;
                        m.think = 0.0;
                    }
                }
                Ai::Attack => {
                    m.facing = to_player.angle();
                    if !m.fired && m.t >= info.fire_time {
                        m.fired = true;
                        if m.kind == MonsterKind::Zombie {
                            m.flash = 0.12;
                        }
                        self.monster_attack(&m);
                    }
                    if m.t >= info.attack_len {
                        m.ai = Ai::Chase;
                        m.t = 0.0;
                        m.cooldown = match m.kind {
                            MonsterKind::Demon => 0.25,
                            MonsterKind::Zombie => self.rng.range(0.7, 1.8),
                            _ => self.rng.range(0.8, 1.6),
                        };
                    }
                }
                Ai::Chase => {
                    m.think -= dt;
                    if m.think <= 0.0 {
                        m.think = 0.2;
                        m.sees = !pdead && self.map.line_of_sight(m.pos, ppos);
                    }
                    let touch = info.radius + PLAYER_RADIUS;
                    let can_melee = info.melee > 0.0 && dist < touch + info.melee;
                    let mut attacked = false;
                    if m.sees && !pdead && m.reaction <= 0.0 && m.cooldown <= 0.0 {
                        let rate = if dist < 6.0 {
                            1.1
                        } else if dist < 12.0 {
                            0.7
                        } else {
                            0.35
                        };
                        let ranged_go = info.ranged && dist < 22.0 && self.rng.chance(rate * dt);
                        if can_melee || ranged_go {
                            m.ai = Ai::Attack;
                            m.t = 0.0;
                            m.fired = false;
                            m.melee = can_melee;
                            m.facing = to_player.angle();
                            attacked = true;
                        }
                    }
                    if !attacked {
                        self.move_monster(&mut m, dt, dist);
                    }
                }
            }
            self.monsters[i] = m;
        }
        self.separate_monsters();
        self.noise = false;
    }

    fn move_monster(&mut self, m: &mut Monster, dt: f32, dist: f32) {
        let info = minfo(m.kind);
        let ppos = self.player.pos;
        let touch = info.radius + PLAYER_RADIUS;
        let mut goal = None;
        if let Some((g, t)) = m.detour {
            if t > 0.0 {
                goal = Some(g);
                m.detour = Some((g, t - dt));
            } else {
                m.detour = None;
            }
        }
        if goal.is_none() {
            goal = if m.sees && dist < 8.0 {
                Some(ppos)
            } else {
                self.flow_step(m.pos).map(|(g, _)| g).or(if m.sees { Some(ppos) } else { None })
            };
        }
        let Some(g) = goal else { return };
        if m.detour.is_none() && m.sees && dist < touch + 0.05 {
            return;
        }
        let dir = (g - m.pos).norm();
        // Monsters open plain doors that are in their way.
        let ahead = m.pos + dir * (info.radius + 0.35);
        if let Some(di) = self.map.cell_at(ahead).door {
            let d = &self.map.doors[di];
            if d.key.is_none() && !d.secret && d.blocks() {
                self.open_door(di);
            }
        }
        let before = m.pos;
        m.pos = self.map.collide(m.pos + dir * (info.speed * dt), info.radius);
        m.facing = dir.angle();
        m.anim += dt * info.speed * 1.5;
        let moved = m.pos.dist(before);
        if moved < info.speed * dt * 0.3 {
            m.stuck += dt;
            if m.stuck > 0.5 {
                m.stuck = 0.0;
                let a = dir.angle() + if self.rng.chance(0.5) { 1.3 } else { -1.3 } + self.rng.range(-0.4, 0.4);
                m.detour = Some((m.pos + V2::from_angle(a) * 1.5, 0.6));
            }
        } else {
            m.stuck = 0.0;
        }
    }

    fn separate_monsters(&mut self) {
        let n = self.monsters.len();
        for i in 0..n {
            if !self.monsters[i].alive() {
                continue;
            }
            let ri = minfo(self.monsters[i].kind).radius;
            for j in (i + 1)..n {
                if !self.monsters[j].alive() {
                    continue;
                }
                let rj = minfo(self.monsters[j].kind).radius;
                let off = self.monsters[j].pos - self.monsters[i].pos;
                let l2 = off.len2();
                let r = ri + rj;
                if l2 < r * r {
                    let l = l2.sqrt().max(1e-4);
                    let push = if l2 > 1e-8 { off * ((r - l) * 0.5 / l) } else { V2::new(0.01, 0.0) };
                    self.monsters[i].pos -= push;
                    self.monsters[j].pos += push;
                }
            }
            let mut p = self.monsters[i].pos;
            for d in &self.decor {
                if d.gone {
                    continue;
                }
                let r = d.radius() + ri;
                let off = p - d.pos;
                let l2 = off.len2();
                if l2 < r * r && l2 > 1e-8 {
                    p = d.pos + off * (r / l2.sqrt());
                }
            }
            if !self.player.dead {
                let r = PLAYER_RADIUS + ri;
                let off = p - self.player.pos;
                let l2 = off.len2();
                if l2 < r * r && l2 > 1e-8 {
                    p = self.player.pos + off * (r / l2.sqrt());
                }
            }
            self.monsters[i].pos = self.map.collide(p, ri);
        }
    }

    fn monster_attack(&mut self, m: &Monster) {
        let info = minfo(m.kind);
        let ppos = self.player.pos;
        let dist = m.pos.dist(ppos);
        let dir = (ppos - m.pos).norm();
        let in_reach = dist < info.radius + PLAYER_RADIUS + info.melee + 0.25;
        match m.kind {
            MonsterKind::Zombie => {
                if self.map.line_of_sight(m.pos, ppos) {
                    let chance = (0.95 - dist * 0.06).clamp(0.25, 0.8);
                    if self.rng.chance(chance) {
                        let dmg = 3 * self.rng.int(1, 5);
                        self.damage_player(dmg, m.pos);
                    } else {
                        let miss = ppos + dir.right() * self.rng.range(-0.6, 0.6);
                        let a = (miss - m.pos).angle();
                        if let Some(h) = self.map.cast(m.pos, V2::from_angle(a), 32.0) {
                            let at = m.pos + V2::from_angle(a) * (h.dist - 0.06);
                            let z = self.rng.range(0.3, 0.6);
                            self.spawn_fx(FxKind::Puff, at, z);
                        }
                    }
                }
            }
            MonsterKind::Imp | MonsterKind::Caco if !m.melee => {
                let (kind, speed, z) = if m.kind == MonsterKind::Imp {
                    (ProjKind::ImpBall, 6.0, 0.5)
                } else {
                    (ProjKind::CacoBall, 6.0, 0.45)
                };
                self.projectiles.push(Projectile {
                    kind,
                    pos: m.pos + dir * (info.radius + 0.1),
                    vel: dir * speed,
                    z,
                    from_player: false,
                    t: 0.0,
                    alive: true,
                });
            }
            MonsterKind::Imp => {
                if in_reach {
                    let dmg = 3 * self.rng.int(1, 8);
                    self.damage_player(dmg, m.pos);
                }
            }
            MonsterKind::Demon => {
                if in_reach {
                    let dmg = 4 * self.rng.int(1, 10);
                    self.damage_player(dmg, m.pos);
                }
            }
            MonsterKind::Caco => {
                if in_reach {
                    let dmg = 10 * self.rng.int(1, 6);
                    self.damage_player(dmg, m.pos);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Projectiles, barrels, effects, pickups
    // -----------------------------------------------------------------------

    fn update_projectiles(&mut self, dt: f32) {
        for i in 0..self.projectiles.len() {
            let mut p = self.projectiles[i];
            if !p.alive {
                continue;
            }
            p.t += dt;
            let travel = p.vel * dt;
            let steps = (travel.len() / 0.08).ceil().max(1.0) as i32;
            let dir = p.vel.norm();
            let mut boom: Option<(V2, Option<Hit>, bool)> = None;
            'steps: for _ in 0..steps {
                p.pos += travel * (1.0 / steps as f32);
                if self.map.blocked(p.pos, 0.06) {
                    boom = Some((p.pos - dir * 0.12, None, false));
                    break;
                }
                if p.from_player {
                    for (j, m) in self.monsters.iter().enumerate() {
                        if m.alive() && m.pos.dist(p.pos) < minfo(m.kind).radius + 0.1 {
                            boom = Some((p.pos - dir * 0.1, Some(Hit::Monster(j)), false));
                            break 'steps;
                        }
                    }
                } else if !self.player.dead && self.player.pos.dist(p.pos) < PLAYER_RADIUS + 0.12 {
                    boom = Some((p.pos - dir * 0.1, None, true));
                    break;
                }
                for (j, d) in self.decor.iter().enumerate() {
                    if d.kind == ThingKind::Barrel && !d.gone && d.pos.dist(p.pos) < d.radius() + 0.08 {
                        boom = Some((p.pos - dir * 0.1, Some(Hit::Barrel(j)), false));
                        break 'steps;
                    }
                }
            }
            if p.t > 8.0 {
                p.alive = false;
            }
            if let Some((at, hit, hit_player)) = boom {
                p.alive = false;
                let direct = match p.kind {
                    ProjKind::ImpBall => 3 * self.rng.int(1, 8),
                    ProjKind::CacoBall => 5 * self.rng.int(1, 8),
                    ProjKind::Rocket => 20 * self.rng.int(1, 8),
                };
                let src = at - dir;
                match hit {
                    Some(Hit::Monster(j)) => self.damage_monster(j, direct, src),
                    Some(Hit::Barrel(j)) => self.damage_barrel(j, direct),
                    None if hit_player => self.damage_player(direct, src),
                    None => {}
                }
                if p.kind == ProjKind::Rocket {
                    self.spawn_fx(FxKind::Explosion, at, p.z);
                    self.splash(at, 2.2, 128.0);
                } else {
                    self.spawn_fx(FxKind::Burst, at, p.z);
                }
            }
            self.projectiles[i] = p;
        }
        self.projectiles.retain(|p| p.alive);
    }

    fn update_decor(&mut self, dt: f32) {
        for i in 0..self.decor.len() {
            let d = self.decor[i];
            if d.gone || d.fuse < 0.0 {
                continue;
            }
            let fuse = d.fuse - dt;
            if fuse <= 0.0 {
                self.decor[i].gone = true;
                self.spawn_fx(FxKind::Explosion, d.pos, 0.4);
                self.splash(d.pos, 2.2, 110.0);
            } else {
                self.decor[i].fuse = fuse;
            }
        }
    }

    fn update_effects(&mut self, dt: f32) {
        for e in self.effects.iter_mut() {
            e.t += dt;
            if e.kind == FxKind::Blood {
                e.z -= dt * 0.6;
            }
        }
        self.effects.retain(|e| e.t < e.kind.duration());
    }

    fn update_pickups(&mut self) {
        if self.player.dead {
            return;
        }
        for i in 0..self.pickups.len() {
            let pk = self.pickups[i];
            if pk.taken || pk.pos.dist(self.player.pos) > 0.6 {
                continue;
            }
            if let Some(msg) = self.give(pk.kind, pk.dropped) {
                self.pickups[i].taken = true;
                if pk.counts {
                    self.stats.items += 1;
                }
                self.player.bonus_flash = 0.35;
                self.msg(msg);
            }
        }
    }

    fn give(&mut self, kind: ThingKind, dropped: bool) -> Option<&'static str> {
        use ThingKind::*;
        let p = &mut self.player;
        let add_ammo = |p: &mut crate::game::Player, t: usize, n: i32| -> bool {
            if p.ammo[t] >= p.max_ammo[t] {
                return false;
            }
            p.ammo[t] = (p.ammo[t] + n).min(p.max_ammo[t]);
            true
        };
        let got_weapon = |p: &mut crate::game::Player, w: Weapon, t: usize, n: i32| -> bool {
            let new = !p.owned[w.index()];
            let ammo = add_ammo(p, t, n);
            if new {
                p.owned[w.index()] = true;
                p.pending = Some(w);
                p.grin_t = 1.5;
            }
            new || ammo
        };
        match kind {
            Stimpack if p.health < 100 => {
                p.health = (p.health + 10).min(100);
                Some("Picked up a stimpack.")
            }
            Medikit if p.health < 100 => {
                let need = p.health < 25;
                p.health = (p.health + 25).min(100);
                Some(if need { "Picked up a medikit that you REALLY need!" } else { "Picked up a medikit." })
            }
            Potion => {
                p.health = (p.health + 2).min(200);
                Some("Picked up a health bonus.")
            }
            Soulsphere => {
                p.health = (p.health + 100).min(200);
                Some("Supercharge!")
            }
            GreenArmor if p.armor < 100 => {
                p.armor = 100;
                p.armor_class = 1;
                Some("Picked up the armor.")
            }
            BlueArmor if p.armor < 200 => {
                p.armor = 200;
                p.armor_class = 2;
                Some("Picked up the MegaArmor!")
            }
            Clip if add_ammo(p, 0, if dropped { 5 } else { 10 }) => Some("Picked up a clip."),
            Shells if add_ammo(p, 1, 4) => Some("Picked up 4 shotgun shells."),
            ShellBox if add_ammo(p, 1, 20) => Some("Picked up a box of shotgun shells."),
            RocketAmmo if add_ammo(p, 2, 1) => Some("Picked up a rocket."),
            RocketBox if add_ammo(p, 2, 5) => Some("Picked up a box of rockets."),
            Shotgun if got_weapon(p, Weapon::Shotgun, 1, 8) => Some("You got the shotgun!"),
            Chaingun if got_weapon(p, Weapon::Chaingun, 0, 20) => Some("You got the chaingun!"),
            Launcher if got_weapon(p, Weapon::Launcher, 2, 2) => Some("You got the rocket launcher!"),
            KeyRed | KeyBlue | KeyYellow => {
                let k = match kind {
                    KeyRed => Key::Red,
                    KeyBlue => Key::Blue,
                    _ => Key::Yellow,
                };
                p.keys[k as usize] = true;
                Some(match k {
                    Key::Red => "Picked up a red keycard.",
                    Key::Blue => "Picked up a blue keycard.",
                    Key::Yellow => "Picked up a yellow keycard.",
                })
            }
            _ => None,
        }
    }

    /// Marks cells visible from the player for the automap.
    fn update_seen(&mut self, dt: f32) {
        self.seen_timer -= dt;
        if self.seen_timer > 0.0 {
            return;
        }
        self.seen_timer = 0.1;
        let o = self.player.pos;
        for k in 0..48 {
            let a = self.player.angle + (k as f32 / 47.0 - 0.5) * 1.9;
            let dir = V2::from_angle(a);
            let hit = self.map.cast(o, dir, 24.0);
            let len = hit.map_or(24.0, |h| h.dist);
            let mut t = 0.0;
            while t < len {
                let p = o + dir * t;
                let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
                if self.map.in_bounds(x, y) {
                    self.seen[(y * self.map.w + x) as usize] = true;
                }
                t += 0.3;
            }
            if let Some(h) = hit
                && self.map.in_bounds(h.x, h.y) {
                    self.seen[(h.y * self.map.w + h.x) as usize] = true;
                }
        }
    }

    // -----------------------------------------------------------------------
    // Rendering data
    // -----------------------------------------------------------------------

    pub fn camera(&self) -> (V2, f32, f32) {
        match self.phase {
            Phase::Title | Phase::Victory => {
                let p = self.player.pos;
                (p, self.start_angle + (self.title_t * 0.12).sin() * 1.2, EYE_HEIGHT)
            }
            _ => {
                let p = &self.player;
                let eye = if p.dead {
                    EYE_HEIGHT - (p.death_t * 1.6).min(1.0) * 0.36
                } else {
                    EYE_HEIGHT + (p.bob_phase * 2.0).sin() * 0.018 * p.bob
                };
                (p.pos, p.angle, eye)
            }
        }
    }

    /// Sector light with flicker, matching the shader.
    fn sector_light(&self, pos: V2) -> f32 {
        let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
        let c = self.map.cell(x, y);
        let mut l = c.light as f32 / 255.0;
        if c.flicker && hash2(x, y, (self.time * 10.0) as u32) > 0.6 {
            l *= 0.45;
        }
        l
    }

    /// CPU copy of the shader's lighting so sprites match walls.
    fn light_at(&self, pos: V2, z: f32, dist: f32, lights: &[Light]) -> [f32; 3] {
        let sector = self.sector_light(pos);
        let fall = 1.0 / (1.0 + 0.018 * dist * dist);
        let b = (sector * (0.3 + 0.85 * fall) * 20.0 + 0.5).floor() / 20.0;
        let mut c = [b; 3];
        for l in lights {
            let d = ((pos.x - l.pos[0]).powi(2) + (pos.y - l.pos[1]).powi(2) + (z - l.pos[2]).powi(2)).sqrt();
            let k = (1.0 - d / l.radius).max(0.0);
            let k = k * k;
            for i in 0..3 {
                c[i] += l.color[i] * k;
            }
        }
        c.map(|v| v.min(1.6))
    }

    fn collect_lights(&self) -> Vec<Light> {
        let mut v: Vec<Light> = Vec::new();
        let flicker = |seed: f32| 0.85 + 0.15 * ((self.time * 13.0 + seed).sin() * (self.time * 7.3 + seed * 2.0).sin());
        if self.player.muzzle_t > 0.0 {
            let d = self.player.dir();
            let p = self.player.pos + d * 0.6;
            v.push(Light { pos: [p.x, p.y, 0.5], radius: 6.0, color: [1.0, 0.75, 0.45] });
        }
        for m in &self.monsters {
            if m.flash > 0.0 {
                v.push(Light { pos: [m.pos.x, m.pos.y, 0.5], radius: 4.0, color: [0.9, 0.65, 0.35] });
            }
        }
        for p in &self.projectiles {
            let color = match p.kind {
                ProjKind::ImpBall => [1.0, 0.45, 0.1],
                ProjKind::CacoBall => [0.8, 0.3, 1.0],
                ProjKind::Rocket => [1.0, 0.6, 0.25],
            };
            v.push(Light { pos: [p.pos.x, p.pos.y, p.z], radius: 3.2, color });
        }
        for e in &self.effects {
            if matches!(e.kind, FxKind::Explosion | FxKind::Burst) {
                let k = 1.0 - e.t / e.kind.duration();
                let r = if e.kind == FxKind::Explosion { 6.0 } else { 3.0 };
                v.push(Light { pos: [e.pos.x, e.pos.y, e.z], radius: r, color: [1.3 * k, 0.8 * k, 0.35 * k] });
            }
        }
        for (i, d) in self.decor.iter().enumerate() {
            if d.gone {
                continue;
            }
            let f = flicker(i as f32 * 1.7);
            match d.kind {
                ThingKind::TorchRed => v.push(Light { pos: [d.pos.x, d.pos.y, 0.6], radius: 3.2, color: [0.9 * f, 0.35 * f, 0.1 * f] }),
                ThingKind::TorchBlue => v.push(Light { pos: [d.pos.x, d.pos.y, 0.6], radius: 3.2, color: [0.2 * f, 0.35 * f, 0.9 * f] }),
                ThingKind::Lamp => v.push(Light { pos: [d.pos.x, d.pos.y, 0.85], radius: 4.0, color: [0.55, 0.6, 0.7] }),
                _ => {}
            }
        }
        // Keep the lights that matter most to the viewer.
        let (cam, _, _) = self.camera();
        v.sort_by(|a, b| {
            let da = V2::new(a.pos[0], a.pos[1]).dist(cam) - a.radius;
            let db = V2::new(b.pos[0], b.pos[1]).dist(cam) - b.radius;
            da.total_cmp(&db)
        });
        v.truncate(MAX_LIGHTS);
        v
    }

    fn monster_layer(&self, m: &Monster, s: &MonsterSprites) -> u32 {
        let info = minfo(m.kind);
        match m.ai {
            Ai::Idle => s.walk[0],
            Ai::Chase => s.walk[(m.anim as usize) % s.walk.len()],
            Ai::Attack => {
                let n = s.attack.len();
                let f = match m.kind {
                    MonsterKind::Zombie => usize::from(m.t >= info.fire_time && m.t < info.fire_time + 0.12),
                    _ => ((m.t / info.attack_len) * n as f32) as usize,
                };
                s.attack[f.min(n - 1)]
            }
            Ai::Pain => s.pain,
            Ai::Dying => s.death[((m.t / 0.12) as usize).min(s.death.len() - 1)],
            Ai::Dead => *s.death.last().unwrap(),
        }
    }

    pub fn build_frame(&self, view: &View, spr: &Sprites) -> Frame {
        let (cam, angle, eye) = self.camera();
        let lights = self.collect_lights();
        let dir = V2::from_angle(angle);
        let right = dir.right();
        let mut sprites = Vec::new();

        // (position, z of the bottom edge, world size, layer, fullbright)
        let mut push = |pos: V2, z0: f32, size: f32, layer: u32, bright: bool| {
            let rel = pos - cam;
            let fwd = rel.dot(dir);
            if fwd < 0.08 || fwd > FAR - 1.0 {
                return;
            }
            let lat = rel.dot(right);
            let cx = lat / (fwd * view.tan_h);
            let hw = size * 0.5 / (fwd * view.tan_h);
            if cx + hw < -1.0 || cx - hw > 1.0 {
                return;
            }
            let yb = (z0 - eye) / (fwd * view.tan_v);
            let yt = (z0 + size - eye) / (fwd * view.tan_v);
            let c = if bright { [1.0; 3] } else { self.light_at(pos, z0 + size * 0.4, fwd, &lights) };
            sprites.push(Quad {
                rect: [cx - hw, yt, cx + hw, yb],
                uv: [0.0, 0.0, 1.0, 1.0],
                color: [c[0], c[1], c[2], 1.0],
                params: [layer as f32, fwd / FAR, 0.0, 0.0],
            });
        };

        let blink = (self.time * 4.0) as usize % 2;
        for pk in &self.pickups {
            if pk.taken {
                continue;
            }
            use ThingKind::*;
            let layer = match pk.kind {
                Stimpack => spr.stimpack,
                Medikit => spr.medikit,
                Potion => spr.potion[blink],
                Soulsphere => spr.soulsphere[blink],
                GreenArmor => spr.green_armor,
                BlueArmor => spr.blue_armor,
                Clip => spr.clip,
                Shells => spr.shells,
                ShellBox => spr.shell_box,
                RocketAmmo => spr.rocket_ammo,
                RocketBox => spr.rocket_box,
                Shotgun => spr.shotgun_pickup,
                Chaingun => spr.chaingun_pickup,
                Launcher => spr.launcher_pickup,
                KeyRed => spr.keys[0][blink],
                KeyBlue => spr.keys[1][blink],
                KeyYellow => spr.keys[2][blink],
                _ => continue,
            };
            let bob = if pk.kind == Soulsphere { (self.time * 3.0).sin() * 0.03 } else { 0.0 };
            push(pk.pos, bob, 1.3, layer, pk.kind == Soulsphere);
        }
        for d in &self.decor {
            if d.gone {
                continue;
            }
            let f = ((self.time * 10.0) as usize + d.pos.x as usize) % 3;
            let (layer, size) = match d.kind {
                ThingKind::Barrel => (spr.barrel[(self.time * 3.0) as usize % 2], 1.1),
                ThingKind::Lamp => (spr.lamp, 1.0),
                ThingKind::TorchRed => (spr.torch_red[f], 1.0),
                ThingKind::TorchBlue => (spr.torch_blue[f], 1.0),
                _ => (spr.skull_pole, 1.0),
            };
            push(d.pos, 0.0, size, layer, false);
        }
        for m in &self.monsters {
            let s = match m.kind {
                MonsterKind::Zombie => &spr.zombie,
                MonsterKind::Imp => &spr.imp,
                MonsterKind::Demon => &spr.demon,
                MonsterKind::Caco => &spr.caco,
            };
            let info = minfo(m.kind);
            let float = if m.kind == MonsterKind::Caco && m.alive() {
                info.float + (self.time * 2.0 + m.seed as f32).sin() * 0.03
            } else {
                0.0
            };
            push(m.pos, float, info.scale, self.monster_layer(m, s), false);
        }
        let frame2 = (self.time * 12.0) as usize % 2;
        for p in &self.projectiles {
            let (layer, size) = match p.kind {
                ProjKind::ImpBall => (spr.imp_ball[frame2], 1.0),
                ProjKind::CacoBall => (spr.caco_ball[frame2], 1.0),
                ProjKind::Rocket => (spr.rocket[frame2], 1.0),
            };
            push(p.pos, p.z - size * 0.5, size, layer, true);
        }
        for e in &self.effects {
            let k = e.t / e.kind.duration();
            let (frames, size, bright) = match e.kind {
                FxKind::Puff => (&spr.puff, 0.8, false),
                FxKind::Blood => (&spr.blood, 0.8, false),
                FxKind::Explosion => (&spr.explosion, 1.6, true),
                FxKind::Burst => (&spr.explosion, 0.8, true),
            };
            let layer = frames[((k * frames.len() as f32) as usize).min(frames.len() - 1)];
            push(e.pos, e.z - size * 0.5, size, layer, bright);
        }

        let ui = crate::hud::build(self, view, spr, &lights);

        let p = &self.player;
        let mut tint = [0.0f32; 4];
        if self.phase == Phase::Playing {
            if p.damage_flash > 0.0 {
                tint = [0.8, 0.0, 0.0, p.damage_flash.min(0.6)];
            } else if p.bonus_flash > 0.0 {
                tint = [0.85, 0.75, 0.3, p.bonus_flash * 0.5];
            }
            if p.dead {
                tint = [0.45, 0.0, 0.0, (0.25 + p.death_t * 0.2).min(0.5)];
            }
        }
        let hazard = self.map.cell_at(p.pos).hazard > 0 && self.phase == Phase::Playing && !p.dead;
        if hazard && tint[3] < 0.12 {
            tint = [0.1, 0.8, 0.1, 0.12];
        }
        Frame { cam_pos: cam, cam_angle: angle, eye_z: eye, time: self.time, lights, sprites, ui, tint }
    }

    /// Light on the first-person weapon (sector light where the player stands).
    pub fn weapon_light(&self, lights: &[Light]) -> [f32; 3] {
        self.light_at(self.player.pos, 0.5, 0.0, lights)
    }

    /// Automap helper: whether a cell is known to the player.
    pub fn cell_seen(&self, x: i32, y: i32) -> bool {
        self.map.in_bounds(x, y) && self.seen[(y * self.map.w + x) as usize]
    }
}
