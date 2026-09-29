//! Hideout: upgradeable stations, crafting recipes and the buildable bunker.

pub mod world;

use serde::{Deserialize, Serialize};

use crate::inventory::{BarterKind, Grid, Item, ItemKind, MedKind};
use crate::weapons::{AmmoType, AttachmentId, ReceiverId, Weapon};
use crate::world::Block;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StationKind {
    Workbench,
    AmmoPress,
    Medstation,
}

pub type Cost = Vec<(ItemKind, u32)>;

fn b(kind: BarterKind, n: u32) -> (ItemKind, u32) {
    (ItemKind::Barter(kind), n)
}

fn rub(n: u32) -> (ItemKind, u32) {
    (ItemKind::Roubles, n)
}

impl StationKind {
    pub const ALL: [StationKind; 3] = [StationKind::Workbench, StationKind::AmmoPress, StationKind::Medstation];

    pub fn name(self) -> &'static str {
        match self {
            StationKind::Workbench => "Workbench",
            StationKind::AmmoPress => "Ammo Press",
            StationKind::Medstation => "Medstation",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            StationKind::Workbench => "Machine weapon parts and attachments from scrap.",
            StationKind::AmmoPress => "Reload spent casings into fresh cartridges.",
            StationKind::Medstation => "Assemble medical supplies.",
        }
    }

    pub fn block(self) -> Block {
        match self {
            StationKind::Workbench => Block::Workbench,
            StationKind::AmmoPress => Block::AmmoPress,
            StationKind::Medstation => Block::Medstation,
        }
    }

    pub fn from_block(b: Block) -> Option<StationKind> {
        match b {
            Block::Workbench => Some(StationKind::Workbench),
            Block::AmmoPress => Some(StationKind::AmmoPress),
            Block::Medstation => Some(StationKind::Medstation),
            _ => None,
        }
    }

    pub fn max_level(self) -> u32 {
        match self {
            StationKind::Medstation => 2,
            _ => 3,
        }
    }

    /// Items required to go from `level - 1` to `level`.
    pub fn upgrade_cost(self, level: u32) -> Option<Cost> {
        use BarterKind::*;
        let c = match (self, level) {
            (StationKind::Workbench, 1) => vec![b(Screws, 4), b(Bolts, 2), rub(20_000)],
            (StationKind::Workbench, 2) => vec![b(WeaponParts, 2), b(Toolset, 1), b(Nuts, 3), rub(60_000)],
            (StationKind::Workbench, 3) => vec![b(CircuitBoard, 2), b(WeaponParts, 2), b(FuelCanister, 1), rub(150_000)],
            (StationKind::AmmoPress, 1) => vec![b(MetalScrap, 3), b(Bolts, 1), rub(25_000)],
            (StationKind::AmmoPress, 2) => vec![b(Toolset, 1), b(GunpowderKite, 2), b(Nuts, 2), rub(50_000)],
            (StationKind::AmmoPress, 3) => vec![b(GunpowderEagle, 2), b(CircuitBoard, 1), b(WeaponParts, 1), rub(120_000)],
            (StationKind::Medstation, 1) => vec![b(DuctTape, 1), b(Matches, 2), rub(15_000)],
            (StationKind::Medstation, 2) => vec![b(CircuitBoard, 1), b(Battery, 1), b(Wires, 1), rub(40_000)],
            _ => return None,
        };
        Some(c)
    }
}

#[derive(Clone, Debug)]
pub struct Recipe {
    pub station: StationKind,
    pub level: u32,
    pub inputs: Cost,
    pub output: ItemKind,
    pub count: u32,
}

impl Recipe {
    pub fn name(&self) -> String {
        if self.count > 1 {
            format!("{} x{}", self.output.name(), self.count)
        } else {
            self.output.name().to_string()
        }
    }
}

