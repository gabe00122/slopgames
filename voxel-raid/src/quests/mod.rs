//! Trader tasks: objectives, progress tracking (in and out of raid) and rewards.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::hideout::{HideoutState, StationKind};
use crate::inventory::{BarterKind, Grid, Item, ItemKind, MedKind, RigKind};
use crate::traders::{TraderId, TradersState};
use crate::weapons::armor::ArmorKind;
use crate::weapons::{AmmoType, AttachmentId, ReceiverId, Slot, Weapon};
use crate::world::ContainerKind;

/// Stat targets for "Gunsmith" hand-over tasks.
#[derive(Clone, Debug, Default)]
pub struct WeaponReq {
    pub min_ergo: Option<f32>,
    pub max_vrecoil: Option<f32>,
    pub min_mag: Option<u32>,
    pub min_zoom: Option<f32>,
    /// Any optic (red dot, holo or scope) fitted.
    pub sight: bool,
    pub suppressed: bool,
}

impl WeaponReq {
    /// Requirement lines, each with pass/fail for the given candidate weapon.
    pub fn lines(&self, w: Option<&Weapon>) -> Vec<(String, Option<bool>)> {
        let s = w.map(|w| w.stats());
        let mut v = Vec::new();
        if let Some(e) = self.min_ergo {
            v.push((format!("Ergonomics at least {e:.0}"), s.map(|s| s.ergonomics >= e)));
        }
        if let Some(r) = self.max_vrecoil {
            v.push((
                format!("Vertical recoil at most {r:.2}°"),
                s.map(|s| s.vertical_recoil <= r + 1e-4),
            ));
        }
        if let Some(m) = self.min_mag {
            v.push((format!("Magazine of at least {m} rounds"), s.map(|s| s.mag_size >= m)));
        }
        if let Some(z) = self.min_zoom {
            v.push((format!("Optic with at least {z:.0}x zoom"), s.map(|s| s.zoom >= z)));
        }
        if self.sight {
            v.push((
                "Any optic fitted".into(),
                w.map(|w| w.attachment(Slot::Sight).is_some()),
            ));
        }
        if self.suppressed {
            v.push(("Suppressed".into(), s.map(|s| s.loudness < 60.0)));
        }
        v.push(("Operable (has a barrel)".into(), s.map(|s| s.operable)));
        v
    }

    pub fn satisfied(&self, w: &Weapon) -> bool {
        self.lines(Some(w)).iter().all(|(_, ok)| ok.unwrap_or(false))
    }
}

#[derive(Clone, Debug)]
pub enum Objective {
    Kill {
        count: u32,
        headshot: bool,
        receiver: Option<ReceiverId>,
    },
    Search {
        kind: ContainerKind,
        count: u32,
    },
    Destroy {
        count: u32,
    },
    Extract {
        count: u32,
        armored: bool,
    },
    HandOver {
        item: ItemKind,
        count: u32,
    },
    HandOverWeapon {
        receiver: ReceiverId,
        req: WeaponReq,
    },
    Station {
        station: StationKind,
        level: u32,
    },
}

impl Objective {
    pub fn target(&self) -> u32 {
        match self {
            Objective::Kill { count, .. }
            | Objective::Search { count, .. }
            | Objective::Destroy { count }
            | Objective::Extract { count, .. }
            | Objective::HandOver { count, .. } => *count,
            Objective::HandOverWeapon { .. } => 1,
            Objective::Station { level, .. } => *level,
        }
    }

    /// Progress happens during raids.
    pub fn is_raid(&self) -> bool {
        matches!(
            self,
            Objective::Kill { .. } | Objective::Search { .. } | Objective::Destroy { .. } | Objective::Extract { .. }
        )
    }

