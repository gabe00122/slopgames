//! Body-part health shared by the player and AI.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyPart {
    Head,
    Thorax,
    Stomach,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

impl BodyPart {
    pub const ALL: [BodyPart; 7] = [
        BodyPart::Head,
        BodyPart::Thorax,
        BodyPart::Stomach,
        BodyPart::LeftArm,
        BodyPart::RightArm,
        BodyPart::LeftLeg,
        BodyPart::RightLeg,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BodyPart::Head => "Head",
            BodyPart::Thorax => "Thorax",
            BodyPart::Stomach => "Stomach",
            BodyPart::LeftArm => "Left arm",
            BodyPart::RightArm => "Right arm",
            BodyPart::LeftLeg => "Left leg",
            BodyPart::RightLeg => "Right leg",
        }
    }

    pub fn max_hp(self) -> f32 {
        match self {
            BodyPart::Head => 35.0,
            BodyPart::Thorax => 85.0,
            BodyPart::Stomach => 70.0,
            BodyPart::LeftArm | BodyPart::RightArm => 60.0,
            BodyPart::LeftLeg | BodyPart::RightLeg => 65.0,
        }
    }

    /// Vital parts kill when destroyed.
    pub fn is_vital(self) -> bool {
        matches!(self, BodyPart::Head | BodyPart::Thorax)
    }

    /// Damage multiplier applied on top of the bullet damage (Tarkov uses 1.0
    /// everywhere; we slightly soften limbs to keep fights readable).
    pub fn damage_mult(self) -> f32 {
        match self {
            BodyPart::LeftArm | BodyPart::RightArm => 0.8,
            BodyPart::LeftLeg | BodyPart::RightLeg => 0.85,
            _ => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DamageOutcome {
    pub killed: bool,
    pub part_destroyed: bool,
    pub dealt: f32,
}

#[derive(Clone, Debug)]
pub struct Body {
    pub hp: [f32; 7],
}

impl Default for Body {
    fn default() -> Self {
        Self::new()
    }
}

impl Body {
    pub fn new() -> Self {
        let mut hp = [0.0; 7];
        for p in BodyPart::ALL {
            hp[p as usize] = p.max_hp();
        }
        Self { hp }
    }

    pub fn part_hp(&self, p: BodyPart) -> f32 {
        self.hp[p as usize]
    }

    pub fn is_destroyed(&self, p: BodyPart) -> bool {
        self.hp[p as usize] <= 0.0
    }

    pub fn is_dead(&self) -> bool {
        self.hp[BodyPart::Head as usize] <= 0.0 || self.hp[BodyPart::Thorax as usize] <= 0.0
    }

    pub fn total(&self) -> f32 {
        self.hp.iter().sum()
    }

    pub fn max_total(&self) -> f32 {
        BodyPart::ALL.iter().map(|p| p.max_hp()).sum()
    }

    /// Apply damage to a body part. Damage to an already destroyed limb spreads
    /// (reduced) across all remaining parts, like in Tarkov.
    pub fn damage(&mut self, part: BodyPart, amount: f32) -> DamageOutcome {
        let mut out = DamageOutcome::default();
        if self.is_dead() || amount <= 0.0 {
            return out;
        }
        let i = part as usize;
        if self.hp[i] <= 0.0 {
            let alive: Vec<usize> = (0..7).filter(|j| self.hp[*j] > 0.0).collect();
            if !alive.is_empty() {
                let share = amount * 0.7 / alive.len() as f32;
                for j in alive {
                    self.hp[j] = (self.hp[j] - share).max(0.0);
                }
            }
            out.dealt = amount * 0.7;
        } else {
            let before = self.hp[i];
            self.hp[i] = (self.hp[i] - amount).max(0.0);
            out.dealt = before - self.hp[i];
            if self.hp[i] <= 0.0 {
                out.part_destroyed = true;
            }
        }
        out.killed = self.is_dead();
        out
    }

    /// Heal up to `amount` HP across damaged (not destroyed) parts, most
    /// injured first. Returns the amount actually healed.
    pub fn heal(&mut self, amount: f32) -> f32 {
        let mut left = amount;
        while left > 0.01 {
            let mut best: Option<(usize, f32)> = None;
            for p in BodyPart::ALL {
                let i = p as usize;
                let frac = self.hp[i] / p.max_hp();
                if self.hp[i] > 0.0 && frac < 0.999 && best.is_none_or(|(_, f)| frac < f) {
                    best = Some((i, frac));
                }
            }
            let Some((i, _)) = best else { break };
            let max = BodyPart::ALL[i].max_hp();
            let add = left.min(max - self.hp[i]).min(10.0);
            self.hp[i] += add;
            left -= add;
        }
        amount - left
    }

    /// Restore one destroyed (non-vital) part to a small amount of HP.
    pub fn repair_destroyed(&mut self) -> Option<BodyPart> {
        for p in BodyPart::ALL {
            if !p.is_vital() && self.hp[p as usize] <= 0.0 {
                self.hp[p as usize] = p.max_hp() * 0.3;
                return Some(p);
            }
        }
        None
    }

    pub fn is_injured(&self) -> bool {
        BodyPart::ALL
            .iter()
            .any(|p| self.hp[*p as usize] > 0.0 && self.hp[*p as usize] < p.max_hp())
    }

    pub fn has_destroyed_limb(&self) -> bool {
        BodyPart::ALL
            .iter()
            .any(|p| !p.is_vital() && self.hp[*p as usize] <= 0.0)
    }

    /// Movement speed multiplier from leg injuries.
    pub fn move_mult(&self) -> f32 {
        let mut m = 1.0;
        for p in [BodyPart::LeftLeg, BodyPart::RightLeg] {
            if self.is_destroyed(p) {
                m -= 0.3;
            }
        }
        m
    }

    /// Weapon sway multiplier from arm injuries.
    pub fn sway_mult(&self) -> f32 {
        let mut m = 1.0;
        for p in [BodyPart::LeftArm, BodyPart::RightArm] {
            if self.is_destroyed(p) {
                m += 1.0;
            }
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headshot_kills() {
        let mut b = Body::new();
        let o = b.damage(BodyPart::Head, 40.0);
        assert!(o.killed);
    }

    #[test]
    fn destroyed_limb_spreads_damage() {
        let mut b = Body::new();
        b.damage(BodyPart::LeftArm, 100.0);
        assert!(b.is_destroyed(BodyPart::LeftArm));
        let before = b.part_hp(BodyPart::Thorax);
        b.damage(BodyPart::LeftArm, 60.0);
        assert!(b.part_hp(BodyPart::Thorax) < before);
        assert!(!b.is_dead());
    }

    #[test]
    fn heal_restores() {
        let mut b = Body::new();
        b.damage(BodyPart::Stomach, 30.0);
        b.damage(BodyPart::RightLeg, 20.0);
        let healed = b.heal(100.0);
        assert!((healed - 50.0).abs() < 0.01);
        assert!(!b.is_injured());
    }
}
