//! An in-progress raid: the map, the player, scavs, combat, loot and extraction.

use std::collections::HashMap;

use glam::{IVec3, Vec2, Vec3};
use winit::keyboard::KeyCode;

use crate::ai::{PlayerInfo, Scav, ScavShot, ScavState};
use crate::effects::Effects;
use crate::game::{gather_move_input, Settings};
use crate::input::Input;
use crate::inventory::{loot, EquipSlot, Equipment, Grid, GridRef, Item, ItemKind, MedKind};
use crate::player::{BodyPart, Player};
use crate::rng::Rng;
use crate::weapons::armor::resolve_armor;
use crate::weapons::ballistics::{self, humanoid_hitboxes, ray_hitbox, Hitbox, ShotOutcome, TargetBoxes, TargetId};
use crate::weapons::{AmmoType, Caliber, GunState, ReloadState, Weapon};
use crate::world::gen::{generate_raid_map, ExtractPoint, MapInfo};
use crate::world::{ContainerKind, World};

/// Raid duration before the player is declared missing in action.
pub const RAID_TIME: f32 = 25.0 * 60.0;
pub const EXTRACT_TIME: f32 = 6.0;
const INTERACT_RANGE: f32 = 2.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponSlot {
    Primary,
    Holster,
}

impl WeaponSlot {
    pub fn equip_slot(self) -> EquipSlot {
        match self {
            WeaponSlot::Primary => EquipSlot::Primary,
            WeaponSlot::Holster => EquipSlot::Holster,
        }
    }
}

pub struct Message {
    pub text: String,
    pub color: [u8; 3],
    pub ttl: f32,
}

