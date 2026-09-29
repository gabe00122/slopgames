//! Calibers and ammunition types.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Caliber {
    C9x19,
    C545x39,
    C762x39,
}

impl Caliber {
    pub fn name(self) -> &'static str {
        match self {
            Caliber::C9x19 => "9x19mm",
            Caliber::C545x39 => "5.45x39mm",
            Caliber::C762x39 => "7.62x39mm",
        }
    }

    pub fn ammo_types(self) -> &'static [AmmoType] {
        match self {
            Caliber::C9x19 => &[AmmoType::Pst9, AmmoType::Rip9, AmmoType::Ap9],
            Caliber::C545x39 => &[AmmoType::Ps545, AmmoType::Hp545, AmmoType::Bs545],
            Caliber::C762x39 => &[AmmoType::Ps762, AmmoType::Hp762, AmmoType::Bp762],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AmmoClass {
    Fmj,
    HollowPoint,
    ArmorPiercing,
}

impl AmmoClass {
    pub fn short(self) -> &'static str {
        match self {
            AmmoClass::Fmj => "FMJ",
            AmmoClass::HollowPoint => "HP",
            AmmoClass::ArmorPiercing => "AP",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AmmoType {
    Pst9,
    Rip9,
    Ap9,
    Ps545,
    Hp545,
    Bs545,
    Ps762,
    Hp762,
    Bp762,
}

pub struct AmmoDef {
    pub name: &'static str,
    pub short: &'static str,
    pub caliber: Caliber,
    pub class: AmmoClass,
    /// Flesh damage per hit.
    pub damage: f32,
    /// Armor penetration power (compare with armor class * 10).
    pub armor_pen: f32,
    /// Block penetration power (compare with block pen_resist).
    pub block_pen: f32,
    /// Damage dealt to blocks on impact / pass-through.
    pub block_damage: f32,
    /// Tracer colour (sRGB).
    pub tracer: [u8; 3],
}

impl AmmoType {
    pub const ALL: [AmmoType; 9] = [
        AmmoType::Pst9,
        AmmoType::Rip9,
        AmmoType::Ap9,
        AmmoType::Ps545,
        AmmoType::Hp545,
        AmmoType::Bs545,
        AmmoType::Ps762,
        AmmoType::Hp762,
        AmmoType::Bp762,
    ];

    pub fn def(self) -> &'static AmmoDef {
        match self {
            AmmoType::Pst9 => &AmmoDef {
                name: "9x19 PST gzh (FMJ)",
                short: "PST",
                caliber: Caliber::C9x19,
                class: AmmoClass::Fmj,
                damage: 50.0,
                armor_pen: 20.0,
                block_pen: 12.0,
                block_damage: 7.0,
                tracer: [255, 226, 150],
            },
            AmmoType::Rip9 => &AmmoDef {
                name: "9x19 RIP (HP)",
                short: "RIP",
                caliber: Caliber::C9x19,
                class: AmmoClass::HollowPoint,
                damage: 82.0,
                armor_pen: 3.0,
                block_pen: 3.0,
                block_damage: 4.0,
                tracer: [255, 190, 120],
            },
            AmmoType::Ap9 => &AmmoDef {
                name: "9x19 AP 6.3 (AP)",
                short: "AP 6.3",
                caliber: Caliber::C9x19,
                class: AmmoClass::ArmorPiercing,
                damage: 44.0,
                armor_pen: 32.0,
                block_pen: 32.0,
                block_damage: 12.0,
                tracer: [170, 230, 255],
            },
            AmmoType::Ps545 => &AmmoDef {
                name: "5.45x39 PS gs (FMJ)",
                short: "PS",
                caliber: Caliber::C545x39,
                class: AmmoClass::Fmj,
                damage: 53.0,
                armor_pen: 23.0,
                block_pen: 18.0,
                block_damage: 10.0,
                tracer: [255, 226, 150],
            },
            AmmoType::Hp545 => &AmmoDef {
                name: "5.45x39 HP (HP)",
                short: "HP",
                caliber: Caliber::C545x39,
                class: AmmoClass::HollowPoint,
                damage: 76.0,
                armor_pen: 9.0,
                block_pen: 4.0,
                block_damage: 6.0,
                tracer: [255, 190, 120],
            },
            AmmoType::Bs545 => &AmmoDef {
                name: "5.45x39 BS gs (AP)",
                short: "BS",
                caliber: Caliber::C545x39,
                class: AmmoClass::ArmorPiercing,
                damage: 43.0,
                armor_pen: 52.0,
                block_pen: 55.0,
                block_damage: 22.0,
                tracer: [170, 230, 255],
            },
            AmmoType::Ps762 => &AmmoDef {
                name: "7.62x39 PS gzh (FMJ)",
                short: "PS",
                caliber: Caliber::C762x39,
                class: AmmoClass::Fmj,
                damage: 58.0,
                armor_pen: 33.0,
                block_pen: 26.0,
                block_damage: 14.0,
                tracer: [255, 226, 150],
            },
            AmmoType::Hp762 => &AmmoDef {
                name: "7.62x39 HP (HP)",
                short: "HP",
                caliber: Caliber::C762x39,
                class: AmmoClass::HollowPoint,
                damage: 84.0,
                armor_pen: 14.0,
                block_pen: 6.0,
                block_damage: 8.0,
                tracer: [255, 190, 120],
            },
            AmmoType::Bp762 => &AmmoDef {
                name: "7.62x39 BP gzh (AP)",
                short: "BP",
                caliber: Caliber::C762x39,
                class: AmmoClass::ArmorPiercing,
                damage: 58.0,
                armor_pen: 47.0,
                block_pen: 66.0,
                block_damage: 28.0,
                tracer: [170, 230, 255],
            },
        }
    }

    pub fn caliber(self) -> Caliber {
        self.def().caliber
    }
}
