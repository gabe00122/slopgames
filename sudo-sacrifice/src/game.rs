//! Shared game state: screens, teams, health and damage, the match record,
//! the terminal log and the per-frame snapshot of everything that can be hit.

use crate::{spells::Spell, units::UnitKind};
use bevy::prelude::*;

#[derive(States, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum Screen {
    /// Startup: assets are built before anything else is entered.
    #[default]
    Boot,
    /// Title card over a demo match between two CPU agents.
    Title,
    Playing,
    /// The match is decided; the world stays up behind the results.
    Over,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Team {
    Blue,
    Red,
}

impl Team {
    pub const BOTH: [Team; 2] = [Team::Blue, Team::Red];

    pub fn i(self) -> usize {
        match self {
            Team::Blue => 0,
            Team::Red => 1,
        }
    }

    pub fn other(self) -> Team {
        match self {
            Team::Blue => Team::Red,
            Team::Red => Team::Blue,
        }
    }

    /// Bright accent color (sRGB hex).
    pub fn color(self) -> u32 {
        match self {
            Team::Blue => 0x3cc8ff,
            Team::Red => 0xff4d6d,
        }
    }

    /// Dark body color.
    pub fn dark(self) -> u32 {
        match self {
            Team::Blue => 0x223a6b,
            Team::Red => 0x6b2236,
        }
    }

    pub fn agent_name(self) -> &'static str {
        match self {
            Team::Blue => "ORCHESTRATOR",
            Team::Red => "HALLUCINATOR",
        }
    }
}

/// Everything that belongs to one match; cleared when a new match starts.
#[derive(Component)]
pub struct MatchEntity;

#[derive(Component)]
pub struct Health {
    pub hp: f32,
    pub max: f32,
    /// Seconds of hit flash left.
    pub flash: f32,
    /// Seconds since last damaged.
    pub since_hit: f32,
    /// Seconds of invulnerability left (just respawned).
    pub shield: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Health {
            hp: max,
            max,
            flash: 0.0,
            since_hit: 99.0,
            shield: 0.0,
        }
    }

    pub fn frac(&self) -> f32 {
        (self.hp / self.max).clamp(0.0, 1.0)
    }

    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }
}

/// Collision radius.
#[derive(Component, Clone, Copy)]
pub struct Radius(pub f32);

/// Knockback velocity waiting to be applied by whatever moves the entity.
#[derive(Component, Default)]
pub struct Knock(pub Vec3);

/// Damage (or healing, when negative) to one entity.
#[derive(Message, Clone, Copy)]
pub struct Hit {
    pub target: Entity,
    pub amount: f32,
    pub knock: Vec3,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "JUNIOR",
            Difficulty::Normal => "SENIOR",
            Difficulty::Hard => "STAFF",
        }
    }
}

#[derive(Resource, Clone)]
pub struct Settings {
    pub difficulty: Difficulty,
    pub seed: u64,
}

/// The two kinds of thing a slot on the spell bar can hold.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Spell(Spell),
    Summon(UnitKind),
}

pub const SLOTS: [Slot; 10] = [
    Slot::Spell(Spell::Segfault),
    Slot::Spell(Spell::Hotfix),
    Slot::Spell(Spell::Firewall),
    Slot::Spell(Spell::ForcePush),
    Slot::Spell(Spell::ForkBomb),
    Slot::Summon(UnitKind::Linter),
    Slot::Summon(UnitKind::TestRunner),
    Slot::Summon(UnitKind::Fuzzer),
    Slot::Summon(UnitKind::Monolith),
    Slot::Summon(UnitKind::Collector),
];

impl Slot {
    pub fn name(self) -> &'static str {
        match self {
            Slot::Spell(s) => s.def().name,
            Slot::Summon(k) => k.def().name,
        }
    }

    pub fn tokens(self) -> f32 {
        match self {
            Slot::Spell(s) => s.def().cost,
            Slot::Summon(k) => k.def().tokens,
        }
    }

    pub fn software(self) -> usize {
        match self {
            Slot::Spell(_) => 0,
            Slot::Summon(k) => k.def().software,
        }
    }
}

/// A deprecation ritual on an altar.
#[derive(Clone, Copy)]
pub struct Ritual {
    /// The team performing it (the altar belongs to the other team).
    pub by: Team,
    pub timer: f32,
    pub collector: Entity,
}

pub const RITUAL_TIME: f32 = 12.0;

#[derive(Default, Clone)]
pub struct Stats {
    pub summoned: u32,
    pub lost: u32,
    pub sacrificed: u32,
    pub spells: u32,
    pub datacenters: u32,
    pub compactions: u32,
}

