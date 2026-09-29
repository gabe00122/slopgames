//! Loot tables for containers and scav bodies.

use super::{BackpackKind, BarterKind, Grid, Item, ItemKind, MedKind, RigKind};
use crate::ai::Scav;
use crate::rng::Rng;
use crate::weapons::armor::ArmorKind;
use crate::weapons::{AmmoType, AttachmentId, ReceiverId, Slot, Weapon};
use crate::world::ContainerKind;

fn random_ammo(rng: &mut Rng) -> Item {
    let a = *rng.pick(&AmmoType::ALL);
    let n = match a.def().class {
        crate::weapons::ammo::AmmoClass::ArmorPiercing => rng.range_i32(10, 30),
        _ => rng.range_i32(20, 60),
    } as u32;
    Item::stack(ItemKind::Ammo(a), n)
}

fn random_barter(rng: &mut Rng, rare: bool) -> Item {
    let weights: Vec<u32> = BarterKind::ALL
        .iter()
        .map(|b| match b {
            BarterKind::Screws
            | BarterKind::Bolts
            | BarterKind::Nuts
            | BarterKind::MetalScrap
            | BarterKind::Matches => 10,
            BarterKind::DuctTape | BarterKind::Wires => 7,
            BarterKind::GunpowderKite | BarterKind::Battery => 4,
            BarterKind::WeaponParts | BarterKind::GunpowderEagle | BarterKind::CircuitBoard => {
                if rare {
                    5
                } else {
                    2
                }
            }
            BarterKind::FuelCanister | BarterKind::Toolset => 2,
            BarterKind::GoldChain => {
                if rare {
                    3
                } else {
                    1
                }
            }
            BarterKind::Bitcoin => {
                if rare {
                    1
                } else {
                    0
                }
            }
        })
        .collect();
    let b = BarterKind::ALL[rng.weighted(&weights)];
    let kind = ItemKind::Barter(b);
    let n = if kind.max_stack() > 1 {
        rng.range_i32(1, 3) as u32
    } else {
        1
    };
    Item::stack(kind, n)
}

fn random_attachment(rng: &mut Rng) -> Item {
    // Cheaper parts are more common.
    let weights: Vec<u32> = AttachmentId::ALL
        .iter()
        .map(|a| (60_000 / a.def().value.max(1500)).max(1))
        .collect();
    Item::new(ItemKind::Attachment(AttachmentId::ALL[rng.weighted(&weights)]))
}

fn random_weapon(rng: &mut Rng) -> Item {
    let r = ReceiverId::ALL[rng.weighted(&[25, 25, 30, 20])];
    let mut w = Weapon::new(r);
    // Occasionally strip or upgrade parts.
    if rng.chance(0.3) && r.has_slot(Slot::Stock) {
        w.set_attachment(Slot::Stock, None);
    }
    if rng.chance(0.2) && r.has_slot(Slot::Sight) {
        let sights: Vec<AttachmentId> = AttachmentId::ALL
            .iter()
            .copied()
            .filter(|a| a.def().slot == Slot::Sight && a.fits(r))
            .collect();
        if !sights.is_empty() {
            w.set_attachment(Slot::Sight, Some(*rng.pick(&sights)));
        }
    }
    if rng.chance(0.4) {
        let ammo = r.def().caliber.ammo_types()[rng.range_usize(0, 2)];
        let n = rng.range_i32(0, w.capacity() as i32) as u32;
        w = w.loaded_with(ammo, n);
    }
    Item::with_weapon(w)
}

fn random_med(rng: &mut Rng) -> Item {
    let m = match rng.weighted(&[60, 30, 10]) {
        0 => MedKind::Ai2,
        1 => MedKind::Salewa,
        _ => MedKind::Surv12,
    };
    Item::new(ItemKind::Med(m))
}

/// A random item of any category (used for Fence's rotating assortment).
pub fn random_trade_item(rng: &mut Rng) -> Item {
    match rng.weighted(&[20, 25, 20, 15, 15, 5]) {
        0 => random_weapon(rng),
        1 => random_attachment(rng),
        2 => random_ammo(rng),
        3 => random_med(rng),
        4 => random_barter(rng, true),
        _ => {
            let kinds = [
                ArmorKind::Paca,
                ArmorKind::Zhuk3,
                ArmorKind::Kiver,
                ArmorKind::Ssh68,
                ArmorKind::Beanie6b47Lite,
            ];
            let k = *rng.pick(&kinds);
            let mut it = Item::new(ItemKind::Armor(k));
            // Fence's armor is used.
            it.durability = Some(k.def().max_durability * rng.range_f32(0.45, 0.9));
            it
        }
    }
}

fn fill(grid: &mut Grid, items: Vec<Item>) {
    for it in items {
        let _ = grid.insert(it);
    }
}

