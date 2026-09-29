//! Modular weapons: receivers, attachments, ammo, armor and ballistics.

pub mod ammo;
pub mod armor;
pub mod attachments;
pub mod ballistics;

use glam::Vec2;
use serde::{Deserialize, Serialize};

pub use ammo::{AmmoType, Caliber};
pub use attachments::{AttachmentId, Slot};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReceiverId {
    Grach,
    Vityaz,
    Ak74n,
    Akm,
}

pub struct ReceiverDef {
    pub name: &'static str,
    pub caliber: Caliber,
    /// Rounds per minute.
    pub fire_rate: f32,
    pub full_auto: bool,
    /// Degrees of muzzle climb per shot with no stock/attachments.
    pub vertical_recoil: f32,
    pub horizontal_recoil: f32,
    pub ergonomics: f32,
    /// Base dispersion when aiming (degrees).
    pub spread: f32,
    pub reload_time: f32,
    pub weight: f32,
    pub slots: &'static [Slot],
    pub defaults: &'static [AttachmentId],
    pub pattern_phase: f32,
    pub size: (u8, u8),
    pub value: u32,
    pub pistol: bool,
}

impl ReceiverId {
    pub const ALL: [ReceiverId; 4] = [ReceiverId::Grach, ReceiverId::Vityaz, ReceiverId::Ak74n, ReceiverId::Akm];

    pub fn def(self) -> &'static ReceiverDef {
        match self {
            ReceiverId::Grach => &ReceiverDef {
                name: "MP-443 Grach 9x19",
                caliber: Caliber::C9x19,
                fire_rate: 420.0,
                full_auto: false,
                vertical_recoil: 1.35,
                horizontal_recoil: 0.5,
                ergonomics: 82.0,
                spread: 0.38,
                reload_time: 1.7,
                weight: 0.9,
                slots: &[Slot::Muzzle, Slot::Sight, Slot::Magazine],
                defaults: &[AttachmentId::GrachMag17],
                pattern_phase: 0.3,
                size: (2, 1),
                value: 14_000,
                pistol: true,
            },
            ReceiverId::Vityaz => &ReceiverDef {
                name: "PP-19-01 Vityaz 9x19",
                caliber: Caliber::C9x19,
                fire_rate: 700.0,
                full_auto: true,
                vertical_recoil: 0.8,
                horizontal_recoil: 0.36,
                ergonomics: 62.0,
                spread: 0.3,
                reload_time: 2.4,
                weight: 2.9,
                slots: &[Slot::Muzzle, Slot::Stock, Slot::Sight, Slot::Magazine, Slot::Grip],
                defaults: &[AttachmentId::Pp19Brake, AttachmentId::Pp19FoldingStock, AttachmentId::Pp19Mag30],
                pattern_phase: 1.7,
                size: (4, 2),
                value: 28_000,
                pistol: false,
            },
            ReceiverId::Ak74n => &ReceiverDef {
                name: "AK-74N 5.45x39",
                caliber: Caliber::C545x39,
                fire_rate: 650.0,
                full_auto: true,
                vertical_recoil: 1.05,
                horizontal_recoil: 0.44,
                ergonomics: 50.0,
                spread: 0.2,
                reload_time: 2.7,
                weight: 3.3,
                slots: &[Slot::Barrel, Slot::Muzzle, Slot::Stock, Slot::Sight, Slot::Magazine, Slot::Grip],
                defaults: &[
                    AttachmentId::Ak74Barrel415,
                    AttachmentId::Ak74Brake,
                    AttachmentId::Ak74PolymerStock,
                    AttachmentId::Ak74Mag30,
                ],
                pattern_phase: 2.9,
                size: (5, 2),
                value: 42_000,
                pistol: false,
            },
            ReceiverId::Akm => &ReceiverDef {
                name: "AKM 7.62x39",
                caliber: Caliber::C762x39,
                fire_rate: 600.0,
                full_auto: true,
                vertical_recoil: 1.45,
                horizontal_recoil: 0.6,
                ergonomics: 46.0,
                spread: 0.22,
                reload_time: 2.9,
                weight: 3.6,
                slots: &[Slot::Barrel, Slot::Muzzle, Slot::Stock, Slot::Sight, Slot::Magazine, Slot::Grip],
                defaults: &[
                    AttachmentId::AkmBarrel415,
                    AttachmentId::AkmSlantBrake,
                    AttachmentId::AkWoodStock,
                    AttachmentId::AkmMag30,
                ],
                pattern_phase: 4.1,
                size: (5, 2),
                value: 38_000,
                pistol: false,
            },
        }
    }

    pub fn has_slot(self, slot: Slot) -> bool {
        self.def().slots.contains(&slot)
    }

    /// Deterministic per-weapon recoil pattern: x = horizontal, y = vertical multiplier.
    pub fn recoil_pattern(self, shot: u32) -> Vec2 {
        let i = shot as f32;
        let ph = self.def().pattern_phase;
        let v = 1.0 + 0.35 * (-i / 3.0).exp();
        let h = (i * 0.55 + ph).sin() * 0.9 + (i * 1.7 + ph * 2.0).sin() * 0.35;
        Vec2::new(h, v)
    }
}

