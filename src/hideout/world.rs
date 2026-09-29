//! The hideout bunker: generation, build mode and interaction.

use std::collections::HashMap;

use glam::{IVec3, Vec3};
use winit::keyboard::KeyCode;

use super::{HideoutState, StationKind};
use crate::game::{gather_move_input, Settings};
use crate::input::Input;
use crate::player::{physics, Player, HALF_WIDTH};
use crate::world::{Block, RayHit, World};

/// 64 x 32 x 64 blocks.
pub const HIDEOUT_CHUNKS: IVec3 = IVec3::new(4, 2, 4);
const HALL_MIN: IVec3 = IVec3::new(12, 4, 12);
const HALL_MAX: IVec3 = IVec3::new(51, 12, 51);
const REACH: f32 = 6.0;
const INTERACT_REACH: f32 = 3.5;

pub fn station_pos(s: StationKind) -> IVec3 {
    match s {
        StationKind::Workbench => IVec3::new(18, 4, 12),
        StationKind::AmmoPress => IVec3::new(27, 4, 12),
        StationKind::Medstation => IVec3::new(36, 4, 12),
    }
}

pub fn stash_pos() -> IVec3 {
    IVec3::new(45, 4, 12)
}

/// Blocks that belong to the hideout itself and can't be removed.
fn protected(world: &World, p: IVec3) -> bool {
    let b = world.get(p);
    b == Block::Bedrock || b == Block::StashBox || StationKind::from_block(b).is_some() || p == stash_pos()
}

fn decor_for(s: StationKind, level: u32) -> Vec<(IVec3, Block)> {
    // Stations grow a little workshop around them as they are upgraded.
    let (a, b, c) = match s {
        StationKind::Workbench => (Block::Planks, Block::Metal, Block::Lamp),
        StationKind::AmmoPress => (Block::Barrel, Block::RustMetal, Block::Lamp),
        StationKind::Medstation => (Block::Plaster, Block::Glass, Block::Lamp),
    };
    let p = station_pos(s);
    let mut v = Vec::new();
    if level >= 1 {
        v.push((p + IVec3::new(-1, 0, 0), a));
    }
    if level >= 2 {
        v.push((p + IVec3::new(1, 0, 0), b));
        v.push((p + IVec3::new(1, 1, 0), b));
    }
    if level >= 3 {
        v.push((p + IVec3::new(0, 2, 0), c));
        v.push((p + IVec3::new(-1, 1, 0), a));
    }
    v
}

/// Place station decorations for the current level (runtime: marks chunks for remeshing).
pub fn apply_station_decor(world: &mut World, s: StationKind, level: u32) {
    for (p, b) in decor_for(s, level) {
        world.set(p, b);
    }
}