#[derive(Resource)]
pub struct Match {
    /// The title screen's CPU-versus-CPU match.
    pub demo: bool,
    pub time: f32,
    pub winner: Option<Team>,
    /// Real seconds since the match was decided.
    pub over_for: f32,
    pub ritual: Option<Ritual>,
    pub stats: [Stats; 2],
    /// Big message for the player: text, seconds left, color.
    pub banner: Option<(String, f32, u32)>,
}

impl Match {
    pub fn new(demo: bool) -> Self {
        Match {
            demo,
            time: 0.0,
            winner: None,
            over_for: 0.0,
            ritual: None,
            stats: [Stats::default(), Stats::default()],
            banner: None,
        }
    }

    pub fn live(&self) -> bool {
        self.winner.is_none()
    }

    pub fn announce(&mut self, text: impl Into<String>, secs: f32, color: u32) {
        self.banner = Some((text.into(), secs, color));
    }
}

// ---------------------------------------------------------------------------
// Terminal log
// ---------------------------------------------------------------------------

pub struct LogLine {
    pub text: String,
    pub color: u32,
    pub age: f32,
}

#[derive(Resource, Default)]
pub struct Log {
    pub lines: Vec<LogLine>,
    /// Lines ever pushed.
    pub total: usize,
}

impl Log {
    pub fn push(&mut self, text: impl Into<String>, color: u32) {
        self.total += 1;
        self.lines.push(LogLine {
            text: text.into(),
            color,
            age: 0.0,
        });
        if self.lines.len() > 40 {
            self.lines.remove(0);
        }
    }
}

pub const LOG_DIM: u32 = 0x8b95a7;
pub const LOG_OK: u32 = 0x7ee787;
pub const LOG_WARN: u32 = 0xf2cc60;
pub const LOG_BAD: u32 = 0xff6b6b;

// ---------------------------------------------------------------------------
// Targets
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Agent,
    Unit(UnitKind),
    Datacenter,
}

#[derive(Clone, Copy)]
pub struct Target {
    pub entity: Entity,
    pub team: Team,
    pub pos: Vec3,
    pub radius: f32,
    pub kind: Kind,
    pub flying: bool,
}

/// Everything that can be attacked, gathered once per frame.
#[derive(Resource, Default)]
pub struct Targets(pub Vec<Target>);

impl Targets {
    pub fn get(&self, e: Entity) -> Option<&Target> {
        self.0.iter().find(|t| t.entity == e)
    }

    /// The nearest enemy of `team` within `range` of `pos`, scored so units
    /// are preferred over buildings. `ground_only` skips fliers.
    pub fn nearest_enemy(&self, team: Team, pos: Vec3, range: f32, ground_only: bool) -> Option<Target> {
        self.0
            .iter()
            .filter(|t| t.team != team && !(ground_only && t.flying))
            .map(|t| {
                let d = t.pos.distance(pos) - t.radius;
                let bias = if t.kind == Kind::Datacenter { 6.0 } else { 0.0 };
                (t, d, d + bias)
            })
            .filter(|(_, d, _)| *d <= range)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(t, _, _)| *t)
    }

    /// Enemies of `team` within `r` of `pos`.
    pub fn enemies_near(&self, team: Team, pos: Vec3, r: f32) -> impl Iterator<Item = &Target> {
        self.0
            .iter()
            .filter(move |t| t.team != team && t.pos.distance(pos) - t.radius <= r)
    }

    pub fn friends_near(&self, team: Team, pos: Vec3, r: f32) -> impl Iterator<Item = &Target> {
        self.0
            .iter()
            .filter(move |t| t.team == team && t.pos.distance(pos) - t.radius <= r)
    }
}

/// Applies damage and healing.
pub fn apply_hits(mut hits: MessageReader<Hit>, mut q: Query<(&mut Health, Option<&mut Knock>)>, time: Res<Time>) {
    let dt = time.delta_secs();
    for (mut h, _) in &mut q {
        h.flash = (h.flash - dt).max(0.0);
        h.shield = (h.shield - dt).max(0.0);
        h.since_hit += dt;
    }
    for hit in hits.read() {
        let Ok((mut h, knock)) = q.get_mut(hit.target) else {
            continue;
        };
        if !h.alive() {
            continue;
        }
        if hit.amount > 0.0 {
            if h.shield > 0.0 {
                continue;
            }
            h.hp -= hit.amount;
            h.flash = 0.15;
            h.since_hit = 0.0;
        } else {
            h.hp = (h.hp - hit.amount).min(h.max);
        }
        if let Some(mut k) = knock {
            k.0 += hit.knock;
        }
    }
}