/// Derived weapon statistics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponStats {
    pub fire_rate: f32,
    pub full_auto: bool,
    pub vertical_recoil: f32,
    pub horizontal_recoil: f32,
    pub ergonomics: f32,
    pub spread: f32,
    pub hip_spread: f32,
    pub ads_time: f32,
    pub mag_size: u32,
    pub reload_time: f32,
    pub zoom: f32,
    /// AI hearing radius for shots (metres).
    pub loudness: f32,
    pub weight: f32,
    pub operable: bool,
}

/// A weapon instance: receiver + attachments + loaded magazine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Weapon {
    pub receiver: ReceiverId,
    pub attachments: [Option<AttachmentId>; 6],
    pub loaded: Option<AmmoType>,
    pub rounds: u32,
    pub auto_mode: bool,
}

impl Weapon {
    /// Receiver with its default attachments and an empty magazine.
    pub fn new(receiver: ReceiverId) -> Self {
        let mut w = Self {
            receiver,
            attachments: [None; 6],
            loaded: None,
            rounds: 0,
            auto_mode: receiver.def().full_auto,
        };
        for a in receiver.def().defaults {
            w.attachments[a.def().slot as usize] = Some(*a);
        }
        w
    }

    /// Bare receiver with no attachments at all.
    pub fn stripped(receiver: ReceiverId) -> Self {
        Self {
            receiver,
            attachments: [None; 6],
            loaded: None,
            rounds: 0,
            auto_mode: receiver.def().full_auto,
        }
    }

    pub fn loaded_with(mut self, ammo: AmmoType, count: u32) -> Self {
        let cap = self.capacity();
        self.loaded = Some(ammo);
        self.rounds = count.min(cap);
        self
    }

