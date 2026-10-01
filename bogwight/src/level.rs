//! The swamp's layout: a heightfield of mud banks, deep pools and reedy
//! marsh generated from a seed, and the plan of who and what lives where.
//! Spawning the plan into entities is `world.rs`'s job.

use crate::{
    game::WATER_Y,
    hunters::HunterKind,
    util::{Rng, fbm2, lerp, noise2, smooth},
};
use bevy::prelude::*;

/// Heightfield sample spacing.
pub const DX: f32 = 0.25;

#[derive(Resource, Clone)]
pub struct Terrain {
    pub h: Vec<f32>,
    pub len: f32,
}

impl Terrain {
    pub fn height(&self, x: f32) -> f32 {
        let f = (x / DX).clamp(0.0, (self.h.len() - 1) as f32);
        let i = (f.floor() as usize).min(self.h.len() - 2);
        lerp(self.h[i], self.h[i + 1], f - i as f32)
    }

    /// How deep the still water is here (0 on dry land).
    pub fn depth(&self, x: f32) -> f32 {
        (WATER_Y - self.height(x)).max(0.0)
    }

    /// The point on the ground under `x`.
    pub fn ground(&self, x: f32) -> Vec2 {
        Vec2::new(x, self.height(x))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// The deep pool the bogwight starts in.
    Lair,
    Bank,
    Camp,
    Pool,
    BridgePool,
    DockPool,
    Marsh,
    FinalCamp,
}

impl Kind {
    pub fn wet(self) -> bool {
        matches!(
            self,
            Kind::Lair | Kind::Pool | Kind::BridgePool | Kind::DockPool | Kind::Marsh
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Lair => "the lair",
            Kind::Bank => "a mud bank",
            Kind::Camp => "a hunters' camp",
            Kind::Pool => "a deep pool",
            Kind::BridgePool => "a rope bridge",
            Kind::DockPool => "a stilt dock",
            Kind::Marsh => "the reeds",
            Kind::FinalCamp => "the warden's camp",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Section {
    pub kind: Kind,
    pub x0: f32,
    pub x1: f32,
    /// Land height for dry sections, water depth for wet ones.
    pub level: f32,
}

impl Section {
    pub fn mid(&self) -> f32 {
        (self.x0 + self.x1) * 0.5
    }
    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }
}

pub struct HunterSpawn {
    pub kind: HunterKind,
    pub pos: Vec2,
    pub patrol: (f32, f32),
    pub boat: Option<usize>,
    pub facing: f32,
    /// Stands still, watching `facing`.
    pub guard: bool,
}

pub struct BoatSpawn {
    pub x: f32,
    pub patrol: (f32, f32),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PropKind {
    Barrel,
    Crate,
    Log,
    LilyPad,
}

pub struct PropSpawn {
    pub kind: PropKind,
    pub pos: Vec2,
    pub angle: f32,
}

pub struct Bridge {
    pub a: Vec2,
    pub b: Vec2,
}

pub struct Dock {
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
}

/// A decoration: `z` is depth into the screen (negative is behind the playfield).
#[derive(Clone, Copy)]
pub struct Decor {
    pub x: f32,
    pub z: f32,
    pub scale: f32,
    pub variant: usize,
}

pub struct Plan {
    pub sections: Vec<Section>,
    pub terrain: Terrain,
    pub start: Vec2,
    pub hunters: Vec<HunterSpawn>,
    pub boats: Vec<BoatSpawn>,
    pub props: Vec<PropSpawn>,
    pub bridges: Vec<Bridge>,
    pub docks: Vec<Dock>,
    /// Posts with a lantern hanging from a rope: (ground point, arm direction).
    pub lantern_posts: Vec<(Vec2, f32)>,
    pub campfires: Vec<Vec2>,
    /// Tents: x and depth.
    pub tents: Vec<(f32, f32)>,
    /// Bouncy mushrooms: ground point and size.
    pub mushrooms: Vec<(Vec2, f32)>,
    /// Glowing fungus clusters on pool floors: x and depth.
    pub fungi: Vec<(f32, f32)>,
    /// Boulders: center, radius. The ones on z = 0 are solid.
    pub rocks: Vec<(Vec2, f32)>,
    pub trees: Vec<Decor>,
    pub reeds: Vec<Decor>,
    pub weeds: Vec<Decor>,
    /// Stretches where reeds stand in front of the playfield and hide the bogwight.
    pub reed_cover: Vec<(f32, f32)>,
    /// Firefly swarms: center.
    pub fireflies: Vec<Vec2>,
}

fn land_height(sec: &Section, x: f32, seed: u32) -> f32 {
    let amp = if matches!(sec.kind, Kind::Camp | Kind::FinalCamp) {
        0.35
    } else {
        1.1
    };
    sec.level + (fbm2(x * 0.07, 3.7, 3, seed) - 0.5) * amp + (noise2(x * 0.7, 1.3, seed) - 0.5) * 0.14
}

/// One side of a pool: from the bank height down a gentle shelf, then a drop
/// to the floor. `d` is the distance from the bank edge.
fn bank_profile(d: f32, bank: f32, floor: f32, shelf_w: f32, drop_w: f32) -> f32 {
    const SHELF: f32 = -0.65;
    if d < shelf_w {
        lerp(bank, SHELF, smooth((d / shelf_w).clamp(0.0, 1.0)))
    } else if d < shelf_w + drop_w {
        lerp(SHELF, floor, smooth(((d - shelf_w) / drop_w).clamp(0.0, 1.0)))
    } else {
        floor
    }
}

pub fn plan(seed: u64, night: u32) -> Plan {
    let s32 = seed as u32;
    let mut rng = Rng::new(seed ^ 0xb0_9e);
    let mut sections = vec![Section {
        kind: Kind::Lair,
        x0: 0.0,
        x1: 40.0,
        level: 8.5,
    }];
    let pairs = 3 + night.min(4) as usize;
    let mut x = 40.0;
    let mut last_wet = Kind::Lair;
    for k in 0..pairs {
        let camp = k % 2 == 1 || rng.chance(0.25);
        let (kind, w) = if camp {
            (Kind::Camp, rng.range(26.0, 32.0))
        } else {
            (Kind::Bank, rng.range(16.0, 24.0))
        };
        let level = rng.range(1.0, 2.1);
        sections.push(Section {
            kind,
            x0: x,
            x1: x + w,
            level,
        });
        x += w;
        let wet = loop {
            let r = rng.f();
            let pick = if r < 0.34 {
                Kind::Pool
            } else if r < 0.56 {
                Kind::BridgePool
            } else if r < 0.78 {
                Kind::DockPool
            } else {
                Kind::Marsh
            };
            if pick != last_wet {
                break pick;
            }
        };
        last_wet = wet;
        let (w, depth) = match wet {
            Kind::Pool => (rng.range(30.0, 40.0), rng.range(6.5, 8.5)),
            Kind::BridgePool => (rng.range(14.0, 18.0), rng.range(5.5, 7.0)),
            Kind::DockPool => (rng.range(24.0, 30.0), rng.range(5.5, 7.0)),
            _ => (rng.range(26.0, 32.0), rng.range(0.9, 1.15)),
        };
        sections.push(Section {
            kind: wet,
            x0: x,
            x1: x + w,
            level: depth,
        });
        x += w;
    }
    sections.push(Section {
        kind: Kind::FinalCamp,
        x0: x,
        x1: x + 44.0,
        level: 1.5,
    });
    let len = x + 44.0;

    // The heightfield.
    let n = (len / DX) as usize + 1;
    let mut h = vec![0.0; n];
    for (i, hv) in h.iter_mut().enumerate() {
        let x = i as f32 * DX;
        let si = sections.iter().position(|s| x < s.x1).unwrap_or(sections.len() - 1);
        let sec = &sections[si];
        let floor_noise = (fbm2(x * 0.11, 9.1, 3, s32) - 0.5) * 2.0;
        *hv = match sec.kind {
            Kind::Bank | Kind::Camp => land_height(sec, x, s32),
            Kind::FinalCamp => {
                let base = land_height(sec, x, s32);
                // A cliff closes the swamp on the right.
                let d = sec.x1 - x;
                lerp(9.0, base, smooth(((d - 1.0) / 4.0).clamp(0.0, 1.0)))
            }
            Kind::Lair => {
                let hr = land_height(&sections[si + 1], sec.x1, s32);
                let floor = -sec.level + floor_noise;
                // A cliff on the left, a shelf out on the right.
                let left = if x < 3.0 {
                    9.0
                } else {
                    lerp(9.0, floor, smooth(((x - 3.0) / 5.0).clamp(0.0, 1.0)))
                };
                left.max(bank_profile(sec.x1 - x, hr, floor, 4.0, 4.0))
            }
            wet => {
                let hl = land_height(&sections[si - 1], sec.x0, s32);
                let hr = land_height(&sections[si + 1], sec.x1, s32);
                let (dl, dr) = (x - sec.x0, sec.x1 - x);
                match wet {
                    Kind::Marsh => {
                        let islets = ((noise2(x * 0.16, 4.2, s32) - 0.6).max(0.0) * 5.5).min(1.8);
                        let floor = -sec.level + floor_noise * 0.3 + islets;
                        bank_profile(dl, hl, floor, 4.0, 1.0).max(bank_profile(dr, hr, floor, 4.0, 1.0))
                    }
                    Kind::BridgePool => {
                        let floor = -sec.level + floor_noise;
                        bank_profile(dl, hl, floor, 1.0, 2.5).max(bank_profile(dr, hr, floor, 1.0, 2.5))
                    }
                    _ => {
                        let floor = -sec.level + floor_noise;
                        bank_profile(dl, hl, floor, 3.0, 3.5).max(bank_profile(dr, hr, floor, 3.0, 3.5))
                    }
                }
            }
        };
    }
    let terrain = Terrain { h, len };

    let mut plan = Plan {
        sections: sections.clone(),
        start: Vec2::new(17.0, -4.0),
        terrain,
        hunters: vec![],
        boats: vec![],
        props: vec![],
        bridges: vec![],
        docks: vec![],
        lantern_posts: vec![],
        campfires: vec![],
        tents: vec![],
        mushrooms: vec![],
        fungi: vec![],
        rocks: vec![],
        trees: vec![],
        reeds: vec![],
        weeds: vec![],
        reed_cover: vec![],
        fireflies: vec![],
    };
    for (i, sec) in sections.iter().enumerate() {
        populate(&mut plan, i, sec, night, &mut rng);
    }
    shore_reeds(&mut plan, &mut rng);
    plan
}

fn hunter_kind(rng: &mut Rng, night: u32) -> HunterKind {
    let crossbow = 0.3 + 0.08 * night.min(4) as f32;
    if rng.chance(crossbow) {
        HunterKind::Crossbow
    } else {
        HunterKind::Lantern
    }
}

fn populate(plan: &mut Plan, index: usize, sec: &Section, night: u32, rng: &mut Rng) {
    let t = plan.terrain.clone();
    let (x0, x1) = (sec.x0, sec.x1);
    let w = sec.width();
    let foot = |x: f32| Vec2::new(x, t.height(x) + 0.85);
    // Trees behind the playfield everywhere, thinner over deep water.
    let trees = (w / if sec.kind.wet() { 11.0 } else { 6.5 }) as usize + 1;
    for _ in 0..trees {
        let x = rng.range(x0, x1);
        if sec.kind.wet() && t.depth(x) > 3.0 && rng.chance(0.6) {
            continue;
        }
        plan.trees.push(Decor {
            x,
            z: rng.range(-1.8, -4.6),
            scale: rng.range(0.8, 1.3),
            variant: rng.below(4),
        });
    }
    match sec.kind {
        Kind::Lair => {
            for _ in 0..9 {
                let x = rng.range(6.0, 34.0);
                plan.fungi.push((x, rng.range(-0.5, -4.0)));
            }
            plan.rocks
                .push((Vec2::new(rng.range(9.0, 13.0), t.height(11.0) + 0.3), 1.1));
            plan.rocks
                .push((Vec2::new(rng.range(24.0, 29.0), t.height(26.0) + 0.2), 0.9));
            plan.props.push(PropSpawn {
                kind: PropKind::Log,
                pos: Vec2::new(22.0, 0.2),
                angle: 0.05,
            });
            for k in 0..4 {
                plan.props.push(PropSpawn {
                    kind: PropKind::LilyPad,
                    pos: Vec2::new(12.0 + k as f32 * 5.3 + rng.sym(1.0), 0.05),
                    angle: 0.0,
                });
            }
            weeds(plan, x0 + 6.0, x1 - 6.0, 10, rng);
            plan.trees.push(Decor {
                x: 1.5,
                z: -2.0,
                scale: 1.2,
                variant: 0,
            });
        }
        Kind::Bank => {
            let a = x0 + 3.5;
            let b = x1 - 3.5;
            let x = rng.range(a, b);
            plan.hunters.push(HunterSpawn {
                kind: hunter_kind(rng, night),
                pos: foot(x),
                patrol: (a, b),
                boat: None,
                facing: if rng.chance(0.5) { 1.0 } else { -1.0 },
                guard: false,
            });
            if night >= 2 && rng.chance(0.5) {
                let x = rng.range(a, b);
                plan.hunters.push(HunterSpawn {
                    kind: hunter_kind(rng, night),
                    pos: foot(x),
                    patrol: (a, b),
                    boat: None,
                    facing: 1.0,
                    guard: false,
                });
            }
            if rng.chance(0.6) {
                let x = rng.range(a, b);
                plan.props.push(PropSpawn {
                    kind: if rng.chance(0.5) {
                        PropKind::Crate
                    } else {
                        PropKind::Barrel
                    },
                    pos: Vec2::new(x, t.height(x) + 0.6),
                    angle: 0.0,
                });
            }
            if rng.chance(0.45) {
                let x = rng.range(a, b);
                plan.mushrooms.push((t.ground(x), rng.range(0.9, 1.2)));
            }
            if rng.chance(0.5) {
                plan.fireflies
                    .push(Vec2::new(rng.range(x0, x1), t.height(sec.mid()) + 2.0));
            }
        }
        Kind::Camp | Kind::FinalCamp => {
            let final_camp = sec.kind == Kind::FinalCamp;
            let (a, b) = (x0 + 3.0, x1 - if final_camp { 8.0 } else { 3.0 });
            let fire = rng.range(a + w * 0.3, b - w * 0.3);
            plan.campfires.push(t.ground(fire));
            plan.tents.push((fire + rng.range(3.0, 5.0), -2.6));
            if final_camp {
                plan.tents.push((fire - rng.range(4.0, 6.0), -3.0));
            }
            // A guard with a crossbow watching the water he came from.
            plan.hunters.push(HunterSpawn {
                kind: HunterKind::Crossbow,
                pos: foot(a + 1.0),
                patrol: (a, a + 2.0),
                boat: None,
                facing: -1.0,
                guard: true,
            });
            plan.hunters.push(HunterSpawn {
                kind: HunterKind::Lantern,
                pos: foot(fire + 2.0),
                patrol: (a + 2.0, b),
                boat: None,
                facing: 1.0,
                guard: false,
            });
            if night >= 2 || final_camp {
                let x = rng.range(a, b);
                plan.hunters.push(HunterSpawn {
                    kind: hunter_kind(rng, night),
                    pos: foot(x),
                    patrol: (a, b),
                    boat: None,
                    facing: -1.0,
                    guard: false,
                });
            }
            if final_camp {
                plan.hunters.push(HunterSpawn {
                    kind: HunterKind::Warden,
                    pos: foot(fire - 2.0),
                    patrol: (fire - 6.0, fire + 4.0),
                    boat: None,
                    facing: -1.0,
                    guard: false,
                });
            }
            for _ in 0..rng.below(3) + 2 {
                let x = rng.range(a, b);
                if (x - fire).abs() < 1.5 {
                    continue;
                }
                plan.props.push(PropSpawn {
                    kind: if rng.chance(0.55) {
                        PropKind::Crate
                    } else {
                        PropKind::Barrel
                    },
                    pos: Vec2::new(x, t.height(x) + 0.6),
                    angle: 0.0,
                });
            }
            let post = if rng.chance(0.5) { a + 2.5 } else { b - 1.0 };
            plan.lantern_posts
                .push((t.ground(post), if post < fire { 1.0 } else { -1.0 }));
            if final_camp {
                plan.lantern_posts.push((t.ground(b - 4.0), -1.0));
            }
            if rng.chance(0.5) || final_camp {
                let x = if rng.chance(0.5) { a + 1.0 } else { b };
                plan.mushrooms.push((t.ground(x), 1.1));
            }
        }
        Kind::Pool => {
            let boat = plan.boats.len();
            let bx = rng.range(x0 + 10.0, x1 - 10.0);
            plan.boats.push(BoatSpawn {
                x: bx,
                patrol: (x0 + 8.0, x1 - 8.0),
            });
            let crew = if night >= 2 || rng.chance(0.5) { 2 } else { 1 };
            for c in 0..crew {
                plan.hunters.push(HunterSpawn {
                    kind: if c == 0 {
                        HunterKind::Crossbow
                    } else {
                        HunterKind::Lantern
                    },
                    pos: Vec2::new(bx + if c == 0 { 0.8 } else { -0.9 }, 1.3),
                    patrol: (bx, bx),
                    boat: Some(boat),
                    facing: if c == 0 { 1.0 } else { -1.0 },
                    guard: false,
                });
            }
            pool_life(plan, sec, rng, 6);
        }
        Kind::BridgePool => {
            let hl = t.height(x0 - 0.5);
            let hr = t.height(x1 + 0.5);
            let a = Vec2::new(x0 - 0.6, hl + 0.35);
            let b = Vec2::new(x1 + 0.6, hr + 0.35);
            plan.bridges.push(Bridge { a, b });
            let prev = &plan.sections[index - 1];
            let next = &plan.sections[index + 1];
            plan.hunters.push(HunterSpawn {
                kind: HunterKind::Lantern,
                pos: foot(x0 - 2.0),
                patrol: ((x0 - 5.0).max(prev.x0 + 2.0), (x1 + 5.0).min(next.x1 - 2.0)),
                boat: None,
                facing: 1.0,
                guard: false,
            });
            pool_life(plan, sec, rng, 3);
        }
        Kind::DockPool => {
            let (d0, d1) = (x0 + 6.5, x1 - 6.5);
            let y = 1.2;
            plan.docks.push(Dock { x0: d0, x1: d1, y });
            let stand = |x: f32| Vec2::new(x, y + 0.9);
            plan.hunters.push(HunterSpawn {
                kind: hunter_kind(rng, night),
                pos: stand(rng.range(d0 + 1.0, d1 - 1.0)),
                patrol: (d0 + 0.8, d1 - 0.8),
                boat: None,
                facing: 1.0,
                guard: false,
            });
            if night >= 2 || rng.chance(0.4) {
                plan.hunters.push(HunterSpawn {
                    kind: HunterKind::Crossbow,
                    pos: stand(d1 - 1.0),
                    patrol: (d1 - 1.5, d1 - 0.8),
                    boat: None,
                    facing: -1.0,
                    guard: true,
                });
            }
            plan.lantern_posts.push((Vec2::new((d0 + d1) * 0.5, y), 1.0));
            pool_life(plan, sec, rng, 4);
        }
        Kind::Marsh => {
            // Wading hunters, knee-deep, among the reeds.
            let x = rng.range(x0 + 6.0, x1 - 6.0);
            plan.hunters.push(HunterSpawn {
                kind: HunterKind::Lantern,
                pos: foot(x),
                patrol: (x0 + 4.0, x1 - 4.0),
                boat: None,
                facing: -1.0,
                guard: false,
            });
            if night >= 3 {
                plan.hunters.push(HunterSpawn {
                    kind: HunterKind::Crossbow,
                    pos: foot(x + 5.0),
                    patrol: (x0 + 4.0, x1 - 4.0),
                    boat: None,
                    facing: 1.0,
                    guard: false,
                });
            }
            let mut x = x0 + 2.0;
            while x < x1 - 2.0 {
                let run = rng.range(2.5, 6.0);
                if rng.chance(0.55) {
                    plan.reed_cover.push((x, x + run));
                    let n = (run * 2.2) as usize;
                    for k in 0..n {
                        plan.reeds.push(Decor {
                            x: x + (k as f32 + rng.f()) * run / n as f32,
                            z: rng.range(0.7, 1.5),
                            scale: rng.range(0.9, 1.35),
                            variant: rng.below(3),
                        });
                    }
                }
                x += run + rng.range(1.5, 4.0);
            }
            for _ in 0..(w * 0.9) as usize {
                plan.reeds.push(Decor {
                    x: rng.range(x0, x1),
                    z: rng.range(-0.8, -3.5),
                    scale: rng.range(0.9, 1.4),
                    variant: rng.below(3),
                });
            }
            for k in 0..3 {
                plan.fireflies.push(Vec2::new(x0 + w * (k as f32 + 0.5) / 3.0, 1.5));
            }
            for _ in 0..4 {
                plan.props.push(PropSpawn {
                    kind: PropKind::LilyPad,
                    pos: Vec2::new(rng.range(x0 + 3.0, x1 - 3.0), 0.05),
                    angle: 0.0,
                });
            }
        }
    }
}

/// Lily pads, a floating log or barrel, rocks and glowing fungus in a pool.
fn pool_life(plan: &mut Plan, sec: &Section, rng: &mut Rng, pads: usize) {
    let (x0, x1) = (sec.x0 + 4.0, sec.x1 - 4.0);
    for _ in 0..pads {
        plan.props.push(PropSpawn {
            kind: PropKind::LilyPad,
            pos: Vec2::new(rng.range(x0, x1), 0.05),
            angle: 0.0,
        });
    }
    if sec.width() > 20.0 && rng.chance(0.7) {
        plan.props.push(PropSpawn {
            kind: PropKind::Log,
            pos: Vec2::new(rng.range(x0 + 2.0, x1 - 2.0), 0.2),
            angle: rng.sym(0.1),
        });
    }
    if rng.chance(0.5) {
        plan.props.push(PropSpawn {
            kind: PropKind::Barrel,
            pos: Vec2::new(rng.range(x0, x1), 0.3),
            angle: 1.2,
        });
    }
    for _ in 0..(sec.width() / 7.0) as usize {
        plan.fungi.push((rng.range(x0, x1), rng.range(-0.4, -4.2)));
    }
    if rng.chance(0.7) {
        let x = rng.range(x0 + 2.0, x1 - 2.0);
        let r = rng.range(0.7, 1.2);
        plan.rocks.push((Vec2::new(x, plan.terrain.height(x) + r * 0.4), r));
    }
    weeds(plan, x0, x1, (sec.width() / 3.0) as usize, rng);
}

fn weeds(plan: &mut Plan, x0: f32, x1: f32, n: usize, rng: &mut Rng) {
    for _ in 0..n {
        let x = rng.range(x0, x1);
        if plan.terrain.depth(x) < 1.5 {
            continue;
        }
        plan.weeds.push(Decor {
            x,
            z: rng.range(-0.8, -4.0),
            scale: rng.range(0.6, 1.4),
            variant: rng.below(3),
        });
    }
}

/// Reeds wherever the ground meets the water, some in front of the playfield.
fn shore_reeds(plan: &mut Plan, rng: &mut Rng) {
    let t = &plan.terrain;
    let mut x = 0.0;
    let mut run: Option<f32> = None;
    while x < t.len {
        let h = t.height(x);
        let shore = (-0.9..0.7).contains(&h);
        if shore {
            if rng.chance(0.55) {
                plan.reeds.push(Decor {
                    x: x + rng.sym(0.2),
                    z: rng.range(-0.6, -2.6),
                    scale: rng.range(0.8, 1.3),
                    variant: rng.below(3),
                });
            }
            if rng.chance(0.35) {
                plan.reeds.push(Decor {
                    x: x + rng.sym(0.2),
                    z: rng.range(0.7, 1.4),
                    scale: rng.range(0.8, 1.2),
                    variant: rng.below(3),
                });
                run.get_or_insert(x);
            }
        } else if let Some(start) = run.take()
            && x - start > 1.5
        {
            plan.reed_cover.push((start, x));
        }
        x += 0.4;
    }
}