/// Generate the contents of a container block.
pub fn container_loot(kind: ContainerKind, rng: &mut Rng) -> Grid {
    let (w, h) = kind.grid_size();
    let mut grid = Grid::new(w, h);
    let mut items = Vec::new();
    match kind {
        ContainerKind::WoodenCrate => {
            for _ in 0..rng.range_i32(2, 5) {
                items.push(random_barter(rng, false));
            }
            for _ in 0..rng.range_i32(0, 2) {
                items.push(random_ammo(rng));
            }
            if rng.chance(0.15) {
                items.push(random_med(rng));
            }
            if rng.chance(0.2) {
                items.push(random_attachment(rng));
            }
            if rng.chance(0.25) {
                items.push(Item::stack(ItemKind::Roubles, rng.range_i32(2, 20) as u32 * 1000));
            }
        }
        ContainerKind::WeaponBox => {
            if rng.chance(0.55) {
                items.push(random_weapon(rng));
            }
            for _ in 0..rng.range_i32(1, 3) {
                items.push(random_attachment(rng));
            }
            for _ in 0..rng.range_i32(1, 3) {
                items.push(random_ammo(rng));
            }
            if rng.chance(0.5) {
                items.push(Item::new(ItemKind::Barter(BarterKind::WeaponParts)));
            }
            if rng.chance(0.35) {
                items.push(Item::stack(
                    ItemKind::Barter(if rng.chance(0.5) {
                        BarterKind::GunpowderKite
                    } else {
                        BarterKind::GunpowderEagle
                    }),
                    rng.range_i32(1, 2) as u32,
                ));
            }
        }
        ContainerKind::MedCase => {
            for _ in 0..rng.range_i32(1, 3) {
                items.push(random_med(rng));
            }
            if rng.chance(0.3) {
                items.push(random_barter(rng, false));
            }
        }
        ContainerKind::FileCabinet => {
            if rng.chance(0.7) {
                items.push(Item::stack(ItemKind::Roubles, rng.range_i32(5, 60) as u32 * 1000));
            }
            for _ in 0..rng.range_i32(1, 3) {
                let mut b = random_barter(rng, true);
                if matches!(
                    b.kind,
                    ItemKind::Barter(BarterKind::FuelCanister) | ItemKind::Barter(BarterKind::Toolset)
                ) {
                    b = Item::new(ItemKind::Barter(BarterKind::CircuitBoard));
                }
                items.push(b);
            }
        }
    }
    fill(&mut grid, items);
    grid
}

/// Everything a dead scav carries, as a lootable grid.
pub fn scav_body_loot(scav: &Scav, rng: &mut Rng) -> Grid {
    let mut grid = Grid::new(7, 7);
    let mut items = vec![Item::with_weapon(scav.weapon.clone())];
    if let Some(a) = scav.armor {
        items.push(Item::armor(a));
    }
    if let Some(h) = scav.helmet {
        items.push(Item::armor(h));
    }
    // Spare ammo for their gun.
    if let Some(ammo) = scav.weapon.loaded {
        items.push(Item::stack(ItemKind::Ammo(ammo), rng.range_i32(10, 45) as u32));
    }
    if rng.chance(0.3) {
        items.push(Item::new(ItemKind::Rig(RigKind::ScavVest)));
    }
    if rng.chance(0.2) {
        let bp = if rng.chance(0.8) {
            BackpackKind::ScavBackpack
        } else {
            BackpackKind::Pilgrim
        };
        let mut b = Item::new(ItemKind::Backpack(bp));
        if let Some(g) = b.contents.as_mut() {
            for _ in 0..rng.range_i32(1, 3) {
                let _ = g.insert(random_barter(rng, false));
            }
        }
        items.push(b);
    }
    for _ in 0..rng.range_i32(0, 2) {
        items.push(random_barter(rng, false));
    }
    if rng.chance(0.3) {
        items.push(random_med(rng));
    }
    if rng.chance(0.4) {
        items.push(Item::stack(ItemKind::Roubles, rng.range_i32(1, 15) as u32 * 1000));
    }
    if rng.chance(0.08) {
        items.push(Item::new(ItemKind::Armor(ArmorKind::Kiver)));
    }
    fill(&mut grid, items);
    grid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn containers_produce_loot() {
        let mut rng = Rng::new(11);
        for kind in [
            ContainerKind::WoodenCrate,
            ContainerKind::WeaponBox,
            ContainerKind::MedCase,
            ContainerKind::FileCabinet,
        ] {
            let mut total = 0;
            for _ in 0..20 {
                total += container_loot(kind, &mut rng).items.len();
            }
            assert!(total > 10, "{kind:?} produced too little loot");
        }
    }

    #[test]
    fn scav_bodies_have_their_gun() {
        let mut rng = Rng::new(3);
        let s = Scav::spawn(0, glam::Vec3::ZERO, &mut rng);
        let g = scav_body_loot(&s, &mut rng);
        assert!(g.items.iter().any(|p| matches!(p.item.kind, ItemKind::Weapon(_))));
    }
}
