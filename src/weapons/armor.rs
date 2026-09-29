//! Helmets and body armor: classes, durability and penetration rolls.

use serde::{Deserialize, Serialize};

use crate::player::BodyPart;
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArmorKind {
    // Helmets
    Beanie6b47Lite,
    Kiver,
    Ssh68,
    Ulach,
    // Body armor
    Paca,
    Zhuk3,
    Kora,
    Gzhel,
}

pub struct ArmorDef {
    pub name: &'static str,
    pub class: u8,
    pub max_durability: f32,
    pub helmet: bool,
    /// Movement / ergonomics penalty (0..1).
    pub weight: f32,
    pub size: (u8, u8),
    pub value: u32,
}

impl ArmorKind {
    pub fn def(self) -> &'static ArmorDef {
        match self {
            ArmorKind::Beanie6b47Lite => &ArmorDef {
                name: "6B47 Ratnik-BSh helmet",
                class: 3,
                max_durability: 45.0,
                helmet: true,
                weight: 0.03,
                size: (2, 2),
                value: 18_000,
            },
            ArmorKind::Kiver => &ArmorDef {
                name: "Kiver-M helmet",
                class: 3,
                max_durability: 32.0,
                helmet: true,
                weight: 0.04,
                size: (2, 2),
                value: 9_000,
            },
            ArmorKind::Ssh68 => &ArmorDef {
                name: "SSh-68 steel helmet",
                class: 2,
                max_durability: 30.0,
                helmet: true,
                weight: 0.05,
                size: (2, 2),
                value: 4_000,
            },
            ArmorKind::Ulach => &ArmorDef {
                name: "Ulach IIIA helmet",
                class: 4,
                max_durability: 50.0,
                helmet: true,
                weight: 0.05,
                size: (2, 2),
                value: 45_000,
            },
            ArmorKind::Paca => &ArmorDef {
                name: "PACA soft armor",
                class: 2,
                max_durability: 50.0,
                helmet: false,
                weight: 0.03,
                size: (3, 3),
                value: 12_000,
            },
            ArmorKind::Zhuk3 => &ArmorDef {
                name: "Zhuk-3 press armor",
                class: 3,
                max_durability: 55.0,
                helmet: false,
                weight: 0.05,
                size: (3, 3),
                value: 22_000,
            },
            ArmorKind::Kora => &ArmorDef {
                name: "Kora-Kulon armor",
                class: 4,
                max_durability: 64.0,
                helmet: false,
                weight: 0.07,
                size: (3, 3),
                value: 48_000,
            },
            ArmorKind::Gzhel => &ArmorDef {
                name: "Gzhel-K armor",
                class: 5,
                max_durability: 65.0,
                helmet: false,
                weight: 0.09,
                size: (3, 3),
                value: 95_000,
            },
        }
    }

    pub fn covers(self, part: BodyPart) -> bool {
        if self.def().helmet {
            part == BodyPart::Head
        } else {
            matches!(part, BodyPart::Thorax | BodyPart::Stomach)
        }
    }
}

/// A worn armor piece with its current durability.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ArmorState {
    pub kind: ArmorKind,
    pub durability: f32,
}

impl ArmorState {
    pub fn new(kind: ArmorKind) -> Self {
        Self {
            kind,
            durability: kind.def().max_durability,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ArmorResult {
    pub damage: f32,
    /// Armor absorbed the hit (blunt damage only).
    pub blocked: bool,
}

/// Resolve a hit against (optional) armor covering the part.
pub fn resolve_armor(
    armor: Option<&mut ArmorState>,
    part: BodyPart,
    damage: f32,
    pen: f32,
    rng: &mut Rng,
) -> ArmorResult {
    let Some(a) = armor else {
        return ArmorResult {
            damage,
            blocked: false,
        };
    };
    if !a.kind.covers(part) || a.durability <= 0.0 {
        return ArmorResult {
            damage,
            blocked: false,
        };
    }
    let def = a.kind.def();
    let dur_frac = (a.durability / def.max_durability).clamp(0.0, 1.0);
    let effective = def.class as f32 * 10.0 * (0.55 + 0.45 * dur_frac);
    let chance = (0.5 + (pen - effective) / 12.0).clamp(0.03, 0.97);
    if rng.chance(chance) {
        a.durability = (a.durability - damage * 0.12 - 1.0).max(0.0);
        ArmorResult {
            damage: damage * (1.0 - 0.04 * def.class as f32),
            blocked: false,
        }
    } else {
        a.durability = (a.durability - pen * 0.18 - 2.0).max(0.0);
        ArmorResult {
            damage: damage * 0.18,
            blocked: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ap_beats_armor_more_often_than_hp() {
        let mut rng = Rng::new(3);
        let trials = 400;
        let mut hp_pens = 0;
        let mut ap_pens = 0;
        for _ in 0..trials {
            let mut a = ArmorState::new(ArmorKind::Kora);
            if !resolve_armor(Some(&mut a), BodyPart::Thorax, 76.0, 9.0, &mut rng).blocked {
                hp_pens += 1;
            }
            let mut a = ArmorState::new(ArmorKind::Kora);
            if !resolve_armor(Some(&mut a), BodyPart::Thorax, 43.0, 52.0, &mut rng).blocked {
                ap_pens += 1;
            }
        }
        assert!(ap_pens > trials * 8 / 10, "ap {ap_pens}");
        assert!(hp_pens < trials / 10, "hp {hp_pens}");
    }

    #[test]
    fn armor_does_not_cover_limbs() {
        let mut rng = Rng::new(1);
        let mut a = ArmorState::new(ArmorKind::Gzhel);
        let r = resolve_armor(Some(&mut a), BodyPart::LeftLeg, 50.0, 5.0, &mut rng);
        assert!(!r.blocked);
        assert_eq!(r.damage, 50.0);
    }
}