    pub fn name(&self) -> &'static str {
        self.receiver.def().name
    }

    pub fn caliber(&self) -> Caliber {
        self.receiver.def().caliber
    }

    pub fn attachment(&self, slot: Slot) -> Option<AttachmentId> {
        self.attachments[slot as usize]
    }

    /// Replace the attachment in a slot, returning the previous one.
    pub fn set_attachment(&mut self, slot: Slot, a: Option<AttachmentId>) -> Option<AttachmentId> {
        let prev = self.attachments[slot as usize];
        self.attachments[slot as usize] = a;
        if slot == Slot::Magazine {
            self.rounds = self.rounds.min(self.capacity());
        }
        prev
    }

    pub fn capacity(&self) -> u32 {
        match self.attachment(Slot::Magazine) {
            Some(m) => m.def().mag_size,
            None => 1,
        }
    }

    pub fn iter_attachments(&self) -> impl Iterator<Item = AttachmentId> + '_ {
        self.attachments.iter().flatten().copied()
    }

    pub fn stats(&self) -> WeaponStats {
        let r = self.receiver.def();
        let mut recoil = 1.0f32;
        let mut ergo = r.ergonomics;
        let mut spread = 1.0f32;
        let mut loud = 1.0f32;
        let mut reload = 1.0f32;
        let mut weight = r.weight;
        let mut zoom = 1.0f32;
        for a in self.iter_attachments() {
            let d = a.def();
            recoil *= 1.0 + d.recoil;
            ergo += d.ergo;
            spread *= 1.0 + d.spread;
            loud *= 1.0 + d.loudness;
            reload *= 1.0 + d.reload;
            weight += d.weight;
            if d.slot == Slot::Sight {
                zoom = d.zoom;
            }
        }
        let ergo = ergo.clamp(0.0, 100.0);
        let operable = !r.slots.contains(&Slot::Barrel) || self.attachment(Slot::Barrel).is_some();
        let spread_deg = r.spread * spread;
        WeaponStats {
            fire_rate: r.fire_rate,
            full_auto: r.full_auto,
            vertical_recoil: r.vertical_recoil * recoil,
            horizontal_recoil: r.horizontal_recoil * recoil,
            ergonomics: ergo,
            spread: spread_deg,
            hip_spread: 1.1 + spread_deg * 2.0 + (1.0 - ergo / 100.0) * 1.2,
            ads_time: 0.14 + 0.32 * (1.0 - ergo / 100.0) + weight * 0.02,
            mag_size: self.capacity(),
            reload_time: r.reload_time * reload * (1.0 + (1.0 - ergo / 100.0) * 0.25),
            zoom,
            loudness: 110.0 * loud,
            weight,
            operable,
        }
    }

    /// Rough trade value of the whole assembled weapon.
    pub fn value(&self) -> u32 {
        self.receiver.def().value + self.iter_attachments().map(|a| a.def().value).sum::<u32>()
    }
}

/// Firing state for a weapon while it is being used in a raid.
#[derive(Clone, Debug, Default)]
pub struct GunState {
    pub cooldown: f32,
    pub reload: Option<ReloadState>,
    /// ADS blend 0..1.
    pub ads: f32,
    pub shot_index: u32,
    pub since_shot: f32,
    /// Accumulated recoil (yaw, pitch) in radians still to be recovered.
    pub recoil_accum: Vec2,
    /// Viewmodel kick amount.
    pub kick: f32,
    pub flash: f32,
    /// Time left before the weapon is ready after switching.
    pub draw: f32,
    pub trigger_prev: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ReloadState {
    pub remaining: f32,
    pub total: f32,
    pub ammo: AmmoType,
}

impl GunState {
    pub fn is_reloading(&self) -> bool {
        self.reload.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_fit_their_receivers() {
        for r in ReceiverId::ALL {
            for a in r.def().defaults {
                assert!(a.fits(r), "{:?} does not fit {:?}", a, r);
                assert!(r.has_slot(a.def().slot));
            }
            let w = Weapon::new(r);
            assert!(w.stats().operable);
            assert!(w.capacity() > 1);
        }
    }

    #[test]
    fn attachments_change_stats() {
        let base = Weapon::new(ReceiverId::Ak74n);
        let s0 = base.stats();
        let mut w = base.clone();
        w.set_attachment(Slot::Muzzle, Some(AttachmentId::Pbs4Suppressor));
        w.set_attachment(Slot::Stock, Some(AttachmentId::ZhukovStock));
        w.set_attachment(Slot::Magazine, Some(AttachmentId::Ak74Mag45));
        w.set_attachment(Slot::Sight, Some(AttachmentId::Pso1Scope));
        let s1 = w.stats();
        assert!(s1.loudness < s0.loudness * 0.5);
        assert!(s1.vertical_recoil < s0.vertical_recoil);
        assert_eq!(s1.mag_size, 45);
        assert_eq!(s1.zoom, 4.0);

        let mut no_stock = base.clone();
        no_stock.set_attachment(Slot::Stock, None);
        assert!(no_stock.stats().vertical_recoil > s0.vertical_recoil * 1.3);

        let mut no_barrel = base;
        no_barrel.set_attachment(Slot::Barrel, None);
        assert!(!no_barrel.stats().operable);
    }
}
