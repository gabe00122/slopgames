//! Item catalogue: kinds, sizes, stacking, values and descriptions.

use serde::{Deserialize, Serialize};

use crate::weapons::armor::ArmorKind;
use crate::weapons::{AmmoType, AttachmentId, ReceiverId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BackpackKind {
    Sling,
    ScavBackpack,
    Pilgrim,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RigKind {
    ScavVest,
    ChestRig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MedKind {
    Ai2,
    Salewa,
    Surv12,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BarterKind {
    Screws,
    Bolts,
    Nuts,
    DuctTape,
    Wires,
    CircuitBoard,
    WeaponParts,
    MetalScrap,
    GunpowderEagle,
    GunpowderKite,
    Battery,
    FuelCanister,
    Bitcoin,
    GoldChain,
    Matches,
    Toolset,
}

impl BarterKind {
    pub const ALL: [BarterKind; 16] = [
        BarterKind::Screws,
        BarterKind::Bolts,
        BarterKind::Nuts,
        BarterKind::DuctTape,
        BarterKind::Wires,
        BarterKind::CircuitBoard,
        BarterKind::WeaponParts,
        BarterKind::MetalScrap,
        BarterKind::GunpowderEagle,
        BarterKind::GunpowderKite,
        BarterKind::Battery,
        BarterKind::FuelCanister,
        BarterKind::Bitcoin,
        BarterKind::GoldChain,
        BarterKind::Matches,
        BarterKind::Toolset,
    ];

    fn info(self) -> (&'static str, &'static str, (u8, u8), u32, u32) {
        // (name, short, size, max stack, value)
        match self {
            BarterKind::Screws => ("Pack of screws", "Screws", (1, 1), 10, 6_000),
            BarterKind::Bolts => ("Bolts", "Bolts", (1, 1), 10, 9_000),
            BarterKind::Nuts => ("Screw nuts", "Nuts", (1, 1), 10, 7_000),
            BarterKind::DuctTape => ("Duct tape", "Tape", (1, 1), 1, 14_000),
            BarterKind::Wires => ("Bundle of wires", "Wires", (1, 1), 1, 11_000),
            BarterKind::CircuitBoard => ("Printed circuit board", "PCB", (1, 1), 1, 30_000),
            BarterKind::WeaponParts => ("Weapon parts", "WParts", (2, 1), 1, 25_000),
            BarterKind::MetalScrap => ("Metal scrap", "Scrap", (1, 1), 10, 5_000),
            BarterKind::GunpowderEagle => ("Gunpowder \"Eagle\"", "GP Eagle", (1, 2), 5, 30_000),
            BarterKind::GunpowderKite => ("Gunpowder \"Kite\"", "GP Kite", (1, 2), 5, 22_000),
            BarterKind::Battery => ("Rechargeable battery", "Battery", (1, 1), 1, 18_000),
            BarterKind::FuelCanister => ("Fuel canister", "Fuel", (2, 2), 1, 40_000),
            BarterKind::Bitcoin => ("Physical bitcoin", "0.2BTC", (1, 1), 1, 250_000),
            BarterKind::GoldChain => ("Golden neck chain", "Chain", (1, 1), 1, 40_000),
            BarterKind::Matches => ("Box of matches", "Matches", (1, 1), 10, 3_000),
            BarterKind::Toolset => ("Toolset", "Toolset", (2, 2), 1, 45_000),
        }
    }
}

impl MedKind {
    /// (name, short, size, resource, use time seconds, value)
    fn info(self) -> (&'static str, &'static str, (u8, u8), u32, f32, u32) {
        match self {
            MedKind::Ai2 => ("AI-2 medkit", "AI-2", (1, 1), 100, 2.0, 8_000),
            MedKind::Salewa => ("Salewa first aid kit", "Salewa", (1, 2), 400, 3.0, 22_000),
            MedKind::Surv12 => ("Surv12 field surgical kit", "Surv12", (3, 1), 2, 6.0, 45_000),
        }
    }

    pub fn resource(self) -> u32 {
        self.info().3
    }

    pub fn use_time(self) -> f32 {
        self.info().4
    }

    /// Surgical kits restore destroyed limbs; the rest heal HP.
    pub fn is_surgery(self) -> bool {
        self == MedKind::Surv12
    }
}

impl BackpackKind {
    fn info(self) -> (&'static str, &'static str, (u8, u8), (u8, u8), u32) {
        // (name, short, item size, grid size, value)
        match self {
            BackpackKind::Sling => ("Tactical sling bag", "Sling", (2, 3), (2, 4), 9_000),
            BackpackKind::ScavBackpack => ("Scav backpack", "Scav BP", (4, 4), (4, 5), 16_000),
            BackpackKind::Pilgrim => ("Pilgrim tourist backpack", "Pilgrim", (5, 5), (5, 6), 38_000),
        }
    }
}

impl RigKind {
    fn info(self) -> (&'static str, &'static str, (u8, u8), (u8, u8), u32) {
        match self {
            RigKind::ScavVest => ("Scav vest", "Vest", (2, 3), (4, 2), 8_000),
            RigKind::ChestRig => ("Tactical chest rig", "Chest rig", (3, 3), (4, 3), 24_000),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    Weapon(ReceiverId),
    Attachment(AttachmentId),
    Ammo(AmmoType),
    Armor(ArmorKind),
    Backpack(BackpackKind),
    Rig(RigKind),
    Med(MedKind),
    Barter(BarterKind),
    Roubles,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Weapon,
    Mod,
    Ammo,
    Gear,
    Container,
    Med,
    Barter,
    Valuable,
    Money,
}

impl ItemKind {
    pub fn name(&self) -> &'static str {
        match self {
            ItemKind::Weapon(r) => r.def().name,
            ItemKind::Attachment(a) => a.def().name,
            ItemKind::Ammo(a) => a.def().name,
            ItemKind::Armor(a) => a.def().name,
            ItemKind::Backpack(b) => b.info().0,
            ItemKind::Rig(r) => r.info().0,
            ItemKind::Med(m) => m.info().0,
            ItemKind::Barter(b) => b.info().0,
            ItemKind::Roubles => "Roubles",
        }
    }

    /// Short label shown on the item's grid cell.
    pub fn short(&self) -> String {
        match self {
            ItemKind::Weapon(r) => match r {
                ReceiverId::Grach => "Grach".into(),
                ReceiverId::Vityaz => "Vityaz".into(),
                ReceiverId::Ak74n => "AK-74N".into(),
                ReceiverId::Akm => "AKM".into(),
            },
            ItemKind::Attachment(a) => {
                let n = a.def().name;
                n.split_whitespace().take(2).collect::<Vec<_>>().join(" ")
            }
            ItemKind::Ammo(a) => {
                let cal = match a.caliber() {
                    crate::weapons::Caliber::C9x19 => "9mm",
                    crate::weapons::Caliber::C545x39 => "5.45",
                    crate::weapons::Caliber::C762x39 => "7.62",
                };
                format!("{}\n{}", a.def().short, cal)
            }
            ItemKind::Armor(a) => a.def().name.split_whitespace().next().unwrap_or("Armor").to_string(),
            ItemKind::Backpack(b) => b.info().1.into(),
            ItemKind::Rig(r) => r.info().1.into(),
            ItemKind::Med(m) => m.info().1.into(),
            ItemKind::Barter(b) => b.info().1.into(),
            ItemKind::Roubles => "RUB".into(),
        }
    }

    pub fn size(&self) -> (u8, u8) {
        match self {
            ItemKind::Weapon(r) => r.def().size,
            ItemKind::Attachment(a) => a.def().size,
            ItemKind::Ammo(_) => (1, 1),
            ItemKind::Armor(a) => a.def().size,
            ItemKind::Backpack(b) => b.info().2,
            ItemKind::Rig(r) => r.info().2,
            ItemKind::Med(m) => m.info().2,
            ItemKind::Barter(b) => b.info().2,
            ItemKind::Roubles => (1, 1),
        }
    }

    pub fn max_stack(&self) -> u32 {
        match self {
            ItemKind::Ammo(_) => 60,
            ItemKind::Barter(b) => b.info().3,
            ItemKind::Roubles => 500_000,
            _ => 1,
        }
    }

    pub fn stackable(&self) -> bool {
        self.max_stack() > 1
    }

    /// Value of one unit in roubles.
    pub fn unit_value(&self) -> u32 {
        match self {
            ItemKind::Weapon(r) => r.def().value,
            ItemKind::Attachment(a) => a.def().value,
            ItemKind::Ammo(a) => match a.def().class {
                crate::weapons::ammo::AmmoClass::Fmj => 120,
                crate::weapons::ammo::AmmoClass::HollowPoint => 150,
                crate::weapons::ammo::AmmoClass::ArmorPiercing => 450,
            },
            ItemKind::Armor(a) => a.def().value,
            ItemKind::Backpack(b) => b.info().4,
            ItemKind::Rig(r) => r.info().4,
            ItemKind::Med(m) => m.info().5,
            ItemKind::Barter(b) => b.info().4,
            ItemKind::Roubles => 1,
        }
    }

    pub fn category(&self) -> Category {
        match self {
            ItemKind::Weapon(_) => Category::Weapon,
            ItemKind::Attachment(_) => Category::Mod,
            ItemKind::Ammo(_) => Category::Ammo,
            ItemKind::Armor(_) => Category::Gear,
            ItemKind::Backpack(_) | ItemKind::Rig(_) => Category::Container,
            ItemKind::Med(_) => Category::Med,
            ItemKind::Barter(BarterKind::Bitcoin) | ItemKind::Barter(BarterKind::GoldChain) => Category::Valuable,
            ItemKind::Barter(_) => Category::Barter,
            ItemKind::Roubles => Category::Money,
        }
    }

    /// Grid size of containers (backpacks, rigs).
    pub fn grid_size(&self) -> Option<(u8, u8)> {
        match self {
            ItemKind::Backpack(b) => Some(b.info().3),
            ItemKind::Rig(r) => Some(r.info().3),
            _ => None,
        }
    }

    pub fn description(&self) -> String {
        match self {
            ItemKind::Weapon(r) => {
                let d = r.def();
                format!(
                    "{} · {:.0} rpm · {}",
                    d.caliber.name(),
                    d.fire_rate,
                    if d.full_auto { "semi/auto" } else { "semi" }
                )
            }
            ItemKind::Attachment(a) => {
                let d = a.def();
                let mut parts = vec![format!("{} attachment", d.slot.name())];
                if d.recoil != 0.0 {
                    parts.push(format!("recoil {:+.0}%", d.recoil * 100.0));
                }
                if d.ergo != 0.0 {
                    parts.push(format!("ergonomics {:+.0}", d.ergo));
                }
                if d.spread != 0.0 {
                    parts.push(format!("accuracy {:+.0}%", -d.spread * 100.0));
                }
                if d.mag_size > 0 {
                    parts.push(format!("{} rounds", d.mag_size));
                }
                if d.zoom > 1.0 {
                    parts.push(format!("{:.1}x zoom", d.zoom));
                }
                if d.loudness != 0.0 {
                    parts.push(format!("loudness {:+.0}%", d.loudness * 100.0));
                }
                let fits: Vec<&str> = d.fits.iter().map(|r| r.def().name.split_whitespace().next().unwrap_or("")).collect();
                parts.push(format!("fits: {}", fits.join(", ")));
                parts.join("\n")
            }
            ItemKind::Ammo(a) => {
                let d = a.def();
                format!(
                    "{} · damage {:.0} · armor pen {:.0} · block pen {:.0}",
                    d.class.short(),
                    d.damage,
                    d.armor_pen,
                    d.block_pen
                )
            }
            ItemKind::Armor(a) => {
                let d = a.def();
                format!(
                    "Class {} {} · durability {:.0}",
                    d.class,
                    if d.helmet { "helmet" } else { "body armor" },
                    d.max_durability
                )
            }
            ItemKind::Backpack(_) | ItemKind::Rig(_) => {
                let (w, h) = self.grid_size().unwrap_or((0, 0));
                format!("Container, {}x{} slots", w, h)
            }
            ItemKind::Med(m) => {
                if m.is_surgery() {
                    format!("Restores a destroyed limb · {} uses", m.resource())
                } else {
                    format!("Heals up to {} HP", m.resource())
                }
            }
            ItemKind::Barter(_) => "Barter / crafting material".into(),
            ItemKind::Roubles => "Russian currency".into(),
        }
    }
}