    pub fn describe(&self) -> String {
        match self {
            Objective::Kill {
                count,
                headshot,
                receiver,
            } => {
                let mut s = format!("Kill {count} Scavs");
                if *headshot {
                    s.push_str(" with headshots");
                }
                if let Some(r) = receiver {
                    s.push_str(&format!(" using an {}", r.def().name));
                }
                s
            }
            Objective::Search { kind, count } => format!("Search {count}x {} in raids", kind.name()),
            Objective::Destroy { count } => format!("Destroy {count} blocks with gunfire"),
            Objective::Extract { count, armored } => {
                let mut s = format!("Survive and extract {count} time{}", if *count == 1 { "" } else { "s" });
                if *armored {
                    s.push_str(" wearing a helmet and body armor");
                }
                s
            }
            Objective::HandOver { item, count } => format!("Hand over {count}x {}", item.name()),
            Objective::HandOverWeapon { receiver, .. } => format!("Hand over a modified {}", receiver.def().name),
            Objective::Station { station, level } => format!("Upgrade the {} to level {level}", station.name()),
        }
    }

    fn short(&self) -> &'static str {
        match self {
            Objective::Kill { .. } => "Scavs killed",
            Objective::Search { .. } => "containers searched",
            Objective::Destroy { .. } => "blocks destroyed",
            Objective::Extract { .. } => "extractions",
            _ => "progress",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Reward {
    Roubles(u32),
    Item(ItemKind, u32),
    Standing(TraderId, f32),
}

impl Reward {
    pub fn describe(&self) -> String {
        match self {
            Reward::Roubles(n) => crate::ui::inventory::value_label(*n as u64),
            Reward::Item(k, n) if *n > 1 => format!("{} x{}", k.name(), n),
            Reward::Item(k, _) => k.name().to_string(),
            Reward::Standing(t, s) => format!("+{s:.2} {} standing", t.name()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Quest {
    pub id: &'static str,
    pub trader: TraderId,
    pub name: &'static str,
    pub description: &'static str,
    pub requires: Vec<&'static str>,
    pub loyalty: u32,
    pub objectives: Vec<Objective>,
    pub rewards: Vec<Reward>,
}

fn build() -> Vec<Quest> {
    use Objective as O;
    use Reward as R;
    let q = |id, trader, name, description, requires: Vec<&'static str>, loyalty, objectives, rewards| Quest {
        id,
        trader,
        name,
        description,
        requires,
        loyalty,
        objectives,
        rewards,
    };
    let kill = |count| O::Kill {
        count,
        headshot: false,
        receiver: None,
    };
    let hand = |b: BarterKind, count| O::HandOver {
        item: ItemKind::Barter(b),
        count,
    };
    let p = TraderId::Prapor;
    let t = TraderId::Therapist;
    let m = TraderId::Mechanic;
    let r = TraderId::Ragman;
    vec![
        // ---------------- Prapor ----------------
        q(
            "prapor_debut",
            p,
            "Debut",
            "Prapor wants to see whether you can handle yourself. Thin out the Scavs around town.",
            vec![],
            1,
            vec![kill(5)],
            vec![
                R::Roubles(20_000),
                R::Item(ItemKind::Ammo(AmmoType::Ps545), 60),
                R::Standing(p, 0.12),
            ],
        ),
        q(
            "prapor_search",
            p,
            "Search Mission",
            "Our weapon caches in town were raided. Check the weapon boxes and see what's left.",
            vec!["prapor_debut"],
            1,
            vec![O::Search {
                kind: ContainerKind::WeaponBox,
                count: 2,
            }],
            vec![
                R::Roubles(25_000),
                R::Item(ItemKind::Attachment(AttachmentId::CobraRedDot), 1),
                R::Standing(p, 0.1),
            ],
        ),
        q(
            "prapor_breach",
            p,
            "Breaching Team",
            "Walls don't stop bullets if you bring the right ammo. Punch holes through 30 blocks with gunfire.",
            vec!["prapor_debut"],
            1,
            vec![O::Destroy { count: 30 }],
            vec![
                R::Roubles(30_000),
                R::Item(ItemKind::Ammo(AmmoType::Bs545), 60),
                R::Standing(p, 0.1),
            ],
        ),
        q(
            "prapor_shootout",
            p,
            "Shootout Picnic",
            "Spraying is for amateurs. Put Scavs down with clean headshots. Unlocks 7.62 BP ammo.",
            vec!["prapor_search"],
            2,
            vec![O::Kill {
                count: 4,
                headshot: true,
                receiver: None,
            }],
            vec![R::Roubles(60_000), R::Standing(p, 0.15)],
        ),
        q(
            "prapor_punisher",
            p,
            "Punisher",
            "Twelve Scavs, and only the AK-74N. Prapor likes consistency.",
            vec!["prapor_shootout"],
            2,
            vec![O::Kill {
                count: 12,
                headshot: false,
                receiver: Some(ReceiverId::Ak74n),
            }],
            vec![
                R::Roubles(120_000),
                R::Item(ItemKind::Weapon(ReceiverId::Akm), 1),
                R::Standing(p, 0.2),
            ],
        ),
        // ---------------- Therapist ----------------
        q(
            "therapist_shortage",
            t,
            "Shortage",
            "The clinic's defibrillators are dead. Bring her rechargeable batteries.",
            vec![],
            1,
            vec![hand(BarterKind::Battery, 2)],
            vec![
                R::Roubles(15_000),
                R::Item(ItemKind::Med(MedKind::Ai2), 2),
                R::Standing(t, 0.12),
            ],
        ),
        q(
            "therapist_sanitary",
            t,
            "Sanitary Standards",
            "Check the medcases around town and report on what's left in them.",
            vec!["therapist_shortage"],
            1,
            vec![O::Search {
                kind: ContainerKind::MedCase,
                count: 3,
            }],
            vec![
                R::Roubles(30_000),
                R::Item(ItemKind::Med(MedKind::Salewa), 1),
                R::Standing(t, 0.1),
            ],
        ),
        q(
            "therapist_aquarius",
            t,
            "Operation Aquarius",
            "Anyone can go in. Prove you can come back alive - twice.",
            vec!["therapist_shortage"],
            1,
            vec![O::Extract {
                count: 2,
                armored: false,
            }],
            vec![R::Roubles(40_000), R::Standing(t, 0.12)],
        ),
        q(
            "therapist_privacy",
            t,
            "Health Care Privacy",
            "A special patient expects a special gift: a physical bitcoin and a golden chain.",
            vec!["therapist_sanitary"],
            2,
            vec![hand(BarterKind::Bitcoin, 1), hand(BarterKind::GoldChain, 1)],
            vec![
                R::Roubles(150_000),
                R::Item(ItemKind::Med(MedKind::Surv12), 1),
                R::Standing(t, 0.2),
            ],
        ),
        // ---------------- Mechanic ----------------
        q(
            "mechanic_gunsmith1",
            m,
            "Gunsmith - Part 1",
            "Mechanic needs a PP-19 Vityaz for a client: fit any optic and keep ergonomics at 60 or better.",
            vec![],
            1,
            vec![O::HandOverWeapon {
                receiver: ReceiverId::Vityaz,
                req: WeaponReq {
                    min_ergo: Some(60.0),
                    sight: true,
                    ..Default::default()
                },
            }],
            vec![
                R::Roubles(25_000),
                R::Item(ItemKind::Attachment(AttachmentId::Rk6Grip), 1),
                R::Standing(m, 0.15),
            ],
        ),
        q(
            "mechanic_signal",
            m,
            "Signal - Part 1",
            "The radio relay needs parts: printed circuit boards and a bundle of wires.",
            vec!["mechanic_gunsmith1"],
            1,
            vec![hand(BarterKind::CircuitBoard, 2), hand(BarterKind::Wires, 1)],
            vec![
                R::Roubles(40_000),
                R::Item(ItemKind::Attachment(AttachmentId::Holo1p87), 1),
                R::Standing(m, 0.1),
            ],
        ),
        q(
            "mechanic_workshop",
            m,
            "Workshop Assistant",
            "Get your hideout workbench to level 2 so Mechanic can send you the fiddly jobs.",
            vec!["mechanic_gunsmith1"],
            1,
            vec![O::Station {
                station: StationKind::Workbench,
                level: 2,
            }],
            vec![
                R::Roubles(30_000),
                R::Item(ItemKind::Barter(BarterKind::WeaponParts), 2),
                R::Standing(m, 0.1),
            ],
        ),
        q(
            "mechanic_gunsmith2",
            m,
            "Gunsmith - Part 2",
            "A suppressed AK-74N with a 4x optic, vertical recoil of 0.65° or less and a 45+ round magazine. \
             Unlocks the PBS-4 for cash.",
            vec!["mechanic_gunsmith1"],
            2,
            vec![O::HandOverWeapon {
                receiver: ReceiverId::Ak74n,
                req: WeaponReq {
                    max_vrecoil: Some(0.65),
                    min_mag: Some(45),
                    min_zoom: Some(4.0),
                    suppressed: true,
                    ..Default::default()
                },
            }],
            vec![R::Roubles(90_000), R::Standing(m, 0.2)],
        ),
        // ---------------- Ragman ----------------
        q(
            "ragman_dressed",
            r,
            "Dressed to Kill",
            "Nobody respects a man in a tracksuit. Survive a raid wearing a helmet and body armor.",
            vec![],
            1,
            vec![O::Extract {
                count: 1,
                armored: true,
            }],
            vec![
                R::Roubles(30_000),
                R::Item(ItemKind::Armor(ArmorKind::Zhuk3), 1),
                R::Standing(r, 0.12),
            ],
        ),
        q(
            "ragman_business",
            r,
            "Only Business",
            "Ragman is short on stock. Bring him two Scav vests.",
            vec![],
            1,
            vec![O::HandOver {
                item: ItemKind::Rig(RigKind::ScavVest),
                count: 2,
            }],
            vec![
                R::Roubles(15_000),
                R::Item(ItemKind::Armor(ArmorKind::Kiver), 1),
                R::Standing(r, 0.12),
            ],
        ),
        q(
            "ragman_stockpile",
            r,
            "Stockpile",
            "Winter is coming. Fuel for the generators and tape for the windows.",
            vec!["ragman_business"],
            1,
            vec![hand(BarterKind::FuelCanister, 1), hand(BarterKind::DuctTape, 2)],
            vec![
                R::Roubles(50_000),
                R::Item(ItemKind::Backpack(crate::inventory::BackpackKind::Pilgrim), 1),
                R::Standing(r, 0.15),
            ],
        ),
    ]
}

pub fn all() -> &'static [Quest] {
    static QUESTS: OnceLock<Vec<Quest>> = OnceLock::new();
    QUESTS.get_or_init(build)
}

pub fn get(id: &str) -> Option<&'static Quest> {
    all().iter().find(|q| q.id == id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestStatus {
    Active,
    Completed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct QuestRecord {
    pub id: String,
    pub status: QuestStatus,
    pub progress: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QuestView {
    Locked(String),
    Available,
    Active,
    Ready,
    Completed,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct QuestLog {
    #[serde(default)]
    pub records: Vec<QuestRecord>,
}

fn reward_items(q: &Quest) -> Vec<Item> {
    let mut out = Vec::new();
    for r in &q.rewards {
        match r {
            Reward::Roubles(n) => out.push(Item::stack(ItemKind::Roubles, *n)),
            Reward::Item(ItemKind::Weapon(rec), n) => {
                for _ in 0..*n {
                    out.push(Item::with_weapon(Weapon::new(*rec)));
                }
            }
            Reward::Item(k, n) if k.stackable() => {
                let mut left = *n;
                while left > 0 {
                    let c = left.min(k.max_stack());
                    out.push(Item::stack(*k, c));
                    left -= c;
                }
            }
            Reward::Item(k, n) => {
                for _ in 0..*n {
                    out.push(Item::new(*k));
                }
            }
            Reward::Standing(..) => {}
        }
    }
    out
}

impl QuestLog {
    pub fn record(&self, id: &str) -> Option<&QuestRecord> {
        self.records.iter().find(|r| r.id == id)
    }

    fn record_mut(&mut self, id: &str) -> Option<&mut QuestRecord> {
        self.records.iter_mut().find(|r| r.id == id)
    }

    pub fn is_completed(&self, id: &str) -> bool {
        self.record(id).is_some_and(|r| r.status == QuestStatus::Completed)
    }

    pub fn progress(&self, q: &Quest, i: usize, hideout: &HideoutState) -> u32 {
        let target = q.objectives[i].target();
        match &q.objectives[i] {
            Objective::Station { station, .. } => hideout.level(*station).min(target),
            _ => self
                .record(q.id)
                .and_then(|r| r.progress.get(i).copied())
                .unwrap_or(0)
                .min(target),
        }
    }

    pub fn objectives_done(&self, q: &Quest, hideout: &HideoutState) -> bool {
        (0..q.objectives.len()).all(|i| self.progress(q, i, hideout) >= q.objectives[i].target())
    }

    pub fn view(&self, q: &Quest, traders: &TradersState, hideout: &HideoutState) -> QuestView {
        match self.record(q.id).map(|r| r.status) {
            Some(QuestStatus::Completed) => QuestView::Completed,
            Some(QuestStatus::Active) => {
                if self.objectives_done(q, hideout) {
                    QuestView::Ready
                } else {
                    QuestView::Active
                }
            }
            None => {
                for req in &q.requires {
                    if !self.is_completed(req) {
                        let name = get(req).map(|r| r.name).unwrap_or(req);
                        return QuestView::Locked(format!("Complete \"{name}\" first"));
                    }
                }
                if traders.loyalty(q.trader) < q.loyalty {
                    return QuestView::Locked(format!("Requires {} loyalty level {}", q.trader.name(), q.loyalty));
                }
                QuestView::Available
            }
        }
    }

    pub fn accept(&mut self, q: &Quest) {
        if self.record(q.id).is_none() {
            self.records.push(QuestRecord {
                id: q.id.to_string(),
                status: QuestStatus::Active,
                progress: vec![0; q.objectives.len()],
            });
        }
    }

    /// Hand over items for objective `i` from the stash.
    pub fn hand_over(&mut self, q: &Quest, i: usize, stash: &mut Grid) -> Result<String, String> {
        let current = self.record(q.id).map(|r| r.progress.get(i).copied().unwrap_or(0));
        let Some(current) = current else {
            return Err("Accept the task first".into());
        };
        let given = match &q.objectives[i] {
            Objective::HandOver { item, count } => {
                let need = count.saturating_sub(current);
                let take = stash.count_kind(*item).min(need);
                if take == 0 {
                    return Err(format!("No {} in your stash", item.name()));
                }
                stash.take_kind(*item, take);
                take
            }
            Objective::HandOverWeapon { receiver, req } => {
                if current >= 1 {
                    return Err("Already handed over".into());
                }
                let uid = stash
                    .items
                    .iter()
                    .find(|p| {
                        p.item
                            .weapon
                            .as_ref()
                            .is_some_and(|w| w.receiver == *receiver && req.satisfied(w))
                    })
                    .map(|p| p.item.uid)
                    .ok_or_else(|| format!("No {} in your stash meets the requirements", receiver.def().name))?;
                stash.remove(uid);
                1
            }
            _ => return Err("This objective is completed in raids".into()),
        };
        if let Some(r) = self.record_mut(q.id) {
            if r.progress.len() < q.objectives.len() {
                r.progress.resize(q.objectives.len(), 0);
            }
            r.progress[i] += given;
        }
        Ok(format!("Handed over {given}"))
    }

    /// Complete a finished task and receive its rewards (into the stash).
    pub fn turn_in(
        &mut self,
        q: &Quest,
        stash: &mut Grid,
        traders: &mut TradersState,
        hideout: &HideoutState,
    ) -> Result<(), String> {
        if self.view(q, traders, hideout) != QuestView::Ready {
            return Err("Objectives are not complete".into());
        }
        let mut trial = stash.clone();
        for it in reward_items(q) {
            trial
                .insert(it)
                .map_err(|_| "Not enough space in your stash for the rewards".to_string())?;
        }
        *stash = trial;
        for r in &q.rewards {
            if let Reward::Standing(t, s) = r {
                traders.standing_mut(*t).standing += s;
            }
        }
        if let Some(r) = self.record_mut(q.id) {
            r.status = QuestStatus::Completed;
        }
        Ok(())
    }

    /// Snapshot of raid objectives for the tracker.
    pub fn tracker(&self) -> QuestTracker {
        let mut items = Vec::new();
        for r in &self.records {
            if r.status != QuestStatus::Active {
                continue;
            }
            let Some(q) = get(&r.id) else { continue };
            for (i, o) in q.objectives.iter().enumerate() {
                let p = r.progress.get(i).copied().unwrap_or(0);
                if o.is_raid() && p < o.target() {
                    items.push(TrackedObjective {
                        quest: q.id,
                        index: i,
                        progress: p,
                        start: p,
                    });
                }
            }
        }
        QuestTracker { items }
    }

    /// Write raid progress back.
    pub fn merge(&mut self, tracker: &QuestTracker) {
        for t in &tracker.items {
            if let Some(r) = self.record_mut(t.quest) {
                if r.progress.len() <= t.index {
                    r.progress.resize(t.index + 1, 0);
                }
                r.progress[t.index] = r.progress[t.index].max(t.progress);
            }
        }
    }

    /// Active tasks with a one-line status each (for menus).
    pub fn active_lines(&self, hideout: &HideoutState) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for r in &self.records {
            if r.status != QuestStatus::Active {
                continue;
            }
            let Some(q) = get(&r.id) else { continue };
            let status = if self.objectives_done(q, hideout) {
                format!("ready to turn in to {}", q.trader.name())
            } else {
                q.objectives
                    .iter()
                    .enumerate()
                    .filter(|(i, o)| self.progress(q, *i, hideout) < o.target())
                    .map(|(i, o)| format!("{} ({}/{})", o.describe(), self.progress(q, i, hideout), o.target()))
                    .next()
                    .unwrap_or_default()
            };
            out.push((q.name.to_string(), status));
        }
        out
    }
}

/// Things that happen in a raid which can advance tasks.
#[derive(Clone, Copy, Debug)]
pub enum QuestEvent {
    Kill {
        headshot: bool,
        receiver: Option<ReceiverId>,
    },
    Search(ContainerKind),
    Destroyed,
    Extracted {
        armored: bool,
    },
}

#[derive(Clone, Debug)]
pub struct TrackedObjective {
    pub quest: &'static str,
    pub index: usize,
    pub progress: u32,
    pub start: u32,
}

/// Live copy of the active raid objectives.
#[derive(Clone, Debug, Default)]
pub struct QuestTracker {
    pub items: Vec<TrackedObjective>,
}

impl QuestTracker {
    /// Apply an event; returns HUD messages for any progress made.
    pub fn apply(&mut self, ev: QuestEvent) -> Vec<String> {
        let mut msgs = Vec::new();
        for t in &mut self.items {
            let Some(q) = get(t.quest) else { continue };
            let o = &q.objectives[t.index];
            let target = o.target();
            if t.progress >= target {
                continue;
            }
            let hit = match (o, ev) {
                (
                    Objective::Kill { headshot, receiver, .. },
                    QuestEvent::Kill {
                        headshot: h,
                        receiver: r,
                    },
                ) => (!headshot || h) && (receiver.is_none() || *receiver == r),
                (Objective::Search { kind, .. }, QuestEvent::Search(k)) => *kind == k,
                (Objective::Destroy { .. }, QuestEvent::Destroyed) => true,
                (Objective::Extract { armored, .. }, QuestEvent::Extracted { armored: a }) => !armored || a,
                _ => false,
            };
            if hit {
                t.progress += 1;
                let noisy = matches!(o, Objective::Destroy { .. });
                if !noisy || t.progress % 5 == 0 || t.progress == target {
                    let done = if t.progress == target { " - complete!" } else { "" };
                    msgs.push(format!(
                        "Task \"{}\": {} {}/{}{}",
                        q.name,
                        o.short(),
                        t.progress,
                        target,
                        done
                    ));
                }
            }
        }
        msgs
    }

    /// Current state lines for the HUD.
    pub fn lines(&self) -> Vec<String> {
        self.items
            .iter()
            .filter_map(|t| {
                let q = get(t.quest)?;
                let o = &q.objectives[t.index];
                Some(format!(
                    "{}: {} {}/{}",
                    q.name,
                    o.short(),
                    t.progress.min(o.target()),
                    o.target()
                ))
            })
            .collect()
    }

    /// Progress made during this raid.
    pub fn summary(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|t| t.progress > t.start)
            .filter_map(|t| {
                let q = get(t.quest)?;
                let o = &q.objectives[t.index];
                Some(format!(
                    "{}: {} {}/{} (+{})",
                    q.name,
                    o.short(),
                    t.progress.min(o.target()),
                    o.target(),
                    t.progress - t.start
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_ids_are_unique_and_prerequisites_exist() {
        let mut seen = std::collections::HashSet::new();
        for q in all() {
            assert!(seen.insert(q.id), "duplicate {}", q.id);
            for r in &q.requires {
                assert!(get(r).is_some(), "{} requires unknown {}", q.id, r);
            }
            assert!(!q.objectives.is_empty() && !q.rewards.is_empty());
        }
        // Offers unlocked by quests reference real quests.
        for t in TraderId::ALL {
            for o in crate::traders::offers(t, 0) {
                if let Some(id) = o.unlocked_by {
                    assert!(get(id).is_some(), "offer unlocked by unknown quest {id}");
                }
            }
        }
    }

    #[test]
    fn kill_task_progresses_in_raid_and_pays_out() {
        let mut log = QuestLog::default();
        let traders_state = TradersState::default();
        let hideout = HideoutState::default();
        let q = get("prapor_debut").unwrap();
        assert_eq!(log.view(q, &traders_state, &hideout), QuestView::Available);
        log.accept(q);
        let mut tr = log.tracker();
        assert_eq!(tr.items.len(), 1);
        for _ in 0..3 {
            tr.apply(QuestEvent::Kill {
                headshot: false,
                receiver: None,
            });
        }
        log.merge(&tr);
        assert_eq!(log.progress(q, 0, &hideout), 3);
        assert_eq!(log.view(q, &traders_state, &hideout), QuestView::Active);
        let mut tr = log.tracker();
        let msgs: Vec<String> = (0..4)
            .flat_map(|_| {
                tr.apply(QuestEvent::Kill {
                    headshot: true,
                    receiver: Some(ReceiverId::Grach),
                })
            })
            .collect();
        assert_eq!(msgs.len(), 2, "only progress up to the target is reported");
        log.merge(&tr);
        assert_eq!(log.view(q, &traders_state, &hideout), QuestView::Ready);

        let mut traders_state = traders_state;
        let mut stash = Grid::new(10, 10);
        log.turn_in(q, &mut stash, &mut traders_state, &hideout).unwrap();
        assert!(log.is_completed("prapor_debut"));
        assert_eq!(stash.count_kind(ItemKind::Roubles), 20_000);
        assert_eq!(stash.count_kind(ItemKind::Ammo(AmmoType::Ps545)), 60);
        assert!((traders_state.standing(TraderId::Prapor).standing - 0.12).abs() < 1e-5);
        // Follow-ups unlock.
        let next = get("prapor_search").unwrap();
        assert_eq!(log.view(next, &traders_state, &hideout), QuestView::Available);
        let shootout = get("prapor_shootout").unwrap();
        assert!(matches!(
            log.view(shootout, &traders_state, &hideout),
            QuestView::Locked(_)
        ));
    }

    #[test]
    fn headshot_and_weapon_filters() {
        let mut tr = QuestTracker {
            items: vec![TrackedObjective {
                quest: "prapor_shootout",
                index: 0,
                progress: 0,
                start: 0,
            }],
        };
        tr.apply(QuestEvent::Kill {
            headshot: false,
            receiver: None,
        });
        assert_eq!(tr.items[0].progress, 0);
        tr.apply(QuestEvent::Kill {
            headshot: true,
            receiver: None,
        });
        assert_eq!(tr.items[0].progress, 1);

        let mut tr = QuestTracker {
            items: vec![TrackedObjective {
                quest: "prapor_punisher",
                index: 0,
                progress: 0,
                start: 0,
            }],
        };
        tr.apply(QuestEvent::Kill {
            headshot: true,
            receiver: Some(ReceiverId::Akm),
        });
        tr.apply(QuestEvent::Kill {
            headshot: false,
            receiver: Some(ReceiverId::Ak74n),
        });
        assert_eq!(tr.items[0].progress, 1);
    }

    #[test]
    fn gunsmith_hand_over_checks_stats() {
        let mut log = QuestLog::default();
        let q = get("mechanic_gunsmith1").unwrap();
        log.accept(q);
        let mut stash = Grid::new(10, 10);
        // Stock Vityaz has no optic: rejected.
        stash
            .insert(Item::with_weapon(Weapon::new(ReceiverId::Vityaz)))
            .unwrap();
        assert!(log.hand_over(q, 0, &mut stash).is_err());
        // With a red dot it qualifies.
        let mut w = Weapon::new(ReceiverId::Vityaz);
        w.set_attachment(Slot::Sight, Some(AttachmentId::CobraRedDot));
        assert!(q.objectives.iter().all(|o| match o {
            Objective::HandOverWeapon { req, .. } => req.satisfied(&w),
            _ => true,
        }));
        stash.insert(Item::with_weapon(w)).unwrap();
        log.hand_over(q, 0, &mut stash).unwrap();
        assert_eq!(
            stash.count_kind(ItemKind::Weapon(ReceiverId::Vityaz)),
            1,
            "only the modded one is taken"
        );
        assert!(log.objectives_done(q, &HideoutState::default()));
    }

    #[test]
    fn gunsmith_two_is_achievable() {
        let q = get("mechanic_gunsmith2").unwrap();
        let Objective::HandOverWeapon { req, .. } = &q.objectives[0] else {
            panic!("expected weapon objective")
        };
        let mut w = Weapon::new(ReceiverId::Ak74n);
        w.set_attachment(Slot::Muzzle, Some(AttachmentId::Pbs4Suppressor));
        w.set_attachment(Slot::Sight, Some(AttachmentId::Pso1Scope));
        w.set_attachment(Slot::Magazine, Some(AttachmentId::Ak74Mag45));
        assert!(!req.satisfied(&w), "recoil still too high without a grip/stock upgrade");
        w.set_attachment(Slot::Grip, Some(AttachmentId::Rk2Grip));
        assert!(req.satisfied(&w));
    }

    #[test]
    fn station_objective_reads_hideout_level() {
        let mut log = QuestLog::default();
        let q = get("mechanic_workshop").unwrap();
        log.accept(q);
        let mut h = HideoutState::default();
        assert!(!log.objectives_done(q, &h));
        h.set_level(StationKind::Workbench, 2);
        assert!(log.objectives_done(q, &h));
    }
}