/// All crafting recipes, unlocked by station level.
pub fn recipes() -> Vec<Recipe> {
    use BarterKind::*;
    let r = |station, level, inputs: Cost, output, count| Recipe {
        station,
        level,
        inputs,
        output,
        count,
    };
    let att = ItemKind::Attachment;
    let ammo = ItemKind::Ammo;
    let wb = StationKind::Workbench;
    let ap = StationKind::AmmoPress;
    let md = StationKind::Medstation;
    vec![
        // Workbench: attachments (and eventually whole receivers).
        r(wb, 1, vec![b(Screws, 2), b(DuctTape, 1)], att(AttachmentId::Rk2Grip), 1),
        r(wb, 1, vec![b(MetalScrap, 2), b(Bolts, 1)], att(AttachmentId::Ak74Brake), 1),
        r(wb, 1, vec![b(MetalScrap, 2), b(Screws, 1)], att(AttachmentId::AkmSlantBrake), 1),
        r(wb, 1, vec![b(Wires, 1), b(Battery, 1), b(Screws, 2)], att(AttachmentId::CobraRedDot), 1),
        r(wb, 2, vec![b(WeaponParts, 1), b(MetalScrap, 3), b(DuctTape, 1)], att(AttachmentId::Pbs4Suppressor), 1),
        r(wb, 2, vec![b(WeaponParts, 1), b(MetalScrap, 2)], att(AttachmentId::Suppressor9mm), 1),
        r(wb, 2, vec![b(MetalScrap, 2), b(Screws, 2), b(Bolts, 1)], att(AttachmentId::Ak74Mag45), 1),
        r(wb, 2, vec![b(DuctTape, 1), b(Screws, 2), b(Nuts, 1)], att(AttachmentId::ZhukovStock), 1),
        r(wb, 2, vec![b(MetalScrap, 2), b(Nuts, 2)], att(AttachmentId::Dtk1Compensator), 1),
        r(wb, 3, vec![b(CircuitBoard, 1), b(Battery, 1), b(Wires, 2)], att(AttachmentId::Pso1Scope), 1),
        r(wb, 3, vec![b(CircuitBoard, 1), b(Wires, 2), b(Battery, 1)], att(AttachmentId::Holo1p87), 1),
        r(wb, 3, vec![b(WeaponParts, 1), b(MetalScrap, 4), b(Bolts, 2)], att(AttachmentId::Rpk16Drum95), 1),
        r(wb, 3, vec![b(WeaponParts, 2), b(MetalScrap, 4), b(Bolts, 2)], ItemKind::Weapon(ReceiverId::Ak74n), 1),
        // Ammo press.
        r(ap, 1, vec![b(GunpowderKite, 1), b(MetalScrap, 1)], ammo(AmmoType::Pst9), 60),
        r(ap, 1, vec![b(GunpowderKite, 1), b(MetalScrap, 2)], ammo(AmmoType::Ps545), 60),
        r(ap, 1, vec![b(GunpowderKite, 1), b(MetalScrap, 2)], ammo(AmmoType::Ps762), 60),
        r(ap, 2, vec![b(GunpowderKite, 1), b(MetalScrap, 1), b(Screws, 1)], ammo(AmmoType::Rip9), 60),
        r(ap, 2, vec![b(GunpowderKite, 1), b(MetalScrap, 2), b(Screws, 1)], ammo(AmmoType::Hp545), 60),
        r(ap, 2, vec![b(GunpowderKite, 1), b(MetalScrap, 2), b(Screws, 1)], ammo(AmmoType::Hp762), 60),
        r(ap, 3, vec![b(GunpowderEagle, 1), b(MetalScrap, 2), b(Bolts, 1)], ammo(AmmoType::Ap9), 40),
        r(ap, 3, vec![b(GunpowderEagle, 1), b(MetalScrap, 2), b(Bolts, 1)], ammo(AmmoType::Bs545), 40),
        r(ap, 3, vec![b(GunpowderEagle, 1), b(MetalScrap, 3), b(Bolts, 1)], ammo(AmmoType::Bp762), 40),
        // Medstation.
        r(md, 1, vec![b(DuctTape, 1), rub(5_000)], ItemKind::Med(MedKind::Ai2), 1),
        r(md, 2, vec![b(DuctTape, 2), b(Wires, 1)], ItemKind::Med(MedKind::Salewa), 1),
        r(md, 2, vec![b(Toolset, 1), b(DuctTape, 2)], ItemKind::Med(MedKind::Surv12), 1),
    ]
}

/// Saved hideout state.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct HideoutState {
    #[serde(default)]
    pub levels: Vec<(StationKind, u32)>,
    /// Player block edits relative to the generated hideout: [x, y, z, block id].
    #[serde(default)]
    pub edits: Vec<[i32; 4]>,
}

impl HideoutState {
    pub fn level(&self, s: StationKind) -> u32 {
        self.levels.iter().find(|(k, _)| *k == s).map(|(_, l)| *l).unwrap_or(0)
    }

    pub fn set_level(&mut self, s: StationKind, level: u32) {
        if let Some(e) = self.levels.iter_mut().find(|(k, _)| *k == s) {
            e.1 = level;
        } else {
            self.levels.push((s, level));
        }
    }
}

pub fn have(stash: &Grid, kind: ItemKind) -> u32 {
    stash.count_kind(kind)
}

pub fn can_afford(stash: &Grid, cost: &[(ItemKind, u32)]) -> bool {
    cost.iter().all(|(k, n)| have(stash, *k) >= *n)
}

fn pay(stash: &mut Grid, cost: &[(ItemKind, u32)]) {
    for (k, n) in cost {
        stash.take_kind(*k, *n);
    }
}

/// Upgrade a station, paying from the stash. Returns the new level.
pub fn upgrade(state: &mut HideoutState, stash: &mut Grid, station: StationKind) -> Result<u32, String> {
    let next = state.level(station) + 1;
    if next > station.max_level() {
        return Err(format!("{} is already at max level", station.name()));
    }
    let cost = station.upgrade_cost(next).ok_or("No upgrade available")?;
    if !can_afford(stash, &cost) {
        return Err("Missing required items in your stash".into());
    }
    pay(stash, &cost);
    state.set_level(station, next);
    Ok(next)
}