pub fn generate(state: &HideoutState) -> World {
    let mut w = World::new(HIDEOUT_CHUNKS);
    let max = w.size - IVec3::ONE;
    w.fill_gen(IVec3::ZERO, max, Block::Stone);
    // Indestructible shell.
    w.fill_gen(IVec3::ZERO, IVec3::new(max.x, 0, max.z), Block::Bedrock);
    w.fill_gen(IVec3::new(0, max.y - 1, 0), max, Block::Bedrock);
    for (lo, hi) in [
        (IVec3::new(0, 0, 0), IVec3::new(1, max.y, max.z)),
        (IVec3::new(max.x - 1, 0, 0), IVec3::new(max.x, max.y, max.z)),
        (IVec3::new(0, 0, 0), IVec3::new(max.x, max.y, 1)),
        (IVec3::new(0, 0, max.z - 1), IVec3::new(max.x, max.y, max.z)),
    ] {
        w.fill_gen(lo, hi, Block::Bedrock);
    }

    // Main hall: concrete shell, tiled floor.
    let (lo, hi) = (HALL_MIN - IVec3::ONE, HALL_MAX + IVec3::ONE);
    w.fill_gen(lo, hi, Block::Concrete);
    w.fill_gen(HALL_MIN, HALL_MAX, Block::Air);
    w.fill_gen(
        IVec3::new(HALL_MIN.x, HALL_MIN.y - 1, HALL_MIN.z),
        IVec3::new(HALL_MAX.x, HALL_MIN.y - 1, HALL_MAX.z),
        Block::FloorTile,
    );
    // Pillars.
    for (x, z) in [(22, 22), (41, 22), (22, 41), (41, 41)] {
        w.fill_gen(
            IVec3::new(x, HALL_MIN.y, z),
            IVec3::new(x, HALL_MAX.y, z),
            Block::Concrete,
        );
    }
    // Ceiling lamps.
    let mut x = HALL_MIN.x + 3;
    while x <= HALL_MAX.x - 3 {
        let mut z = HALL_MIN.z + 3;
        while z <= HALL_MAX.z - 3 {
            w.set_gen(IVec3::new(x, HALL_MAX.y + 1, z), Block::Lamp);
            z += 7;
        }
        x += 7;
    }
    // Brick wainscot along the walls.
    for x in HALL_MIN.x - 1..=HALL_MAX.x + 1 {
        for z in [HALL_MIN.z - 1, HALL_MAX.z + 1] {
            w.set_gen(IVec3::new(x, HALL_MIN.y, z), Block::Brick);
        }
    }
    for z in HALL_MIN.z - 1..=HALL_MAX.z + 1 {
        for x in [HALL_MIN.x - 1, HALL_MAX.x + 1] {
            w.set_gen(IVec3::new(x, HALL_MIN.y, z), Block::Brick);
        }
    }

    // Stations and stash along the north wall.
    for s in StationKind::ALL {
        w.set_gen(station_pos(s), s.block());
    }
    w.set_gen(stash_pos(), Block::StashBox);
    // Generator corner and some storage.
    w.set_gen(IVec3::new(50, 4, 50), Block::Generator);
    w.set_gen(IVec3::new(49, 4, 50), Block::Barrel);
    w.set_gen(IVec3::new(50, 4, 49), Block::Barrel);
    for (x, z) in [(13, 49), (14, 49), (13, 50), (14, 50), (13, 51)] {
        w.set_gen(IVec3::new(x, 4, z), Block::Crate);
    }
    w.set_gen(IVec3::new(13, 5, 50), Block::Crate);
    // A small shooting-range backstop of sandbags on the east side.
    for z in 28..=35 {
        w.set_gen(IVec3::new(50, 4, z), Block::Sandbag);
        w.set_gen(IVec3::new(50, 5, z), Block::Sandbag);
    }

    for s in StationKind::ALL {
        for (p, b) in decor_for(s, state.level(s)) {
            w.set_gen(p, b);
        }
    }
    // Player edits.
    for e in &state.edits {
        let p = IVec3::new(e[0], e[1], e[2]);
        if w.in_bounds(p) && !protected(&w, p) {
            w.set_gen(p, Block::from_u8(e[3] as u8));
        }
    }
    w.finish_generation();
    w
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HideoutInteract {
    Station(StationKind),
    Stash,
}

pub struct HideoutSession {
    pub world: World,
    pub player: Player,
    pub selected: usize,
    pub edits: HashMap<IVec3, Block>,
    pub open_station: Option<StationKind>,
    pub stash_open: bool,
    pub messages: Vec<(String, f32)>,
    build_cooldown: f32,
    pub station_status: Option<(String, bool)>,
}

impl HideoutSession {
    pub fn new(state: &HideoutState) -> Self {
        let world = generate(state);
        let spawn = Vec3::new(32.5, HALL_MIN.y as f32, 30.5);
        let mut player = Player::new(spawn, 0.0);
        player.pitch = -0.1;
        let edits = state
            .edits
            .iter()
            .map(|e| (IVec3::new(e[0], e[1], e[2]), Block::from_u8(e[3] as u8)))
            .collect();
        Self {
            world,
            player,
            selected: 0,
            edits,
            open_station: None,
            stash_open: false,
            messages: Vec::new(),
            build_cooldown: 0.0,
            station_status: None,
        }
    }

    pub fn message(&mut self, text: impl Into<String>) {
        self.messages.push((text.into(), 4.0));
        if self.messages.len() > 5 {
            self.messages.remove(0);
        }
    }

    pub fn edits_vec(&self) -> Vec<[i32; 4]> {
        let mut v: Vec<[i32; 4]> = self.edits.iter().map(|(p, b)| [p.x, p.y, p.z, *b as i32]).collect();
        v.sort();
        v
    }

    pub fn selected_block(&self) -> Block {
        let list = Block::buildable();
        list[self.selected % list.len()]
    }

    pub fn target(&self) -> Option<RayHit> {
        self.world
            .raycast(self.player.eye_pos(), self.player.look_dir(), REACH, |b| !b.is_air())
    }

    pub fn interaction(&self) -> Option<HideoutInteract> {
        let hit = self
            .world
            .raycast(self.player.eye_pos(), self.player.look_dir(), INTERACT_REACH, |b| {
                !b.is_air()
            })?;
        if hit.block == Block::StashBox {
            return Some(HideoutInteract::Stash);
        }
        StationKind::from_block(hit.block).map(HideoutInteract::Station)
    }

    fn record(&mut self, p: IVec3, b: Block) {
        self.world.set(p, b);
        self.edits.insert(p, b);
    }

    /// Remove the targeted block. Returns true if something changed.
    pub fn break_target(&mut self) -> bool {
        let Some(hit) = self.target() else { return false };
        if protected(&self.world, hit.pos) {
            self.message(format!("{} can't be removed", hit.block.name()));
            return false;
        }
        self.record(hit.pos, Block::Air);
        true
    }

    /// Place the selected block against the targeted face.
    pub fn place_at_target(&mut self) -> bool {
        let Some(hit) = self.target() else { return false };
        let p = hit.pos + hit.normal;
        if hit.normal == IVec3::ZERO || !self.world.in_bounds(p) || !self.world.get(p).is_air() {
            return false;
        }
        // Don't entomb the player.
        let (min, max) = physics::body_aabb(self.player.pos, HALF_WIDTH, self.player.height());
        let bmin = p.as_vec3();
        let bmax = bmin + Vec3::ONE;
        if min.x < bmax.x && max.x > bmin.x && min.y < bmax.y && max.y > bmin.y && min.z < bmax.z && max.z > bmin.z {
            return false;
        }
        let b = self.selected_block();
        self.record(p, b);
        true
    }

    pub fn update(&mut self, dt: f32, input: &Input, settings: &Settings, accept: bool) {
        for m in &mut self.messages {
            m.1 -= dt;
        }
        self.messages.retain(|m| m.1 > 0.0);
        self.build_cooldown -= dt;
        if accept {
            let look = input.mouse_delta * settings.sensitivity;
            self.player.look(-look.x, -look.y);
            let n = Block::buildable().len();
            let keys = [
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
                KeyCode::Minus,
                KeyCode::Equal,
            ];
            for (i, k) in keys.iter().enumerate() {
                if input.pressed(*k) && i < n {
                    self.selected = i;
                }
            }
            if input.scroll > 0.1 {
                self.selected = (self.selected + n - 1) % n;
            } else if input.scroll < -0.1 {
                self.selected = (self.selected + 1) % n;
            }
            if self.build_cooldown <= 0.0 {
                // Break takes priority over place; either action starts the repeat delay.
                let acted = (input.lmb() && self.break_target()) || (input.rmb() && self.place_at_target());
                if acted {
                    self.build_cooldown = 0.22;
                }
            }
            if !input.lmb() && !input.rmb() {
                self.build_cooldown = 0.0;
            }
        }
        let mv = if accept {
            gather_move_input(input)
        } else {
            Default::default()
        };
        self.player.update(&self.world, &mv, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hideout_generates_with_stations() {
        let mut state = HideoutState::default();
        state.set_level(StationKind::Workbench, 3);
        let w = generate(&state);
        for s in StationKind::ALL {
            assert_eq!(w.get(station_pos(s)), s.block());
        }
        assert_eq!(w.get(stash_pos()), Block::StashBox);
        // Level-3 workbench decor.
        assert_eq!(
            w.get(station_pos(StationKind::Workbench) + IVec3::new(0, 2, 0)),
            Block::Lamp
        );
        let s = HideoutSession::new(&state);
        let p = s.player.pos.floor().as_ivec3();
        assert!(w.get(p).is_air() && w.get(p - IVec3::Y).is_solid());
    }

    #[test]
    fn building_and_edits_persist() {
        let state = HideoutState::default();
        let mut s = HideoutSession::new(&state);
        // Look straight down at the floor and place/remove blocks in front.
        s.player.pitch = -1.2;
        s.player.yaw = 0.0;
        assert!(s.place_at_target());
        let placed: Vec<_> = s.edits.iter().filter(|(_, b)| !b.is_air()).map(|(p, _)| *p).collect();
        assert_eq!(placed.len(), 1);
        assert!(s.break_target());
        let saved = HideoutState {
            levels: vec![],
            edits: s.edits_vec(),
        };
        let w = generate(&saved);
        assert!(w.get(placed[0]).is_air());
        // Stations are protected.
        s.player.pos = Vec3::new(18.5, 4.0, 14.5);
        s.player.yaw = 0.0;
        s.player.pitch = -0.35;
        if let Some(h) = s.target() {
            if StationKind::from_block(h.block).is_some() {
                assert!(!s.break_target());
            }
        }
    }
}
