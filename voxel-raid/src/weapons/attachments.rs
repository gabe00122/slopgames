//! Attachment slots and attachment definitions.

use serde::{Deserialize, Serialize};

use super::ReceiverId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Slot {
    Barrel,
    Muzzle,
    Stock,
    Sight,
    Magazine,
    Grip,
}

impl Slot {
    pub const ALL: [Slot; 6] = [
        Slot::Barrel,
        Slot::Muzzle,
        Slot::Stock,
        Slot::Sight,
        Slot::Magazine,
        Slot::Grip,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Slot::Barrel => "Barrel",
            Slot::Muzzle => "Muzzle",
            Slot::Stock => "Stock",
            Slot::Sight => "Sight",
            Slot::Magazine => "Magazine",
            Slot::Grip => "Foregrip",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttachmentId {
    // Barrels
    Ak74Barrel415,
    Ak74Barrel206,
    AkmBarrel415,
    AkmBarrel520,
    // Muzzle devices
    Ak74Brake,
    AkmSlantBrake,
    Dtk1Compensator,
    Pp19Brake,
    Pbs4Suppressor,
    Pbs1Suppressor,
    Suppressor9mm,
    // Stocks
    AkWoodStock,
    Ak74PolymerStock,
    ZhukovStock,
    Pp19FoldingStock,
    Pp19CqbStock,
    // Sights
    CobraRedDot,
    Rmr,
    Holo1p87,
    Pso1Scope,
    // Magazines
    GrachMag17,
    PistolMag33,
    Pp19Mag20,
    Pp19Mag30,
    Ak74Mag30,
    Ak74Mag45,
    Rpk16Drum95,
    AkmMag30,
    AkmMag40,
    AkmDrum75,
    // Foregrips
    Rk2Grip,
    Rk6Grip,
    AfgGrip,
}

pub struct AttachmentDef {
    pub name: &'static str,
    pub slot: Slot,
    pub fits: &'static [ReceiverId],
    /// Fractional recoil modifier (-0.1 = 10% less recoil).
    pub recoil: f32,
    /// Additive ergonomics.
    pub ergo: f32,
    /// Fractional spread (accuracy) modifier.
    pub spread: f32,
    /// Fractional loudness modifier (suppressors).
    pub loudness: f32,
    /// Fractional reload time modifier.
    pub reload: f32,
    pub weight: f32,
    pub mag_size: u32,
    pub zoom: f32,
    pub size: (u8, u8),
    pub value: u32,
}

const AK: &[ReceiverId] = &[ReceiverId::Ak74n, ReceiverId::Akm];
const AK74: &[ReceiverId] = &[ReceiverId::Ak74n];
const AKM: &[ReceiverId] = &[ReceiverId::Akm];
const PP19: &[ReceiverId] = &[ReceiverId::Vityaz];
const PISTOL: &[ReceiverId] = &[ReceiverId::Grach];
const NINE: &[ReceiverId] = &[ReceiverId::Grach, ReceiverId::Vityaz];
const RIFLES: &[ReceiverId] = &[ReceiverId::Ak74n, ReceiverId::Akm, ReceiverId::Vityaz];

/// Default attachment stats; a macro (not a const fn) so that
/// `&AttachmentDef { ..base!(..) }` is promoted to a `'static` reference.
macro_rules! base {
    ($name:expr, $slot:expr, $fits:expr) => {
        AttachmentDef {
            name: $name,
            slot: $slot,
            fits: $fits,
            recoil: 0.0,
            ergo: 0.0,
            spread: 0.0,
            loudness: 0.0,
            reload: 0.0,
            weight: 0.1,
            mag_size: 0,
            zoom: 1.0,
            size: (1, 1),
            value: 3000,
        }
    };
}

impl AttachmentId {
    pub const ALL: [AttachmentId; 33] = [
        AttachmentId::Ak74Barrel415,
        AttachmentId::Ak74Barrel206,
        AttachmentId::AkmBarrel415,
        AttachmentId::AkmBarrel520,
        AttachmentId::Ak74Brake,
        AttachmentId::AkmSlantBrake,
        AttachmentId::Dtk1Compensator,
        AttachmentId::Pp19Brake,
        AttachmentId::Pbs4Suppressor,
        AttachmentId::Pbs1Suppressor,
        AttachmentId::Suppressor9mm,
        AttachmentId::AkWoodStock,
        AttachmentId::Ak74PolymerStock,
        AttachmentId::ZhukovStock,
        AttachmentId::Pp19FoldingStock,
        AttachmentId::Pp19CqbStock,
        AttachmentId::CobraRedDot,
        AttachmentId::Rmr,
        AttachmentId::Holo1p87,
        AttachmentId::Pso1Scope,
        AttachmentId::GrachMag17,
        AttachmentId::PistolMag33,
        AttachmentId::Pp19Mag20,
        AttachmentId::Pp19Mag30,
        AttachmentId::Ak74Mag30,
        AttachmentId::Ak74Mag45,
        AttachmentId::Rpk16Drum95,
        AttachmentId::AkmMag30,
        AttachmentId::AkmMag40,
        AttachmentId::AkmDrum75,
        AttachmentId::Rk2Grip,
        AttachmentId::Rk6Grip,
        AttachmentId::AfgGrip,
    ];

    pub fn def(self) -> &'static AttachmentDef {
        use AttachmentId::*;
        match self {
            Ak74Barrel415 => &AttachmentDef {
                weight: 0.6,
                size: (3, 1),
                value: 6000,
                ..base!("AK-74 415mm barrel", Slot::Barrel, AK74)
            },
            Ak74Barrel206 => &AttachmentDef {
                spread: 0.45,
                recoil: 0.1,
                ergo: 8.0,
                weight: 0.35,
                size: (2, 1),
                value: 9000,
                ..base!("AK-74 206mm short barrel", Slot::Barrel, AK74)
            },
            AkmBarrel415 => &AttachmentDef {
                weight: 0.65,
                size: (3, 1),
                value: 6000,
                ..base!("AKM 415mm barrel", Slot::Barrel, AKM)
            },
            AkmBarrel520 => &AttachmentDef {
                spread: -0.35,
                recoil: -0.05,
                ergo: -6.0,
                weight: 0.85,
                size: (4, 1),
                value: 14000,
                ..base!("RPK 520mm long barrel", Slot::Barrel, AKM)
            },
            Ak74Brake => &AttachmentDef {
                recoil: -0.1,
                ergo: -1.0,
                value: 2500,
                ..base!("AK-74 muzzle brake", Slot::Muzzle, AK74)
            },
            AkmSlantBrake => &AttachmentDef {
                recoil: -0.08,
                ergo: -1.0,
                value: 2500,
                ..base!("AKM slant brake", Slot::Muzzle, AKM)
            },
            Dtk1Compensator => &AttachmentDef {
                recoil: -0.18,
                ergo: -3.0,
                value: 12000,
                ..base!("Zenit DTK-1 compensator", Slot::Muzzle, AK)
            },
            Pp19Brake => &AttachmentDef {
                recoil: -0.07,
                ergo: -1.0,
                value: 2500,
                ..base!("PP-19 muzzle brake", Slot::Muzzle, PP19)
            },
            Pbs4Suppressor => &AttachmentDef {
                recoil: -0.12,
                ergo: -8.0,
                loudness: -0.65,
                weight: 0.5,
                size: (2, 1),
                value: 22000,
                ..base!("PBS-4 5.45 suppressor", Slot::Muzzle, AK74)
            },
            Pbs1Suppressor => &AttachmentDef {
                recoil: -0.15,
                ergo: -9.0,
                loudness: -0.65,
                weight: 0.6,
                size: (2, 1),
                value: 24000,
                ..base!("PBS-1 7.62 suppressor", Slot::Muzzle, AKM)
            },
            Suppressor9mm => &AttachmentDef {
                recoil: -0.08,
                ergo: -5.0,
                loudness: -0.7,
                weight: 0.3,
                size: (2, 1),
                value: 15000,
                ..base!("Rotor 43 9mm suppressor", Slot::Muzzle, NINE)
            },
            AkWoodStock => &AttachmentDef {
                recoil: -0.25,
                ergo: 2.0,
                weight: 0.5,
                size: (2, 1),
                value: 3000,
                ..base!("AK wooden stock", Slot::Stock, AK)
            },
            Ak74PolymerStock => &AttachmentDef {
                recoil: -0.28,
                ergo: 3.0,
                weight: 0.4,
                size: (2, 1),
                value: 4000,
                ..base!("AK-74 polymer stock", Slot::Stock, AK)
            },
            ZhukovStock => &AttachmentDef {
                recoil: -0.34,
                ergo: 7.0,
                weight: 0.35,
                size: (2, 1),
                value: 16000,
                ..base!("Magpul Zhukov-S stock", Slot::Stock, AK)
            },
            Pp19FoldingStock => &AttachmentDef {
                recoil: -0.25,
                ergo: 3.0,
                weight: 0.3,
                size: (2, 1),
                value: 3000,
                ..base!("PP-19 folding stock", Slot::Stock, PP19)
            },
            Pp19CqbStock => &AttachmentDef {
                recoil: -0.33,
                ergo: 5.0,
                weight: 0.3,
                size: (2, 1),
                value: 11000,
                ..base!("PP-19 CQB stock", Slot::Stock, PP19)
            },
            CobraRedDot => &AttachmentDef {
                spread: -0.08,
                ergo: -1.0,
                value: 9000,
                ..base!("Cobra EKP-8 red dot", Slot::Sight, RIFLES)
            },
            Rmr => &AttachmentDef {
                spread: -0.08,
                value: 8000,
                ..base!("Trijicon RMR red dot", Slot::Sight, PISTOL)
            },
            Holo1p87 => &AttachmentDef {
                spread: -0.12,
                ergo: -2.0,
                zoom: 1.5,
                value: 14000,
                ..base!("Valday 1P87 holographic", Slot::Sight, RIFLES)
            },
            Pso1Scope => &AttachmentDef {
                spread: -0.2,
                ergo: -6.0,
                zoom: 4.0,
                weight: 0.6,
                size: (2, 1),
                value: 21000,
                ..base!("PSO-1 4x scope", Slot::Sight, AK)
            },
            GrachMag17 => &AttachmentDef {
                mag_size: 17,
                value: 1500,
                ..base!("Grach 17-round magazine", Slot::Magazine, PISTOL)
            },
            PistolMag33 => &AttachmentDef {
                mag_size: 33,
                ergo: -4.0,
                reload: 0.1,
                size: (1, 2),
                value: 7000,
                ..base!("Grach 33-round extended magazine", Slot::Magazine, PISTOL)
            },
            Pp19Mag20 => &AttachmentDef {
                mag_size: 20,
                ergo: 2.0,
                reload: -0.05,
                size: (1, 2),
                value: 2000,
                ..base!("PP-19 20-round magazine", Slot::Magazine, PP19)
            },
            Pp19Mag30 => &AttachmentDef {
                mag_size: 30,
                size: (1, 2),
                value: 3000,
                ..base!("PP-19 30-round magazine", Slot::Magazine, PP19)
            },
            Ak74Mag30 => &AttachmentDef {
                mag_size: 30,
                size: (1, 2),
                value: 3000,
                ..base!("AK-74 30-round magazine", Slot::Magazine, AK74)
            },
            Ak74Mag45 => &AttachmentDef {
                mag_size: 45,
                ergo: -5.0,
                reload: 0.1,
                size: (1, 2),
                value: 9000,
                ..base!("RPK-74 45-round magazine", Slot::Magazine, AK74)
            },
            Rpk16Drum95 => &AttachmentDef {
                mag_size: 95,
                ergo: -12.0,
                reload: 0.3,
                weight: 1.2,
                size: (2, 2),
                value: 30000,
                ..base!("RPK-16 95-round drum", Slot::Magazine, AK74)
            },
            AkmMag30 => &AttachmentDef {
                mag_size: 30,
                size: (1, 2),
                value: 3000,
                ..base!("AKM 30-round magazine", Slot::Magazine, AKM)
            },
            AkmMag40 => &AttachmentDef {
                mag_size: 40,
                ergo: -4.0,
                reload: 0.08,
                size: (1, 2),
                value: 8000,
                ..base!("RPK 40-round magazine", Slot::Magazine, AKM)
            },
            AkmDrum75 => &AttachmentDef {
                mag_size: 75,
                ergo: -12.0,
                reload: 0.3,
                weight: 1.3,
                size: (2, 2),
                value: 26000,
                ..base!("RPK 75-round drum", Slot::Magazine, AKM)
            },
            Rk2Grip => &AttachmentDef {
                recoil: -0.04,
                ergo: 4.0,
                value: 4000,
                ..base!("Zenit RK-2 foregrip", Slot::Grip, RIFLES)
            },
            Rk6Grip => &AttachmentDef {
                recoil: -0.07,
                ergo: 2.0,
                value: 6000,
                ..base!("Zenit RK-6 foregrip", Slot::Grip, RIFLES)
            },
            AfgGrip => &AttachmentDef {
                recoil: -0.03,
                ergo: 7.0,
                value: 8000,
                ..base!("Magpul AFG grip", Slot::Grip, RIFLES)
            },
        }
    }

    pub fn fits(self, r: ReceiverId) -> bool {
        self.def().fits.contains(&r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_list_is_complete_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for a in AttachmentId::ALL {
            assert!(seen.insert(a));
            assert!(!a.def().fits.is_empty());
            if a.def().slot == Slot::Magazine {
                assert!(a.def().mag_size > 0);
            }
        }
    }
}