fn make_output(r: &Recipe) -> Vec<Item> {
    match r.output {
        ItemKind::Weapon(rec) => vec![Item::with_weapon(Weapon::new(rec))],
        k if k.stackable() => {
            let mut out = Vec::new();
            let mut left = r.count;
            while left > 0 {
                let n = left.min(k.max_stack());
                out.push(Item::stack(k, n));
                left -= n;
            }
            out
        }
        k => (0..r.count).map(|_| Item::new(k)).collect(),
    }
}

/// Craft a recipe instantly: pay the inputs from the stash and put the output there.
pub fn craft(state: &HideoutState, stash: &mut Grid, r: &Recipe) -> Result<(), String> {
    if state.level(r.station) < r.level {
        return Err(format!("Requires {} level {}", r.station.name(), r.level));
    }
    if !can_afford(stash, &r.inputs) {
        return Err("Missing ingredients".into());
    }
    // Dry run on a copy so a full stash never eats the ingredients.
    let mut trial = stash.clone();
    pay(&mut trial, &r.inputs);
    for it in make_output(r) {
        trial.insert(it).map_err(|_| "Not enough space in the stash".to_string())?;
    }
    *stash = trial;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stash_with(items: &[(ItemKind, u32)]) -> Grid {
        let mut g = Grid::new(10, 20);
        for (k, n) in items {
            let mut left = *n;
            while left > 0 {
                let c = left.min(k.max_stack());
                g.insert(Item::stack(*k, c)).unwrap();
                left -= c;
            }
        }
        g
    }

    #[test]
    fn upgrade_costs_items_and_unlocks_recipes() {
        let mut state = HideoutState::default();
        let mut stash = stash_with(&[
            (ItemKind::Barter(BarterKind::Screws), 6),
            (ItemKind::Barter(BarterKind::Bolts), 2),
            (ItemKind::Barter(BarterKind::DuctTape), 1),
            (ItemKind::Roubles, 30_000),
        ]);
        let grip = recipes()
            .into_iter()
            .find(|r| r.output == ItemKind::Attachment(AttachmentId::Rk2Grip))
            .unwrap();
        assert!(craft(&state, &mut stash, &grip).is_err(), "locked at level 0");
        assert_eq!(upgrade(&mut state, &mut stash, StationKind::Workbench), Ok(1));
        assert_eq!(have(&stash, ItemKind::Barter(BarterKind::Bolts)), 0);
        assert_eq!(have(&stash, ItemKind::Roubles), 10_000);
        assert!(upgrade(&mut state, &mut stash, StationKind::Workbench).is_err(), "can't afford level 2");
        craft(&state, &mut stash, &grip).unwrap();
        assert_eq!(have(&stash, ItemKind::Attachment(AttachmentId::Rk2Grip)), 1);
        assert_eq!(have(&stash, ItemKind::Barter(BarterKind::Screws)), 0);
        assert!(craft(&state, &mut stash, &grip).is_err(), "out of ingredients");
    }

    #[test]
    fn ammo_press_makes_stacks() {
        let mut state = HideoutState::default();
        state.set_level(StationKind::AmmoPress, 3);
        let mut stash = stash_with(&[
            (ItemKind::Barter(BarterKind::GunpowderEagle), 1),
            (ItemKind::Barter(BarterKind::MetalScrap), 2),
            (ItemKind::Barter(BarterKind::Bolts), 1),
        ]);
        let bs = recipes()
            .into_iter()
            .find(|r| r.output == ItemKind::Ammo(AmmoType::Bs545))
            .unwrap();
        craft(&state, &mut stash, &bs).unwrap();
        assert_eq!(have(&stash, ItemKind::Ammo(AmmoType::Bs545)), 40);
    }

    #[test]
    fn full_stash_keeps_ingredients() {
        let mut state = HideoutState::default();
        state.set_level(StationKind::Workbench, 3);
        let mut stash = Grid::new(3, 1);
        stash.insert(Item::stack(ItemKind::Barter(BarterKind::WeaponParts), 1)).unwrap();
        stash.insert(Item::stack(ItemKind::Barter(BarterKind::MetalScrap), 3)).unwrap();
        let supp = recipes()
            .into_iter()
            .find(|r| r.output == ItemKind::Attachment(AttachmentId::Suppressor9mm))
            .unwrap();
        // Suppressor is 2x1; after paying there's a 2x1 hole... but scrap remains (1 left), so no room.
        let before = stash.clone();
        let res = craft(&state, &mut stash, &supp);
        if res.is_err() {
            assert_eq!(stash, before);
        }
    }

    #[test]
    fn every_station_level_has_a_cost_and_recipes() {
        for s in StationKind::ALL {
            for lvl in 1..=s.max_level() {
                assert!(s.upgrade_cost(lvl).is_some(), "{s:?} {lvl}");
                assert!(recipes().iter().any(|r| r.station == s && r.level == lvl), "{s:?} {lvl}");
            }
            assert!(s.upgrade_cost(s.max_level() + 1).is_none());
        }
    }
}
