//! Tarkov-style grid inventory: items occupy WxH cells in containers.

pub mod items;
pub mod loot;

use serde::{Deserialize, Serialize};

pub use items::{BackpackKind, BarterKind, Category, ItemKind, MedKind, RigKind};

use crate::weapons::armor::{ArmorKind, ArmorState};
use crate::weapons::{AmmoType, AttachmentId, PartSource, Weapon};

/// A concrete item instance.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub uid: u64,
    pub kind: ItemKind,
    pub count: u32,
    #[serde(default)]
    pub weapon: Option<Weapon>,
    #[serde(default)]
    pub durability: Option<f32>,
    /// Remaining med resource (HP or uses).
    #[serde(default)]
    pub uses: Option<u32>,
    /// Contents of backpacks and rigs.
    #[serde(default)]
    pub contents: Option<Grid>,
}

pub fn new_uid() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    t.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (c << 1) ^ c.wrapping_mul(0xBF58_476D_1CE4_E5B9)
}

impl Item {
    pub fn new(kind: ItemKind) -> Self {
        let mut it = Self {
            uid: new_uid(),
            kind,
            count: 1,
            weapon: None,
            durability: None,
            uses: None,
            contents: None,
        };
        match kind {
            ItemKind::Weapon(r) => it.weapon = Some(Weapon::new(r)),
            ItemKind::Armor(a) => it.durability = Some(a.def().max_durability),
            ItemKind::Med(m) => it.uses = Some(m.resource()),
            ItemKind::Backpack(_) | ItemKind::Rig(_) => {
                let (w, h) = kind.grid_size().unwrap_or((1, 1));
                it.contents = Some(Grid::new(w, h));
            }
            _ => {}
        }
        it
    }

    pub fn stack(kind: ItemKind, count: u32) -> Self {
        let mut it = Self::new(kind);
        it.count = count.clamp(1, kind.max_stack());
        it
    }

    pub fn with_weapon(w: Weapon) -> Self {
        let mut it = Self::new(ItemKind::Weapon(w.receiver));
        it.weapon = Some(w);
        it
    }

    pub fn armor(state: ArmorState) -> Self {
        let mut it = Self::new(ItemKind::Armor(state.kind));
        it.durability = Some(state.durability);
        it
    }

    pub fn armor_state(&self) -> Option<ArmorState> {
        match self.kind {
            ItemKind::Armor(k) => Some(ArmorState {
                kind: k,
                durability: self.durability.unwrap_or(k.def().max_durability),
            }),
            _ => None,
        }
    }

    pub fn size(&self) -> (u8, u8) {
        self.kind.size()
    }