pub struct DeathInfo {
    pub cause: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LootRef {
    Container(IVec3),
    Body(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Interact {
    Container(IVec3, ContainerKind),
    Body(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutcomeKind {
    Survived(String),
    Killed(String),
    MissingInAction,
}

#[derive(Clone, Debug)]
pub struct RaidOutcome {
    pub kind: OutcomeKind,
    pub time: f32,
    pub kills: u32,
    pub value_in: u64,
    pub value_out: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct HealState {
    pub remaining: f32,
    pub total: f32,
    pub grid: GridRef,
    pub uid: u64,
    pub kind: MedKind,
}

pub struct Raid {
    pub seed: u64,
    pub world: World,
    pub map: MapInfo,
    pub player: Player,
    pub time: f32,
    pub scavs: Vec<Scav>,
    pub effects: Effects,
    pub rng: Rng,
    pub equipment: Equipment,
    pub active: WeaponSlot,
    pub gun: GunState,
    /// Preferred ammo type per calibre for the next reload.
    pub ammo_pref: Vec<(Caliber, AmmoType)>,
    pub messages: Vec<Message>,
    pub hit_marker: f32,
    pub hit_marker_kill: bool,
    pub damage_flash: f32,
    /// World-space origin of the last hit taken (for the HUD indicator).
    pub last_hit_from: Option<Vec3>,
    pub hit_indicator: f32,
    pub kills: u32,
    pub dead: Option<DeathInfo>,
    last_damage_cause: String,
    footstep_timer: f32,
    /// Generated lazily the first time a container is opened.
    pub containers: HashMap<IVec3, Grid>,
    pub bodies: HashMap<usize, Grid>,
    pub open_loot: Option<LootRef>,
    /// Extraction points available this raid.
    pub extracts: Vec<ExtractPoint>,
    pub extract_progress: f32,
    pub extracting_at: Option<usize>,
    pub heal: Option<HealState>,
    pub value_in: u64,
    pub finished: Option<RaidOutcome>,
    pub show_extracts: f32,
}

fn yaw_towards(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    (-d.x).atan2(-d.z)
}

/// Box the (lying) corpse occupies, for interaction raycasts.
pub fn corpse_box(s: &Scav) -> Hitbox {
    Hitbox {
        part: BodyPart::Thorax,
        center: s.pos + s.forward() * 0.9 + Vec3::Y * 0.15,
        half: Vec3::new(0.4, 0.25, 0.95),
        yaw: s.yaw,
    }
}

impl Raid {
    pub fn new(seed: u64, equipment: Equipment) -> Self {
        let (world, map) = generate_raid_map(seed);
        let mut rng = Rng::new(seed ^ 0xABCD_EF01);
        let spawn = if map.player_spawns.is_empty() {
            Vec3::new(96.5, 40.0, 96.5)
        } else {
            *rng.pick(&map.player_spawns)
        };
        let center = Vec3::new(96.0, spawn.y, 96.0);
        let player = Player::new(spawn, yaw_towards(spawn, center));

        // Extracts: the ones far from the spawn are open this raid.
        let mut extracts = map.extracts.clone();
        extracts.sort_by(|a, b| {
            b.pos
                .distance(spawn)
                .partial_cmp(&a.pos.distance(spawn))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let far: Vec<ExtractPoint> = extracts.iter().filter(|e| e.pos.distance(spawn) > 70.0).cloned().collect();
        let extracts = if far.len() >= 2 {
            far.into_iter().take(3).collect()
        } else {
            extracts.into_iter().take(2).collect()
        };

        // Scavs: spread out, away from the player.
        let mut candidates: Vec<Vec3> = map
            .scav_spawns
            .iter()
            .chain(map.patrol_points.iter())
            .copied()
            .filter(|p| p.distance(spawn) > 50.0)
            .collect();
        let mut scavs: Vec<Scav> = Vec::new();
        let target = 12;
        let mut guard = 0;
        while scavs.len() < target && !candidates.is_empty() && guard < 500 {
            guard += 1;
            let i = rng.range_usize(0, candidates.len());
            let p = candidates[i];
            if scavs.iter().all(|s| s.pos.distance(p) > 9.0) {
                let id = scavs.len();
                scavs.push(Scav::spawn(id, p, &mut rng));
                candidates.swap_remove(i);
            }
        }
        let active = if equipment.primary.is_some() {
            WeaponSlot::Primary
        } else {
            WeaponSlot::Holster
        };
        let value_in = equipment.total_value();
        let mut raid = Self {
            seed,
            world,
            map,
            player,
            time: 0.0,
            scavs,
            effects: Effects::default(),
            rng,
            equipment,
            active,
            gun: GunState::default(),
            ammo_pref: Vec::new(),
            messages: Vec::new(),
            hit_marker: 0.0,
            hit_marker_kill: false,
            damage_flash: 0.0,
            last_hit_from: None,
            hit_indicator: 0.0,
            kills: 0,
            dead: None,
            last_damage_cause: "Unknown".into(),
            footstep_timer: 0.0,
            containers: HashMap::new(),
            bodies: HashMap::new(),
            open_loot: None,
            extracts,
            extract_progress: 0.0,
            extracting_at: None,
            heal: None,
            value_in,
            finished: None,
            show_extracts: 10.0,
        };
        let names: Vec<String> = raid.extracts.iter().map(|e| e.name.clone()).collect();
        raid.message(format!("Extracts: {}", names.join(", ")), [140, 220, 140]);
        raid.message("Find loot and reach an extraction point. [O] shows exits.", [220, 210, 170]);
        raid
    }

    pub fn message(&mut self, text: impl Into<String>, color: [u8; 3]) {
        self.messages.push(Message {
            text: text.into(),
            color,
            ttl: 5.0,
        });
        if self.messages.len() > 6 {
            self.messages.remove(0);
        }
    }

    pub fn weapon(&self) -> Option<&Weapon> {
        self.equipment.weapon(self.active.equip_slot())
    }

    fn weapon_mut(&mut self) -> Option<&mut Weapon> {
        self.equipment.weapon_mut(self.active.equip_slot())
    }

    pub fn time_left(&self) -> f32 {
        (RAID_TIME - self.time).max(0.0)
    }

    pub fn preferred_ammo(&self, cal: Caliber) -> Option<AmmoType> {
        self.ammo_pref.iter().find(|(c, _)| *c == cal).map(|(_, a)| *a)
    }

    /// Ammo type that the next reload would use.
    pub fn next_reload_ammo(&self) -> Option<AmmoType> {
        let w = self.weapon()?;
        let cal = w.caliber();
        if let Some(p) = self.preferred_ammo(cal) {
            if self.equipment.ammo_count(p) > 0 {
                return Some(p);
            }
        }
        if let Some(l) = w.loaded {
            if self.equipment.ammo_count(l) > 0 {
                return Some(l);
            }
        }
        cal.ammo_types().iter().copied().find(|a| self.equipment.ammo_count(*a) > 0)
    }

    pub fn reserve_for_active(&self) -> u32 {
        self.weapon()
            .map(|w| w.caliber().ammo_types().iter().map(|a| self.equipment.ammo_count(*a)).sum())
            .unwrap_or(0)
    }

    /// Put loose rounds back into the player's pouches.
    fn return_ammo(&mut self, ammo: AmmoType, mut n: u32) {
        while n > 0 {
            let chunk = n.min(ItemKind::Ammo(ammo).max_stack());
            n -= chunk;
            if self.equipment.stow(Item::stack(ItemKind::Ammo(ammo), chunk)).is_err() {
                self.message(format!("No space for {} rounds - dropped", chunk + n), [230, 120, 90]);
                return;
            }
        }
    }

    pub fn is_scoped(&self) -> bool {
        self.gun.ads > 0.9 && self.weapon().map(|w| w.stats().zoom >= 3.0).unwrap_or(false)
    }

    /// Current zoom factor including ADS blend (irons/red dots zoom slightly).
    pub fn zoom(&self) -> f32 {
        let z = self.weapon().map(|w| w.stats().zoom).unwrap_or(1.0).max(1.15);
        1.0 + (z - 1.0) * self.gun.ads
    }

    fn player_info(&self) -> PlayerInfo {
        let p = &self.player;
        PlayerInfo {
            feet: p.pos,
            eye: p.eye_pos(),
            chest: p.pos + Vec3::Y * if p.crouching { 0.95 } else { 1.3 },
            crouching: p.crouching,
            alive: self.dead.is_none() && self.finished.is_none(),
            speed: p.horizontal_speed(),
        }
    }

    pub fn player_hitboxes(&self) -> TargetBoxes {
        TargetBoxes {
            target: TargetId::Player,
            boxes: humanoid_hitboxes(self.player.pos, self.player.yaw, if self.player.crouching { 1.0 } else { 0.0 }),
        }
    }

    fn scav_targets(&self) -> Vec<TargetBoxes> {
        self.scavs
            .iter()
            .enumerate()
            .filter(|(_, s)| s.alive())
            .map(|(i, s)| TargetBoxes {
                target: TargetId::Scav(i),
                boxes: s.hitboxes(),
            })
            .collect()
    }

    /// World position of the player's muzzle (approximate, for tracers).
    pub fn muzzle_world(&self) -> Vec3 {
        let p = &self.player;
        let dir = p.look_dir();
        let right = p.right_flat();
        let hip = p.eye_pos() + dir * 0.7 + right * 0.18 - Vec3::Y * 0.16;
        let ads = p.eye_pos() + dir * 0.7 - Vec3::Y * 0.05;
        hip.lerp(ads, self.gun.ads)
    }

    // ------------------------------------------------------------------
    // Interaction & loot
    // ------------------------------------------------------------------

    /// What the player is looking at and could interact with.
    pub fn interaction(&self) -> Option<Interact> {
        if self.dead.is_some() {
            return None;
        }
        let eye = self.player.eye_pos();
        let dir = self.player.look_dir();
        let mut best: Option<(f32, Interact)> = None;
        if let Some(h) = self.world.raycast(eye, dir, INTERACT_RANGE, |b| b.is_solid()) {
            if let Some(k) = h.block.container() {
                best = Some((h.t, Interact::Container(h.pos, k)));
            }
        }
        for (i, s) in self.scavs.iter().enumerate() {
            if s.alive() || s.pos.distance(eye) > 4.0 {
                continue;
            }
            if let Some(t) = ray_hitbox(eye, dir, &corpse_box(s)) {
                if t < INTERACT_RANGE && best.is_none_or(|(bt, _)| t < bt) {
                    best = Some((t, Interact::Body(i)));
                }
            }
        }
        best.map(|b| b.1)
    }

    pub fn interaction_label(&self, i: Interact) -> String {
        match i {
            Interact::Container(p, k) => {
                if self.containers.contains_key(&p) {
                    format!("[F] Open {}", k.name())
                } else {
                    format!("[F] Search {}", k.name())
                }
            }
            Interact::Body(i) => format!("[F] Search {}", self.scavs[i].name),
        }
    }

    /// Open a container / body for looting (generating its contents if needed).
    pub fn open(&mut self, i: Interact) {
        match i {
            Interact::Container(p, kind) => {
                if !self.containers.contains_key(&p) {
                    let g = loot::container_loot(kind, &mut self.rng);
                    self.containers.insert(p, g);
                }
                self.open_loot = Some(LootRef::Container(p));
            }
            Interact::Body(i) => {
                if !self.bodies.contains_key(&i) {
                    let g = loot::scav_body_loot(&self.scavs[i], &mut self.rng);
                    self.bodies.insert(i, g);
                }
                self.scavs[i].searched = true;
                self.open_loot = Some(LootRef::Body(i));
            }
        }
    }

    pub fn loot_name(&self) -> Option<String> {
        match self.open_loot? {
            LootRef::Container(p) => self.world.get(p).container().map(|k| k.name().to_string()),
            LootRef::Body(i) => Some(self.scavs[i].name.clone()),
        }
    }

    /// Split borrow: equipment and the open loot grid at the same time.
    pub fn equipment_and_loot(&mut self) -> (&mut Equipment, Option<&mut Grid>) {
        let loot = match self.open_loot {
            Some(LootRef::Container(p)) => self.containers.get_mut(&p),
            Some(LootRef::Body(i)) => self.bodies.get_mut(&i),
            None => None,
        };
        (&mut self.equipment, loot)
    }

    // ------------------------------------------------------------------
    // Healing
    // ------------------------------------------------------------------

    pub fn start_heal_with(&mut self, grid: GridRef, uid: u64) {
        if self.heal.is_some() || self.dead.is_some() {
            return;
        }
        let Some(item) = self.equipment.grid(grid).and_then(|g| g.get(uid)) else { return };
        let ItemKind::Med(kind) = item.item.kind else { return };
        if kind.is_surgery() && !self.player.body.has_destroyed_limb() {
            self.message("No destroyed limbs to operate on", [200, 200, 200]);
            return;
        }
        if !kind.is_surgery() && !self.player.body.is_injured() {
            self.message("You are not injured", [200, 200, 200]);
            return;
        }
        self.heal = Some(HealState {
            remaining: kind.use_time(),
            total: kind.use_time(),
            grid,
            uid,
            kind,
        });
        self.gun.reload = None;
    }

    fn quick_heal(&mut self) {
        let surgery = self.player.body.has_destroyed_limb() && self.equipment.find_med(true).is_some();
        let found = if surgery {
            self.equipment.find_med(true)
        } else {
            self.equipment.find_med(false)
        };
        match found {
            Some((g, uid, _)) => self.start_heal_with(g, uid),
            None => self.message("No medical supplies in your rig, pockets or backpack", [230, 120, 90]),
        }
    }

    fn finish_heal(&mut self, h: HealState) {
        let body = &mut self.player.body;
        let Some(grid) = self.equipment.grid_mut(h.grid) else { return };
        let Some(p) = grid.get_mut(h.uid) else { return };
        let uses = p.item.uses.unwrap_or(0);
        let msg;
        if h.kind.is_surgery() {
            if let Some(part) = body.repair_destroyed() {
                p.item.uses = Some(uses.saturating_sub(1));
                msg = format!("Operated on {}", part.name());
            } else {
                msg = "Nothing to operate on".into();
            }
        } else {
            let healed = body.heal(uses as f32);
            p.item.uses = Some(uses.saturating_sub(healed.ceil() as u32));
            msg = format!("Healed {:.0} HP", healed);
        }
        if p.item.uses == Some(0) {
            grid.remove(h.uid);
        }
        self.message(msg, [140, 220, 140]);
    }

    // ------------------------------------------------------------------
    // Update
    // ------------------------------------------------------------------

    fn finish(&mut self, kind: OutcomeKind) {
        if self.finished.is_some() {
            return;
        }
        let value_out = if matches!(kind, OutcomeKind::Survived(_)) {
            self.equipment.total_value()
        } else {
            0
        };
        self.finished = Some(RaidOutcome {
            kind,
            time: self.time,
            kills: self.kills,
            value_in: self.value_in,
            value_out,
        });
    }

    /// Player gave up / left the raid.
    pub fn abandon(&mut self) {
        self.finish(OutcomeKind::MissingInAction);
    }

    /// Called from the death screen.
    pub fn accept_death(&mut self) {
        let cause = self.dead.as_ref().map(|d| d.cause.clone()).unwrap_or_default();
        self.finish(OutcomeKind::Killed(cause));
    }

    pub fn update(&mut self, dt: f32, input: &Input, settings: &Settings, accept_input: bool) {
        if self.finished.is_some() {
            return;
        }
        self.time += dt;
        for m in &mut self.messages {
            m.ttl -= dt;
        }
        self.messages.retain(|m| m.ttl > 0.0);
        self.hit_marker = (self.hit_marker - dt).max(0.0);
        self.damage_flash = (self.damage_flash - dt * 1.5).max(0.0);
        self.hit_indicator = (self.hit_indicator - dt).max(0.0);
        self.show_extracts = (self.show_extracts - dt).max(0.0);

        let alive = self.dead.is_none();
        let accept = accept_input && alive;

        // --- Look & recoil compensation ---
        if accept {
            let sens = settings.sensitivity / self.zoom();
            let look = input.mouse_delta * sens;
            let dpitch = -look.y;
            if dpitch < 0.0 && self.gun.recoil_accum.y > 0.0 {
                self.gun.recoil_accum.y = (self.gun.recoil_accum.y + dpitch).max(0.0);
            }
            self.player.look(-look.x, dpitch);
            if input.pressed(KeyCode::KeyN) && cfg!(debug_assertions) {
                self.player.noclip = !self.player.noclip;
            }
            if input.pressed(KeyCode::KeyO) {
                self.show_extracts = 8.0;
            }
            if input.pressed(KeyCode::KeyH) {
                self.quick_heal();
            }
        }

        // --- Movement ---
        let mut mv = if accept { gather_move_input(input) } else { Default::default() };
        let armor_weight: f32 = [EquipSlot::Helmet, EquipSlot::Armor]
            .iter()
            .filter_map(|s| self.equipment.armor_state(*s))
            .map(|a| a.kind.def().weight)
            .sum();
        let mut speed_mult = (1.0 - 0.4 * self.gun.ads) * (1.0 - armor_weight);
        if self.gun.is_reloading() {
            speed_mult *= 0.85;
        }
        if self.heal.is_some() {
            speed_mult *= 0.55;
            mv.sprint = false;
        }
        mv.speed_mult = speed_mult;
        if self.gun.ads > 0.3 || input.lmb() && accept {
            mv.sprint = false;
        }
        if alive {
            self.player.update(&self.world, &mv, dt);
            if self.player.last_fall_damage > 0.0 {
                self.damage_flash = 1.0;
                self.last_damage_cause = "Fall damage".into();
            }
        }

        // Sprinting footsteps can be heard nearby.
        self.footstep_timer -= dt;
        if alive && self.player.sprinting && self.footstep_timer <= 0.0 {
            self.footstep_timer = 0.5;
            let pos = self.player.pos;
            for s in &mut self.scavs {
                s.hear(pos, 11.0, &mut self.rng);
            }
        }

        // --- Healing ---
        if let Some(mut h) = self.heal {
            h.remaining -= dt;
            if h.remaining <= 0.0 {
                self.heal = None;
                self.finish_heal(h);
            } else {
                self.heal = Some(h);
            }
        }

        // --- Weapon handling ---
        if alive {
            self.update_weapon(dt, input, accept);
        }

        // --- AI ---
        let info = self.player_info();
        let mut shots: Vec<ScavShot> = Vec::new();
        for s in &mut self.scavs {
            s.update(dt, &self.world, &info, &self.map.patrol_points, &mut self.rng, &mut shots);
        }
        for shot in shots {
            self.resolve_scav_shot(shot);
        }

        self.effects.update(dt, &self.world);

        if self.dead.is_none() && self.player.body.is_dead() {
            self.dead = Some(DeathInfo {
                cause: self.last_damage_cause.clone(),
            });
            self.gun.ads = 0.0;
            self.heal = None;
            self.open_loot = None;
        }

        // --- Extraction & timer ---
        if self.dead.is_none() {
            let p = self.player.pos;
            let inside = self.extracts.iter().position(|e| {
                Vec2::new(p.x - e.pos.x, p.z - e.pos.z).length() < e.radius && (p.y - e.pos.y).abs() < 3.0
            });
            if inside != self.extracting_at {
                self.extract_progress = 0.0;
                if let Some(i) = inside {
                    let name = self.extracts[i].name.clone();
                    self.message(format!("Extracting at {name} - stay in the zone"), [140, 220, 140]);
                }
            }
            self.extracting_at = inside;
            if let Some(i) = inside {
                self.extract_progress += dt;
                if self.extract_progress >= EXTRACT_TIME {
                    let name = self.extracts[i].name.clone();
                    self.finish(OutcomeKind::Survived(name));
                }
            }
            if self.time >= RAID_TIME {
                self.finish(OutcomeKind::MissingInAction);
            }
        }
    }

    fn update_weapon(&mut self, dt: f32, input: &Input, accept: bool) {
        let g = &mut self.gun;
        g.cooldown -= dt;
        g.since_shot += dt;
        g.kick *= (-14.0 * dt).exp();
        g.flash = (g.flash - dt).max(0.0);
        g.draw = (g.draw - dt).max(0.0);

        // Recoil recovery once the trigger is released.
        if g.since_shot > 0.12 {
            let r = g.recoil_accum * (dt * 6.0).min(1.0);
            self.player.pitch -= r.y;
            self.player.yaw += r.x;
            g.recoil_accum -= r;
        }
        if g.since_shot > 0.35 {
            g.shot_index = 0;
        }

        if accept {
            let want = if input.pressed(KeyCode::Digit1) {
                Some(WeaponSlot::Primary)
            } else if input.pressed(KeyCode::Digit2) {
                Some(WeaponSlot::Holster)
            } else if input.scroll.abs() > 0.1 {
                Some(match self.active {
                    WeaponSlot::Primary => WeaponSlot::Holster,
                    WeaponSlot::Holster => WeaponSlot::Primary,
                })
            } else {
                None
            };
            if let Some(slot) = want {
                self.switch_weapon(slot);
            }
        }
        // The active weapon may have been moved away in the inventory.
        if self.weapon().is_none() {
            let other = match self.active {
                WeaponSlot::Primary => WeaponSlot::Holster,
                WeaponSlot::Holster => WeaponSlot::Primary,
            };
            if self.equipment.weapon(other.equip_slot()).is_some() {
                self.switch_weapon(other);
            }
        }

        let Some(stats) = self.weapon().map(|w| w.stats()) else {
            self.gun.ads = 0.0;
            self.gun.reload = None;
            return;
        };

        // Reload progress.
        if let Some(mut r) = self.gun.reload {
            r.remaining -= dt;
            if r.remaining <= 0.0 {
                self.finish_reload(r);
                self.gun.reload = None;
            } else {
                self.gun.reload = Some(r);
            }
        }

        // ADS.
        let want_ads = accept && input.rmb() && !self.player.sprinting && self.gun.draw <= 0.0 && self.heal.is_none();
        let rate = 1.0 / stats.ads_time.max(0.05);
        if want_ads {
            self.gun.ads = (self.gun.ads + rate * dt).min(1.0);
        } else {
            self.gun.ads = (self.gun.ads - rate * 1.4 * dt).max(0.0);
        }

        if !accept {
            self.gun.trigger_prev = false;
            return;
        }

        if input.pressed(KeyCode::KeyB) && stats.full_auto {
            if let Some(w) = self.weapon_mut() {
                w.auto_mode = !w.auto_mode;
                let mode = if w.auto_mode { "Full auto" } else { "Semi-auto" };
                self.message(format!("Fire mode: {mode}"), [200, 200, 200]);
            }
        }
        if input.pressed(KeyCode::KeyT) {
            self.cycle_ammo_pref();
        }
        if input.pressed(KeyCode::KeyR) && self.heal.is_none() {
            self.start_reload();
        }

        // Firing.
        let trigger = input.lmb();
        let auto = self.weapon().map(|w| w.auto_mode && stats.full_auto).unwrap_or(false);
        let pulled = trigger && (auto || !self.gun.trigger_prev);
        self.gun.trigger_prev = trigger;
        if pulled
            && self.gun.cooldown <= 0.0
            && !self.gun.is_reloading()
            && self.gun.draw <= 0.0
            && !self.player.sprinting
            && self.heal.is_none()
        {
            let rounds = self.weapon().map(|w| w.rounds).unwrap_or(0);
            if !stats.operable {
                if input.lmb_pressed() {
                    self.message("Weapon is inoperable (missing barrel)", [230, 120, 90]);
                }
            } else if rounds == 0 {
                if input.lmb_pressed() {
                    self.message("*click* Magazine empty - press R to reload", [230, 180, 90]);
                }
                self.gun.cooldown = 0.25;
            } else {
                self.fire_player_shot(stats);
            }
        }
    }

    fn switch_weapon(&mut self, slot: WeaponSlot) {
        if slot == self.active && self.weapon().is_some() {
            return;
        }
        if self.equipment.weapon(slot.equip_slot()).is_none() {
            return;
        }
        self.active = slot;
        self.gun.reload = None;
        self.gun.ads = 0.0;
        self.gun.draw = 0.45;
        self.gun.shot_index = 0;
    }

    fn cycle_ammo_pref(&mut self) {
        let Some(w) = self.weapon() else { return };
        let cal = w.caliber();
        let types: Vec<AmmoType> = cal
            .ammo_types()
            .iter()
            .copied()
            .filter(|a| self.equipment.ammo_count(*a) > 0)
            .collect();
        if types.is_empty() {
            self.message(format!("No spare {} ammo", cal.name()), [230, 120, 90]);
            return;
        }
        let current = self.next_reload_ammo();
        let idx = current
            .and_then(|c| types.iter().position(|t| *t == c))
            .map(|i| (i + 1) % types.len())
            .unwrap_or(0);
        let next = types[idx];
        self.ammo_pref.retain(|(c, _)| *c != cal);
        self.ammo_pref.push((cal, next));
        self.message(format!("Next reload: {}", next.def().name), [200, 200, 160]);
    }

    fn start_reload(&mut self) {
        if self.gun.is_reloading() || self.gun.draw > 0.0 {
            return;
        }
        let Some(ammo) = self.next_reload_ammo() else {
            self.message("No ammo for this weapon in your rig, pockets or backpack", [230, 120, 90]);
            return;
        };
        let Some(w) = self.weapon() else { return };
        let stats = w.stats();
        if w.loaded == Some(ammo) && w.rounds >= stats.mag_size {
            return;
        }
        self.gun.reload = Some(ReloadState {
            remaining: stats.reload_time,
            total: stats.reload_time,
            ammo,
        });
        self.gun.ads = self.gun.ads.min(0.3);
    }

    fn finish_reload(&mut self, r: ReloadState) {
        let slot = self.active.equip_slot();
        let Some(w) = self.equipment.weapon_mut(slot) else { return };
        let cap = w.capacity();
        // Unload rounds of a different type back into the pouches.
        let mut returned = None;
        if w.loaded != Some(r.ammo) && w.rounds > 0 {
            returned = w.loaded.map(|a| (a, w.rounds));
            w.rounds = 0;
        }
        let rounds_before = w.rounds;
        let need = cap.saturating_sub(rounds_before);
        let got = self.equipment.take_kind(ItemKind::Ammo(r.ammo), need);
        if let Some(w) = self.equipment.weapon_mut(slot) {
            w.rounds = rounds_before + got;
            w.loaded = Some(r.ammo);
        }
        if let Some((a, n)) = returned {
            self.return_ammo(a, n);
        }
    }

    fn fire_player_shot(&mut self, stats: crate::weapons::WeaponStats) {
        let (ammo, receiver) = {
            let w = self.weapon_mut().expect("weapon");
            w.rounds -= 1;
            (w.loaded.unwrap_or(AmmoType::Ps545), w.receiver)
        };
        self.gun.cooldown = 60.0 / stats.fire_rate;
        self.gun.since_shot = 0.0;
        self.gun.kick = 1.0;
        self.gun.flash = 0.05;

        // Spread.
        let ads = self.gun.ads;
        let mut spread = stats.hip_spread + (stats.spread - stats.hip_spread) * ads;
        let speed = self.player.horizontal_speed();
        spread *= 1.0 + speed / 5.0;
        if !self.player.on_ground {
            spread *= 2.0;
        }
        spread += (self.gun.shot_index as f32 * 0.03).min(0.5);
        spread *= self.player.body.sway_mult();

        let eye = self.player.eye_pos();
        let dir = ballistics::apply_spread(self.player.look_dir(), spread, &mut self.rng);
        let targets = self.scav_targets();
        let out = ballistics::fire(&mut self.world, eye, dir, ammo, 320.0, &targets, Some(TargetId::Player));
        let muzzle = self.muzzle_world();
        self.effects.tracer(muzzle, out.end, ammo.def().tracer);
        self.apply_shot_effects(&out, dir);
        if let Some(hit) = out.entity {
            if let TargetId::Scav(i) = hit.target {
                self.damage_scav(i, hit.part, hit.damage, hit.pen, ammo, eye);
            }
        }

        // Recoil kick.
        let pat = receiver.recoil_pattern(self.gun.shot_index);
        let mut v = pat.y * stats.vertical_recoil;
        let mut h = pat.x * stats.horizontal_recoil * self.rng.range_f32(0.75, 1.25);
        let mult = (1.0 - 0.2 * ads) * if self.player.crouching { 0.85 } else { 1.0 };
        v *= mult;
        h *= mult;
        let kick = Vec2::new(h.to_radians(), v.to_radians());
        self.player.pitch = (self.player.pitch + kick.y).clamp(-1.54, 1.54);
        self.player.yaw -= kick.x;
        self.gun.recoil_accum += kick;
        self.gun.shot_index += 1;

        // Scavs hear gunshots.
        let pos = self.player.pos;
        for s in &mut self.scavs {
            s.hear(pos, stats.loudness, &mut self.rng);
        }
    }

    fn apply_shot_effects(&mut self, out: &ShotOutcome, dir: Vec3) {
        for b in &out.blocks {
            let tile = b.block.info().tiles[1];
            let color = crate::render::atlas::tile_color(tile);
            let n = b.normal.as_vec3();
            if b.destroyed {
                self.effects.block_break(b.pos.as_vec3() + Vec3::splat(0.5), color, &mut self.rng);
            } else if b.penetrated {
                // Entry and exit puffs so shots through walls read clearly.
                self.effects.debris(b.point + n * 0.02, n, color, 4, &mut self.rng);
                let exit = b.point + dir * 1.05;
                self.effects.debris(exit, dir, color, 4, &mut self.rng);
            } else {
                self.effects.debris(b.point + n * 0.02, n, color, 5, &mut self.rng);
                if matches!(
                    b.block,
                    crate::world::Block::Metal | crate::world::Block::RustMetal | crate::world::Block::Concrete
                ) {
                    self.effects.sparks(b.point + n * 0.02, n, &mut self.rng);
                }
            }
        }
        if let Some(e) = &out.entity {
            self.effects.blood(e.point, -dir, &mut self.rng);
        }
    }

    fn damage_scav(&mut self, i: usize, part: BodyPart, damage: f32, pen: f32, ammo: AmmoType, from: Vec3) {
        let time = self.time;
        let s = &mut self.scavs[i];
        if !s.alive() {
            return;
        }
        let armor = if part == BodyPart::Head { s.helmet.as_mut() } else { s.armor.as_mut() };
        let res = resolve_armor(armor, part, damage * part.damage_mult(), pen, &mut self.rng);
        let out = s.body.damage(part, res.damage);
        s.on_hit(from, &mut self.rng);
        self.hit_marker = 0.18;
        self.hit_marker_kill = out.killed;
        if out.killed {
            s.kill(time);
            let name = s.name.clone();
            self.kills += 1;
            self.message(format!("Killed {} ({}, {})", name, part.name(), ammo.def().short), [230, 110, 90]);
        } else if res.blocked {
            let eye = s.eye();
            self.effects.sparks(from.lerp(eye, 0.98), Vec3::Y, &mut self.rng);
        }
    }

    fn resolve_scav_shot(&mut self, shot: ScavShot) {
        let targets = if self.dead.is_none() && !self.player.noclip {
            vec![self.player_hitboxes()]
        } else {
            vec![]
        };
        let out = ballistics::fire(&mut self.world, shot.origin, shot.dir, shot.ammo, 250.0, &targets, None);
        self.effects.tracer(shot.origin + shot.dir * 0.4, out.end, shot.ammo.def().tracer);
        self.effects.muzzle_flash(shot.origin + shot.dir * 0.35);
        let dir = shot.dir;
        if out.end.distance(self.player.pos) < 120.0 {
            let o = ShotOutcome {
                end: out.end,
                blocks: out.blocks.clone(),
                entity: None,
            };
            self.apply_shot_effects(&o, dir);
        }
        if let Some(hit) = out.entity {
            let slot = if hit.part == BodyPart::Head {
                EquipSlot::Helmet
            } else {
                EquipSlot::Armor
            };
            let mut state = self.equipment.armor_state(slot);
            let res = resolve_armor(state.as_mut(), hit.part, hit.damage * hit.part.damage_mult(), hit.pen, &mut self.rng);
            if let Some(s) = state {
                self.equipment.set_durability(slot, s.durability);
            }
            let outcome = self.player.body.damage(hit.part, res.damage);
            self.damage_flash = (self.damage_flash + 0.6).min(1.0);
            self.last_hit_from = Some(shot.origin);
            self.hit_indicator = 1.2;
            self.heal = None;
            let shooter = self.scavs.get(shot.scav).map(|s| s.name.clone()).unwrap_or_else(|| "Scav".into());
            self.last_damage_cause = format!("{} - {} ({})", shooter, hit.part.name(), shot.ammo.def().name);
            if res.blocked {
                self.message(format!("Armor stopped a round ({})", hit.part.name()), [170, 170, 200]);
            } else if outcome.part_destroyed {
                self.message(format!("{} destroyed!", hit.part.name()), [230, 80, 70]);
            }
        }
        let origin = shot.origin;
        let loud = shot.loudness * 0.4;
        for s in &mut self.scavs {
            if s.id != shot.scav && s.state == ScavState::Patrol {
                s.hear(origin, loud, &mut self.rng);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Block;
    use glam::IVec3;

    /// A raid with a flat, empty arena carved out around the player.
    fn arena() -> Raid {
        let (_, eq) = crate::inventory::starter_profile_items();
        let mut raid = Raid::new(7, eq);
        let w = &mut raid.world;
        for x in 40..120 {
            for z in 40..120 {
                for y in 1..64 {
                    let b = if y <= 20 { Block::Stone } else { Block::Air };
                    w.set(IVec3::new(x, y, z), b);
                }
            }
        }
        let _ = w.take_dirty();
        raid.player.pos = Vec3::new(60.5, 21.0, 80.5);
        raid.player.yaw = 0.0; // facing -Z
        raid.player.pitch = 0.0;
        for s in &mut raid.scavs {
            s.pos = Vec3::new(10.0, 60.0, 10.0);
            s.kill(0.0);
        }
        raid
    }

    fn place_scav(raid: &mut Raid, pos: Vec3) -> usize {
        let mut rng = Rng::new(1);
        let mut s = Scav::spawn(raid.scavs.len(), pos, &mut rng);
        s.helmet = None;
        s.armor = None;
        raid.scavs.push(s);
        raid.scavs.len() - 1
    }

    #[test]
    fn player_can_kill_scav() {
        let mut raid = arena();
        let i = place_scav(&mut raid, Vec3::new(60.5, 21.0, 70.5));
        let stats = raid.weapon().unwrap().stats();
        for _ in 0..30 {
            if !raid.scavs[i].alive() {
                break;
            }
            // Aim at the thorax each shot (no spread worries at 10 m).
            let target = raid.scavs[i].pos + Vec3::Y * 1.3;
            let d = target - raid.player.eye_pos();
            raid.player.yaw = (-d.x).atan2(-d.z);
            raid.player.pitch = (d.y / Vec3::new(d.x, 0.0, d.z).length()).atan();
            raid.gun.ads = 1.0;
            raid.gun.shot_index = 0;
            raid.fire_player_shot(stats);
        }
        assert!(!raid.scavs[i].alive(), "scav should be dead");
        assert_eq!(raid.kills, 1);
    }

    #[test]
    fn scav_detects_and_damages_player() {
        let mut raid = arena();
        let i = place_scav(&mut raid, Vec3::new(60.5, 21.0, 62.5));
        raid.scavs[i].yaw = std::f32::consts::PI; // facing +Z, towards the player
        let input = Input::default();
        let settings = Settings::default();
        let start = raid.player.body.total();
        for _ in 0..(60 * 12) {
            raid.update(1.0 / 60.0, &input, &settings, false);
            if raid.player.body.total() < start {
                break;
            }
        }
        assert_eq!(raid.scavs[i].state, ScavState::Engage);
        assert!(raid.player.body.total() < start, "player should have been hit");
    }

    #[test]
    fn ap_ammo_breaches_brick_and_blocks_break() {
        let mut raid = arena();
        // Brick wall between player and target.
        for x in 55..66 {
            for y in 21..25 {
                raid.world.set(IVec3::new(x, y, 75), Block::Brick);
            }
        }
        let wall = IVec3::new(60, 22, 75);
        if let Some(w) = raid.equipment.weapon_mut(EquipSlot::Primary) {
            w.loaded = Some(AmmoType::Bs545);
            w.rounds = 30;
        }
        let stats = raid.weapon().unwrap().stats();
        raid.player.pitch = -0.0;
        let mut destroyed = false;
        for _ in 0..30 {
            raid.gun.ads = 1.0;
            raid.player.yaw = 0.0;
            raid.player.pitch = ((22.5 - raid.player.eye_pos().y) / 5.0f32).atan();
            raid.fire_player_shot(stats);
            if raid.world.get(wall) == Block::Air {
                destroyed = true;
                break;
            }
        }
        assert!(destroyed, "sustained AP fire should destroy a brick block");
        assert!(!raid.world.take_dirty().is_empty(), "destroyed block must trigger a remesh");
    }

    #[test]
    fn reaching_extract_survives_with_loot() {
        let mut raid = arena();
        let e = raid.extracts[0].clone();
        // Build a floor under the extract and stand there.
        let base = e.pos.floor().as_ivec3();
        for x in -3..=3 {
            for z in -3..=3 {
                raid.world.set(base + IVec3::new(x, -1, z), Block::Stone);
                for y in 0..3 {
                    raid.world.set(base + IVec3::new(x, y, z), Block::Air);
                }
            }
        }
        raid.player.pos = e.pos;
        let input = Input::default();
        let settings = Settings::default();
        for _ in 0..(60 * 7) {
            raid.update(1.0 / 60.0, &input, &settings, false);
        }
        let out = raid.finished.as_ref().expect("raid should be over");
        assert!(matches!(out.kind, OutcomeKind::Survived(_)), "{:?}", out.kind);
        assert!(out.value_out > 0);
    }

    #[test]
    fn looting_a_body_and_healing() {
        let mut raid = arena();
        let i = place_scav(&mut raid, Vec3::new(60.5, 21.0, 78.5));
        raid.scavs[i].kill(0.0);
        raid.open(Interact::Body(i));
        let (eq, loot) = raid.equipment_and_loot();
        let loot = loot.expect("body loot");
        let gun = loot
            .items
            .iter()
            .find(|p| matches!(p.item.kind, ItemKind::Weapon(_)))
            .map(|p| p.item.uid)
            .expect("gun on body");
        let placed = loot.remove(gun).unwrap();
        assert!(eq.grid_mut(GridRef::Backpack).unwrap().insert(placed.item).is_ok());

        // Heal with the AI-2 in the pockets.
        raid.player.body.damage(BodyPart::Stomach, 40.0);
        raid.quick_heal();
        assert!(raid.heal.is_some());
        let input = Input::default();
        let settings = Settings::default();
        for _ in 0..(60 * 3) {
            raid.update(1.0 / 60.0, &input, &settings, false);
        }
        assert!(raid.heal.is_none());
        assert!(!raid.player.body.is_injured());
    }

    #[test]
    fn reload_draws_ammo_from_rig() {
        let mut raid = arena();
        let before = raid.equipment.ammo_count(AmmoType::Ps545);
        if let Some(w) = raid.equipment.weapon_mut(EquipSlot::Primary) {
            w.rounds = 5;
        }
        raid.start_reload();
        let r = raid.gun.reload.expect("reloading");
        raid.finish_reload(r);
        assert_eq!(raid.weapon().unwrap().rounds, 30);
        assert_eq!(raid.equipment.ammo_count(AmmoType::Ps545), before - 25);
    }

    #[test]
    fn gunshots_alert_scavs() {
        let mut raid = arena();
        let i = place_scav(&mut raid, Vec3::new(100.5, 21.0, 100.5));
        let stats = raid.weapon().unwrap().stats();
        raid.fire_player_shot(stats);
        assert_eq!(raid.scavs[i].state, ScavState::Investigate);
    }
}