    pub fn name(&self) -> &'static str {
        self.kind.name()
    }

    /// Total value including stack size, weapon parts and container contents.
    pub fn value(&self) -> u64 {
        let base = match (&self.weapon, self.kind) {
            (Some(w), _) => w.value() as u64 + w.rounds as u64 * 100,
            (None, ItemKind::Armor(a)) => {
                let d = a.def();
                let frac = self.durability.unwrap_or(d.max_durability) / d.max_durability;
                (d.value as f32 * (0.3 + 0.7 * frac)) as u64
            }
            (None, k) => k.unit_value() as u64 * self.count as u64,
        };
        base + self.contents.as_ref().map(|g| g.total_value()).unwrap_or(0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Placed {
    pub item: Item,
    pub x: u8,
    pub y: u8,
    pub rotated: bool,
}

impl Placed {
    pub fn size(&self) -> (u8, u8) {
        let (w, h) = self.item.size();
        if self.rotated {
            (h, w)
        } else {
            (w, h)
        }
    }
}

/// A rectangular container grid.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Grid {
    pub w: u8,
    pub h: u8,
    pub items: Vec<Placed>,
}

impl Grid {
    pub fn new(w: u8, h: u8) -> Self {
        Self {
            w,
            h,
            items: Vec::new(),
        }
    }

    fn occupancy(&self, ignore: Option<u64>) -> Vec<bool> {
        let mut occ = vec![false; self.w as usize * self.h as usize];
        for p in &self.items {
            if Some(p.item.uid) == ignore {
                continue;
            }
            let (w, h) = p.size();
            for y in p.y..(p.y + h).min(self.h) {
                for x in p.x..(p.x + w).min(self.w) {
                    occ[y as usize * self.w as usize + x as usize] = true;
                }
            }
        }
        occ
    }

    fn fits_occ(&self, occ: &[bool], size: (u8, u8), x: i32, y: i32) -> bool {
        let (w, h) = (size.0 as i32, size.1 as i32);
        if x < 0 || y < 0 || x + w > self.w as i32 || y + h > self.h as i32 {
            return false;
        }
        for yy in y..y + h {
            for xx in x..x + w {
                if occ[yy as usize * self.w as usize + xx as usize] {
                    return false;
                }
            }
        }
        true
    }

    /// Can an item of `size` (already rotated) be placed at (x, y)?
    pub fn can_place(&self, size: (u8, u8), x: i32, y: i32, ignore: Option<u64>) -> bool {
        let occ = self.occupancy(ignore);
        self.fits_occ(&occ, size, x, y)
    }

    pub fn place(&mut self, item: Item, x: u8, y: u8, rotated: bool) -> Result<(), Item> {
        let (w, h) = item.size();
        let size = if rotated { (h, w) } else { (w, h) };
        if !self.can_place(size, x as i32, y as i32, None) {
            return Err(item);
        }
        self.items.push(Placed { item, x, y, rotated });
        Ok(())
    }

    /// Find a free spot (top-left first, trying both orientations).
    pub fn find_spot(&self, size: (u8, u8), avoid: Option<(u8, u8, u8, u8)>) -> Option<(u8, u8, bool)> {
        let mut occ = self.occupancy(None);
        if let Some((ax, ay, aw, ah)) = avoid {
            for y in ay..(ay + ah).min(self.h) {
                for x in ax..(ax + aw).min(self.w) {
                    occ[y as usize * self.w as usize + x as usize] = true;
                }
            }
        }
        for y in 0..self.h as i32 {
            for x in 0..self.w as i32 {
                if self.fits_occ(&occ, size, x, y) {
                    return Some((x as u8, y as u8, false));
                }
                if size.0 != size.1 && self.fits_occ(&occ, (size.1, size.0), x, y) {
                    return Some((x as u8, y as u8, true));
                }
            }
        }
        None
    }

    /// Merge into existing stacks, then place the remainder in free space.
    pub fn insert(&mut self, item: Item) -> Result<(), Item> {
        self.insert_avoiding(item, None)
    }

    pub fn insert_avoiding(&mut self, mut item: Item, avoid: Option<(u8, u8, u8, u8)>) -> Result<(), Item> {
        if item.kind.stackable() {
            let max = item.kind.max_stack();
            for p in self.items.iter_mut() {
                if p.item.kind == item.kind && p.item.count < max {
                    let add = (max - p.item.count).min(item.count);
                    p.item.count += add;
                    item.count -= add;
                    if item.count == 0 {
                        return Ok(());
                    }
                }
            }
        }
        match self.find_spot(item.size(), avoid) {
            Some((x, y, rot)) => {
                self.items.push(Placed {
                    item,
                    x,
                    y,
                    rotated: rot,
                });
                Ok(())
            }
            None => Err(item),
        }
    }

    pub fn remove(&mut self, uid: u64) -> Option<Placed> {
        let i = self.items.iter().position(|p| p.item.uid == uid)?;
        Some(self.items.remove(i))
    }

    pub fn get(&self, uid: u64) -> Option<&Placed> {
        self.items.iter().find(|p| p.item.uid == uid)
    }

    pub fn get_mut(&mut self, uid: u64) -> Option<&mut Placed> {
        self.items.iter_mut().find(|p| p.item.uid == uid)
    }

    pub fn item_at(&self, x: u8, y: u8) -> Option<&Placed> {
        self.items.iter().find(|p| {
            let (w, h) = p.size();
            x >= p.x && x < p.x + w && y >= p.y && y < p.y + h
        })
    }

    pub fn count_kind(&self, kind: ItemKind) -> u32 {
        self.items
            .iter()
            .filter(|p| p.item.kind == kind)
            .map(|p| p.item.count)
            .sum()
    }

    /// Remove up to `n` units of a kind (from stacks or single items).
    pub fn take_kind(&mut self, kind: ItemKind, n: u32) -> u32 {
        let mut left = n;
        for p in self.items.iter_mut() {
            if left == 0 {
                break;
            }
            if p.item.kind == kind {
                let t = p.item.count.min(left);
                p.item.count -= t;
                left -= t;
            }
        }
        self.items.retain(|p| p.item.count > 0);
        n - left
    }

    pub fn total_value(&self) -> u64 {
        self.items.iter().map(|p| p.item.value()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Sort items: large first, then by category, repacking from the top-left.
    pub fn sort(&mut self) {
        let mut items: Vec<Item> = self.items.drain(..).map(|p| p.item).collect();
        items.sort_by_key(|i| {
            let (w, h) = i.size();
            (
                std::cmp::Reverse(w as u32 * h as u32),
                i.kind.category() as u8,
                i.name(),
            )
        });
        let mut leftovers = Vec::new();
        for it in items {
            if let Err(it) = self.insert(it) {
                leftovers.push(it);
            }
        }
        // Should never happen (same items fit before), but never lose items.
        for it in leftovers {
            self.items.push(Placed {
                item: it,
                x: 0,
                y: 0,
                rotated: false,
            });
        }
    }
}

/// Stash grids act as a source of attachments for the modding screen.
pub struct GridParts<'a> {
    pub grid: &'a mut Grid,
    /// Cells that must stay free (the weapon being modded came from here).
    pub reserved: Option<(u8, u8, u8, u8)>,
}

impl PartSource for GridParts<'_> {
    fn count(&self, a: AttachmentId) -> u32 {
        self.grid.count_kind(ItemKind::Attachment(a))
    }

    fn take(&mut self, a: AttachmentId) -> bool {
        self.grid.take_kind(ItemKind::Attachment(a), 1) == 1
    }

    fn give(&mut self, a: AttachmentId) -> bool {
        self.grid
            .insert_avoiding(Item::new(ItemKind::Attachment(a)), self.reserved)
            .is_ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipSlot {
    Primary,
    Holster,
    Helmet,
    Armor,
    Rig,
    Backpack,
}

impl EquipSlot {
    pub const ALL: [EquipSlot; 6] = [
        EquipSlot::Primary,
        EquipSlot::Holster,
        EquipSlot::Helmet,
        EquipSlot::Armor,
        EquipSlot::Rig,
        EquipSlot::Backpack,
    ];

    pub fn name(self) -> &'static str {
        match self {
            EquipSlot::Primary => "Primary weapon",
            EquipSlot::Holster => "Holster",
            EquipSlot::Helmet => "Headwear",
            EquipSlot::Armor => "Body armor",
            EquipSlot::Rig => "Tactical rig",
            EquipSlot::Backpack => "Backpack",
        }
    }

    pub fn accepts(self, kind: ItemKind) -> bool {
        match (self, kind) {
            (EquipSlot::Primary, ItemKind::Weapon(r)) => !r.def().pistol,
            (EquipSlot::Holster, ItemKind::Weapon(r)) => r.def().pistol,
            (EquipSlot::Helmet, ItemKind::Armor(a)) => a.def().helmet,
            (EquipSlot::Armor, ItemKind::Armor(a)) => !a.def().helmet,
            (EquipSlot::Rig, ItemKind::Rig(_)) => true,
            (EquipSlot::Backpack, ItemKind::Backpack(_)) => true,
            _ => false,
        }
    }
}

/// Grids the player carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GridRef {
    Stash,
    Rig,
    Backpack,
    Pockets,
    Pouch,
    Loot,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Equipment {
    pub primary: Option<Item>,
    pub holster: Option<Item>,
    pub helmet: Option<Item>,
    pub armor: Option<Item>,
    pub rig: Option<Item>,
    pub backpack: Option<Item>,
    pub pockets: Grid,
    /// Small belt pouch (extra carry space; lost on death like everything else).
    pub pouch: Grid,
}

impl Default for Equipment {
    fn default() -> Self {
        Self {
            primary: None,
            holster: None,
            helmet: None,
            armor: None,
            rig: None,
            backpack: None,
            pockets: Grid::new(4, 1),
            pouch: Grid::new(2, 2),
        }
    }
}

impl Equipment {
    pub fn slot(&self, s: EquipSlot) -> &Option<Item> {
        match s {
            EquipSlot::Primary => &self.primary,
            EquipSlot::Holster => &self.holster,
            EquipSlot::Helmet => &self.helmet,
            EquipSlot::Armor => &self.armor,
            EquipSlot::Rig => &self.rig,
            EquipSlot::Backpack => &self.backpack,
        }
    }

    pub fn slot_mut(&mut self, s: EquipSlot) -> &mut Option<Item> {
        match s {
            EquipSlot::Primary => &mut self.primary,
            EquipSlot::Holster => &mut self.holster,
            EquipSlot::Helmet => &mut self.helmet,
            EquipSlot::Armor => &mut self.armor,
            EquipSlot::Rig => &mut self.rig,
            EquipSlot::Backpack => &mut self.backpack,
        }
    }

    pub fn grid(&self, g: GridRef) -> Option<&Grid> {
        match g {
            GridRef::Rig => self.rig.as_ref().and_then(|i| i.contents.as_ref()),
            GridRef::Backpack => self.backpack.as_ref().and_then(|i| i.contents.as_ref()),
            GridRef::Pockets => Some(&self.pockets),
            GridRef::Pouch => Some(&self.pouch),
            _ => None,
        }
    }

    pub fn grid_mut(&mut self, g: GridRef) -> Option<&mut Grid> {
        match g {
            GridRef::Rig => self.rig.as_mut().and_then(|i| i.contents.as_mut()),
            GridRef::Backpack => self.backpack.as_mut().and_then(|i| i.contents.as_mut()),
            GridRef::Pockets => Some(&mut self.pockets),
            GridRef::Pouch => Some(&mut self.pouch),
            _ => None,
        }
    }

    /// Carried grids in the order ammo/meds are searched.
    pub const POUCHES: [GridRef; 4] = [GridRef::Rig, GridRef::Pockets, GridRef::Pouch, GridRef::Backpack];

    pub fn count_kind(&self, kind: ItemKind) -> u32 {
        Self::POUCHES
            .iter()
            .filter_map(|g| self.grid(*g))
            .map(|g| g.count_kind(kind))
            .sum()
    }

    pub fn take_kind(&mut self, kind: ItemKind, n: u32) -> u32 {
        let mut got = 0;
        for g in Self::POUCHES {
            if got >= n {
                break;
            }
            if let Some(grid) = self.grid_mut(g) {
                got += grid.take_kind(kind, n - got);
            }
        }
        got
    }

    /// Put an item into the first carried grid with space.
    pub fn stow(&mut self, mut item: Item) -> Result<(), Item> {
        for g in [GridRef::Rig, GridRef::Pockets, GridRef::Pouch, GridRef::Backpack] {
            if let Some(grid) = self.grid_mut(g) {
                match grid.insert(item) {
                    Ok(()) => return Ok(()),
                    Err(back) => item = back,
                }
            }
        }
        Err(item)
    }

    pub fn ammo_count(&self, a: AmmoType) -> u32 {
        self.count_kind(ItemKind::Ammo(a))
    }

    pub fn weapon(&self, s: EquipSlot) -> Option<&Weapon> {
        self.slot(s).as_ref().and_then(|i| i.weapon.as_ref())
    }

    pub fn weapon_mut(&mut self, s: EquipSlot) -> Option<&mut Weapon> {
        self.slot_mut(s).as_mut().and_then(|i| i.weapon.as_mut())
    }

    pub fn armor_state(&self, s: EquipSlot) -> Option<ArmorState> {
        self.slot(s).as_ref().and_then(|i| i.armor_state())
    }

    pub fn set_durability(&mut self, s: EquipSlot, d: f32) {
        if let Some(i) = self.slot_mut(s).as_mut() {
            i.durability = Some(d);
        }
    }

    /// First usable med item across carried grids: (grid, uid, kind).
    pub fn find_med(&self, want_surgery: bool) -> Option<(GridRef, u64, MedKind)> {
        for g in Self::POUCHES {
            if let Some(grid) = self.grid(g) {
                for p in &grid.items {
                    if let ItemKind::Med(m) = p.item.kind {
                        if m.is_surgery() == want_surgery && p.item.uses.unwrap_or(0) > 0 {
                            return Some((g, p.item.uid, m));
                        }
                    }
                }
            }
        }
        None
    }

    pub fn total_value(&self) -> u64 {
        let slots: u64 = EquipSlot::ALL
            .iter()
            .filter_map(|s| self.slot(*s).as_ref())
            .map(|i| i.value())
            .sum();
        slots + self.pockets.total_value() + self.pouch.total_value()
    }

    /// Dying loses everything that was brought into the raid.
    pub fn strip_on_death(&mut self) {
        *self = Equipment::default();
    }

    pub fn has_any_weapon(&self) -> bool {
        self.primary.is_some() || self.holster.is_some()
    }
}

/// A new player's starting stash and loadout.
pub fn starter_profile_items() -> (Grid, Equipment) {
    use crate::weapons::ReceiverId;
    let mut stash = Grid::new(10, 30);
    let mut eq = Equipment::default();

    eq.primary = Some(Item::with_weapon(
        Weapon::new(ReceiverId::Ak74n).loaded_with(AmmoType::Ps545, 30),
    ));
    eq.holster = Some(Item::with_weapon(
        Weapon::new(ReceiverId::Grach).loaded_with(AmmoType::Pst9, 17),
    ));
    eq.helmet = Some(Item::new(ItemKind::Armor(ArmorKind::Kiver)));
    eq.armor = Some(Item::new(ItemKind::Armor(ArmorKind::Paca)));
    let mut rig = Item::new(ItemKind::Rig(RigKind::ScavVest));
    if let Some(g) = rig.contents.as_mut() {
        let _ = g.insert(Item::stack(ItemKind::Ammo(AmmoType::Ps545), 60));
        let _ = g.insert(Item::stack(ItemKind::Ammo(AmmoType::Ps545), 30));
        let _ = g.insert(Item::stack(ItemKind::Ammo(AmmoType::Pst9), 34));
    }
    eq.rig = Some(rig);
    eq.backpack = Some(Item::new(ItemKind::Backpack(BackpackKind::ScavBackpack)));
    let _ = eq.pockets.insert(Item::new(ItemKind::Med(MedKind::Ai2)));

    let stash_items = [
        Item::with_weapon(Weapon::new(ReceiverId::Vityaz).loaded_with(AmmoType::Pst9, 30)),
        Item::stack(ItemKind::Ammo(AmmoType::Ps545), 60),
        Item::stack(ItemKind::Ammo(AmmoType::Hp545), 60),
        Item::stack(ItemKind::Ammo(AmmoType::Bs545), 30),
        Item::stack(ItemKind::Ammo(AmmoType::Pst9), 60),
        Item::stack(ItemKind::Ammo(AmmoType::Rip9), 30),
        Item::new(ItemKind::Attachment(AttachmentId::CobraRedDot)),
        Item::new(ItemKind::Attachment(AttachmentId::Rk2Grip)),
        Item::new(ItemKind::Attachment(AttachmentId::Ak74Mag45)),
        Item::new(ItemKind::Attachment(AttachmentId::Pp19CqbStock)),
        Item::new(ItemKind::Med(MedKind::Ai2)),
        Item::new(ItemKind::Med(MedKind::Ai2)),
        Item::new(ItemKind::Med(MedKind::Salewa)),
        Item::new(ItemKind::Armor(ArmorKind::Ssh68)),
        Item::new(ItemKind::Rig(RigKind::ScavVest)),
        Item::new(ItemKind::Backpack(BackpackKind::Sling)),
        Item::stack(ItemKind::Barter(BarterKind::Screws), 4),
        Item::stack(ItemKind::Barter(BarterKind::Bolts), 2),
        Item::stack(ItemKind::Barter(BarterKind::MetalScrap), 3),
        Item::new(ItemKind::Barter(BarterKind::DuctTape)),
        Item::stack(ItemKind::Roubles, 150_000),
    ];
    for it in stash_items {
        let _ = stash.insert(it);
    }
    (stash, eq)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_occupy_cells() {
        let mut g = Grid::new(4, 4);
        let ak = Item::new(ItemKind::Weapon(crate::weapons::ReceiverId::Ak74n)); // 5x2
        assert!(
            g.clone().place(ak.clone(), 0, 0, false).is_err(),
            "5 wide cannot fit in 4"
        );
        let mut tall = Grid::new(2, 5);
        assert!(tall.place(ak, 0, 0, true).is_ok(), "rotated 2x5 fits");
        let med = Item::new(ItemKind::Med(MedKind::Salewa)); // 1x2
        assert!(g.place(med.clone(), 3, 3, false).is_err(), "would overflow bottom");
        assert!(g.place(med, 3, 2, false).is_ok());
        assert!(!g.can_place((1, 1), 3, 3, None));
        assert!(g.can_place((1, 1), 2, 3, None));
    }

    #[test]
    fn stacks_merge_and_take() {
        let mut g = Grid::new(3, 1);
        g.insert(Item::stack(ItemKind::Ammo(AmmoType::Ps545), 50)).unwrap();
        g.insert(Item::stack(ItemKind::Ammo(AmmoType::Ps545), 30)).unwrap();
        assert_eq!(g.items.len(), 2);
        assert_eq!(g.count_kind(ItemKind::Ammo(AmmoType::Ps545)), 80);
        assert_eq!(g.take_kind(ItemKind::Ammo(AmmoType::Ps545), 70), 70);
        assert_eq!(g.count_kind(ItemKind::Ammo(AmmoType::Ps545)), 10);
        assert_eq!(g.items.len(), 1);
    }

    #[test]
    fn grid_fills_up() {
        let mut g = Grid::new(2, 2);
        for _ in 0..4 {
            g.insert(Item::new(ItemKind::Barter(BarterKind::DuctTape))).unwrap();
        }
        assert!(g.insert(Item::new(ItemKind::Barter(BarterKind::DuctTape))).is_err());
    }

    #[test]
    fn death_loses_everything() {
        let (_, mut eq) = starter_profile_items();
        assert!(eq.has_any_weapon());
        eq.pouch.insert(Item::stack(ItemKind::Roubles, 1000)).unwrap();
        eq.strip_on_death();
        assert!(!eq.has_any_weapon());
        assert!(eq.rig.is_none());
        assert!(eq.pouch.is_empty() && eq.pockets.is_empty());
        assert_eq!(eq.total_value(), 0);
    }

    #[test]
    fn stash_parts_source() {
        let mut stash = Grid::new(4, 4);
        stash
            .insert(Item::new(ItemKind::Attachment(AttachmentId::Ak74Mag45)))
            .unwrap();
        let mut w = Weapon::new(crate::weapons::ReceiverId::Ak74n);
        let mut src = GridParts {
            grid: &mut stash,
            reserved: None,
        };
        crate::weapons::swap_attachment(
            &mut w,
            crate::weapons::Slot::Magazine,
            Some(AttachmentId::Ak74Mag45),
            &mut src,
        )
        .unwrap();
        assert_eq!(stash.count_kind(ItemKind::Attachment(AttachmentId::Ak74Mag30)), 1);
        assert_eq!(stash.count_kind(ItemKind::Attachment(AttachmentId::Ak74Mag45)), 0);
    }

    #[test]
    fn json_round_trip() {
        let (stash, eq) = starter_profile_items();
        let s = serde_json::to_string(&(stash.clone(), eq.clone())).unwrap();
        let (s2, e2): (Grid, Equipment) = serde_json::from_str(&s).unwrap();
        assert_eq!(stash, s2);
        assert_eq!(eq, e2);
    }
}
