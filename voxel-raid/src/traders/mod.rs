//! Traders: loyalty levels, catalogues (cash and barter offers), buying and selling.

use serde::{Deserialize, Serialize};

use crate::inventory::{BackpackKind, BarterKind, Category, Grid, Item, ItemKind, MedKind, RigKind};
use crate::rng::Rng;
use crate::weapons::armor::ArmorKind;
use crate::weapons::{AmmoType, AttachmentId, ReceiverId, Weapon};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TraderId {
    Prapor,
    Therapist,
    Mechanic,
    Ragman,
    Fence,
}

pub const MAX_LOYALTY: u32 = 3;

impl TraderId {
    pub const ALL: [TraderId; 5] = [
        TraderId::Prapor,
        TraderId::Therapist,
        TraderId::Mechanic,
        TraderId::Ragman,
        TraderId::Fence,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TraderId::Prapor => "Prapor",
            TraderId::Therapist => "Therapist",
            TraderId::Mechanic => "Mechanic",
            TraderId::Ragman => "Ragman",
            TraderId::Fence => "Fence",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            TraderId::Prapor => "Warrant officer of the old garrison. Weapons, ammunition and magazines.",
            TraderId::Therapist => "Former head of trauma care. Medicine, and she pays well for valuables.",
            TraderId::Mechanic => "A gunsmith who lives at the bench. Attachments and weapon parts.",
            TraderId::Ragman => "Sells clothes, armor and anything that holds your loot.",
            TraderId::Fence => "Buys anything, no questions asked. Stock rotates after every raid.",
        }
    }

    /// Portrait colour (sRGB).
    pub fn color(self) -> [u8; 3] {
        match self {
            TraderId::Prapor => [120, 110, 70],
            TraderId::Therapist => [150, 70, 70],
            TraderId::Mechanic => [70, 100, 130],
            TraderId::Ragman => [100, 80, 120],
            TraderId::Fence => [80, 80, 80],
        }
    }

    pub fn buys(self, c: Category) -> bool {
        match self {
            TraderId::Prapor => matches!(c, Category::Weapon | Category::Ammo | Category::Mod),
            TraderId::Therapist => matches!(c, Category::Med | Category::Barter | Category::Valuable),
            TraderId::Mechanic => matches!(c, Category::Mod | Category::Weapon | Category::Barter),
            TraderId::Ragman => matches!(c, Category::Gear | Category::Container),
            TraderId::Fence => c != Category::Money,
        }
    }

    pub fn buys_label(self) -> &'static str {
        match self {
            TraderId::Prapor => "weapons, ammo, attachments",
            TraderId::Therapist => "meds, barter goods, valuables",
            TraderId::Mechanic => "attachments, weapons, barter goods",
            TraderId::Ragman => "armor, helmets, rigs, backpacks",
            TraderId::Fence => "anything (at a poor price)",
        }
    }

    /// Fraction of an item's value paid when buying it from the player.
    pub fn buy_rate(self) -> f32 {
        match self {
            TraderId::Fence => 0.4,
            TraderId::Therapist | TraderId::Mechanic => 0.6,
            _ => 0.55,
        }
    }

    pub fn max_loyalty(self) -> u32 {
        if self == TraderId::Fence {
            1
        } else {
            MAX_LOYALTY
        }
    }
}

/// Requirements (standing, trade volume in roubles) to reach a loyalty level.
pub fn loyalty_requirement(level: u32) -> (f32, u64) {
    match level {
        0 | 1 => (0.0, 0),
        2 => (0.2, 150_000),
        _ => (0.45, 500_000),
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TraderStanding {
    pub standing: f32,
    /// Roubles traded with this trader (purchases + sales).
    pub volume: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TradersState {
    #[serde(default)]
    pub standings: Vec<(TraderId, TraderStanding)>,
    /// Units bought from limited offers since the last restock.
    #[serde(default)]
    pub purchased: Vec<(String, u32)>,
}

impl TradersState {
    pub fn standing(&self, t: TraderId) -> TraderStanding {
        self.standings
            .iter()
            .find(|(k, _)| *k == t)
            .map(|(_, s)| *s)
            .unwrap_or_default()
    }

    pub fn standing_mut(&mut self, t: TraderId) -> &mut TraderStanding {
        if let Some(i) = self.standings.iter().position(|(k, _)| *k == t) {
            &mut self.standings[i].1
        } else {
            self.standings.push((t, TraderStanding::default()));
            &mut self.standings.last_mut().expect("just pushed").1
        }
    }

    pub fn loyalty(&self, t: TraderId) -> u32 {
        let s = self.standing(t);
        let mut level = 1;
        for l in 2..=t.max_loyalty() {
            let (need_standing, need_volume) = loyalty_requirement(l);
            if s.standing + 1e-4 >= need_standing && s.volume >= need_volume {
                level = l;
            } else {
                break;
            }
        }
        level
    }

    pub fn purchased(&self, key: &str) -> u32 {
        self.purchased
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, n)| *n)
            .unwrap_or(0)
    }

    fn record_purchase(&mut self, key: &str) {
        if let Some(e) = self.purchased.iter_mut().find(|(k, _)| k == key) {
            e.1 += 1;
        } else {
            self.purchased.push((key.to_string(), 1));
        }
    }

    /// Limited stock refills (called after every raid).
    pub fn restock(&mut self) {
        self.purchased.clear();
    }
}

#[derive(Clone, Debug)]
pub struct Offer {
    pub trader: TraderId,
    pub key: String,
    /// The exact item handed over (weapons with parts, used armor, stacks...).
    pub item: Item,
    pub price: Vec<(ItemKind, u32)>,
    pub loyalty: u32,
    /// Purchases allowed per restock.
    pub stock: Option<u32>,
    /// Quest that must be completed before this offer appears.
    pub unlocked_by: Option<&'static str>,
}

impl Offer {
    pub fn label(&self) -> String {
        if self.item.count > 1 {
            format!("{} x{}", self.item.name(), self.item.count)
        } else {
            self.item.name().to_string()
        }
    }

    /// Price expressed in roubles (barter items at their value), for loyalty volume.
    pub fn price_value(&self) -> u64 {
        self.price.iter().map(|(k, n)| k.unit_value() as u64 * *n as u64).sum()
    }

    pub fn is_barter(&self) -> bool {
        self.price.iter().any(|(k, _)| *k != ItemKind::Roubles)
    }

    fn items(&self) -> Vec<Item> {
        let mut out = Vec::new();
        let kind = self.item.kind;
        if kind.stackable() {
            let mut left = self.item.count;
            while left > 0 {
                let n = left.min(kind.max_stack());
                out.push(Item::stack(kind, n));
                left -= n;
            }
        } else {
            let mut it = self.item.clone();
            it.uid = crate::inventory::new_uid();
            out.push(it);
        }
        out
    }
}

struct OfferSpec {
    item: Item,
    price: Vec<(ItemKind, u32)>,
    loyalty: u32,
    stock: Option<u32>,
    unlocked_by: Option<&'static str>,
}

fn cash(item: Item, rub: u32, loyalty: u32) -> OfferSpec {
    OfferSpec {
        item,
        price: vec![(ItemKind::Roubles, rub)],
        loyalty,
        stock: None,
        unlocked_by: None,
    }
}

fn gun(r: ReceiverId) -> Item {
    Item::with_weapon(Weapon::new(r))
}

fn ammo(a: AmmoType, n: u32) -> Item {
    Item::stack(ItemKind::Ammo(a), n)
}

fn att(a: AttachmentId) -> Item {
    Item::new(ItemKind::Attachment(a))
}

fn item(k: ItemKind) -> Item {
    Item::new(k)
}

fn barter(k: BarterKind, n: u32) -> (ItemKind, u32) {
    (ItemKind::Barter(k), n)
}

fn limited(mut o: OfferSpec, stock: u32) -> OfferSpec {
    o.stock = Some(stock);
    o
}

fn unlocked(mut o: OfferSpec, quest: &'static str) -> OfferSpec {
    o.unlocked_by = Some(quest);
    o
}

fn specs(t: TraderId) -> Vec<OfferSpec> {
    use AttachmentId as A;
    use BarterKind as B;
    match t {
        TraderId::Prapor => vec![
            cash(gun(ReceiverId::Grach), 18_000, 1),
            cash(ammo(AmmoType::Pst9, 60), 6_500, 1),
            cash(ammo(AmmoType::Ps545, 60), 8_500, 1),
            cash(ammo(AmmoType::Ps762, 60), 9_500, 1),
            cash(ammo(AmmoType::Rip9, 60), 9_000, 1),
            cash(ammo(AmmoType::Hp545, 60), 9_500, 1),
            cash(att(A::GrachMag17), 2_000, 1),
            cash(att(A::Pp19Mag30), 3_500, 1),
            cash(att(A::Ak74Mag30), 3_800, 1),
            cash(att(A::AkmMag30), 3_800, 1),
            cash(gun(ReceiverId::Vityaz), 38_000, 2),
            cash(gun(ReceiverId::Ak74n), 58_000, 2),
            cash(gun(ReceiverId::Akm), 52_000, 2),
            cash(ammo(AmmoType::Hp762, 60), 11_000, 2),
            limited(cash(ammo(AmmoType::Ap9, 60), 24_000, 2), 3),
            limited(cash(ammo(AmmoType::Bs545, 60), 30_000, 2), 2),
            limited(
                unlocked(cash(ammo(AmmoType::Bp762, 60), 34_000, 2), "prapor_shootout"),
                2,
            ),
            cash(att(A::Ak74Mag45), 12_000, 3),
            cash(att(A::AkmMag40), 11_000, 3),
            limited(cash(ammo(AmmoType::Bs545, 60), 28_000, 3), 4),
            OfferSpec {
                item: att(A::AkmDrum75),
                price: vec![barter(B::WeaponParts, 2), barter(B::GunpowderEagle, 1)],
                loyalty: 3,
                stock: Some(1),
                unlocked_by: None,
            },
        ],
        TraderId::Therapist => vec![
            cash(item(ItemKind::Med(MedKind::Ai2)), 9_500, 1),
            cash(item(ItemKind::Barter(B::DuctTape)), 17_000, 1),
            cash(Item::stack(ItemKind::Barter(B::Matches), 2), 7_000, 1),
            cash(item(ItemKind::Med(MedKind::Salewa)), 27_000, 2),
            limited(cash(item(ItemKind::Med(MedKind::Surv12)), 58_000, 2), 1),
            OfferSpec {
                item: item(ItemKind::Med(MedKind::Surv12)),
                price: vec![barter(B::GoldChain, 1), barter(B::DuctTape, 1)],
                loyalty: 2,
                stock: Some(1),
                unlocked_by: None,
            },
            cash(item(ItemKind::Barter(B::Toolset)), 52_000, 3),
            OfferSpec {
                item: item(ItemKind::Barter(B::FuelCanister)),
                price: vec![barter(B::Battery, 2)],
                loyalty: 3,
                stock: Some(2),
                unlocked_by: None,
            },
        ],
        TraderId::Mechanic => vec![
            cash(att(A::Rk2Grip), 5_500, 1),
            cash(att(A::CobraRedDot), 11_500, 1),
            cash(att(A::Rmr), 10_000, 1),
            cash(att(A::Ak74Brake), 3_200, 1),
            cash(att(A::AkmSlantBrake), 3_200, 1),
            cash(att(A::Pp19CqbStock), 13_500, 1),
            cash(att(A::Ak74PolymerStock), 5_000, 1),
            cash(Item::stack(ItemKind::Barter(B::Screws), 2), 14_000, 1),
            cash(Item::stack(ItemKind::Barter(B::Bolts), 2), 20_000, 1),
            cash(att(A::ZhukovStock), 19_000, 2),
            cash(att(A::AfgGrip), 10_000, 2),
            cash(att(A::Rk6Grip), 7_500, 2),
            cash(att(A::Dtk1Compensator), 15_000, 2),
            cash(att(A::Suppressor9mm), 19_500, 2),
            cash(att(A::Holo1p87), 17_500, 2),
            limited(cash(item(ItemKind::Barter(B::WeaponParts)), 32_000, 2), 2),
            unlocked(cash(att(A::Pbs4Suppressor), 29_000, 2), "mechanic_gunsmith2"),
            OfferSpec {
                item: att(A::Pbs4Suppressor),
                price: vec![barter(B::CircuitBoard, 2)],
                loyalty: 2,
                stock: Some(1),
                unlocked_by: None,
            },
            cash(att(A::Pso1Scope), 27_000, 3),
            cash(att(A::Pbs1Suppressor), 29_000, 3),
            cash(att(A::AkmBarrel520), 18_000, 3),
            limited(cash(att(A::Rpk16Drum95), 39_000, 3), 1),
            OfferSpec {
                item: att(A::Pso1Scope),
                price: vec![barter(B::Bitcoin, 1)],
                loyalty: 3,
                stock: Some(1),
                unlocked_by: None,
            },
        ],
        TraderId::Ragman => vec![
            cash(item(ItemKind::Rig(RigKind::ScavVest)), 10_500, 1),
            cash(item(ItemKind::Backpack(BackpackKind::Sling)), 11_500, 1),
            cash(item(ItemKind::Armor(ArmorKind::Ssh68)), 5_500, 1),
            cash(item(ItemKind::Armor(ArmorKind::Paca)), 16_000, 1),
            cash(item(ItemKind::Backpack(BackpackKind::ScavBackpack)), 19_500, 1),
            cash(item(ItemKind::Armor(ArmorKind::Kiver)), 12_500, 2),
            cash(item(ItemKind::Armor(ArmorKind::Zhuk3)), 29_000, 2),
            cash(item(ItemKind::Rig(RigKind::ChestRig)), 30_000, 2),
            cash(item(ItemKind::Armor(ArmorKind::Beanie6b47Lite)), 23_000, 2),
            cash(item(ItemKind::Armor(ArmorKind::Kora)), 60_000, 3),
            limited(cash(item(ItemKind::Armor(ArmorKind::Gzhel)), 118_000, 3), 1),
            cash(item(ItemKind::Armor(ArmorKind::Ulach)), 56_000, 3),
            cash(item(ItemKind::Backpack(BackpackKind::Pilgrim)), 46_000, 3),
        ],
        TraderId::Fence => Vec::new(),
    }
}

/// Everything a trader has on the shelf. Fence's assortment depends on `fence_seed`
/// (it changes after every raid).
pub fn offers(t: TraderId, fence_seed: u64) -> Vec<Offer> {
    if t == TraderId::Fence {
        let mut rng = Rng::new(fence_seed ^ 0xFE4C_E000);
        return (0..10)
            .map(|i| {
                let it = crate::inventory::loot::random_trade_item(&mut rng);
                let price = ((it.value() as f32 * 1.35 / 100.0).ceil() * 100.0) as u32;
                Offer {
                    trader: t,
                    key: format!("fence:{fence_seed}:{i}"),
                    item: it,
                    price: vec![(ItemKind::Roubles, price.max(500))],
                    loyalty: 1,
                    stock: Some(1),
                    unlocked_by: None,
                }
            })
            .collect();
    }
    specs(t)
        .into_iter()
        .enumerate()
        .map(|(i, s)| Offer {
            trader: t,
            key: format!("{}:{}", t.name(), i),
            item: s.item,
            price: s.price,
            loyalty: s.loyalty,
            stock: s.stock,
            unlocked_by: s.unlocked_by,
        })
        .collect()
}

pub fn can_afford(stash: &Grid, price: &[(ItemKind, u32)]) -> bool {
    price.iter().all(|(k, n)| stash.count_kind(*k) >= *n)
}

/// Buy an offer, paying from the stash and delivering into it.
/// `unlocked` says whether the offer's quest requirement is met.
pub fn buy(state: &mut TradersState, stash: &mut Grid, offer: &Offer, unlocked: bool) -> Result<(), String> {
    if !unlocked {
        return Err("Complete the trader's task to unlock this offer".into());
    }
    if state.loyalty(offer.trader) < offer.loyalty {
        return Err(format!("Requires loyalty level {}", offer.loyalty));
    }
    if let Some(stock) = offer.stock {
        if state.purchased(&offer.key) >= stock {
            return Err("Out of stock until the next restock".into());
        }
    }
    if !can_afford(stash, &offer.price) {
        return Err("You can't afford that".into());
    }
    let mut trial = stash.clone();
    for (k, n) in &offer.price {
        trial.take_kind(*k, *n);
    }
    for it in offer.items() {
        trial
            .insert(it)
            .map_err(|_| "Not enough space in your stash".to_string())?;
    }
    *stash = trial;
    state.record_purchase(&offer.key);
    state.standing_mut(offer.trader).volume += offer.price_value();
    Ok(())
}

/// What a trader would pay for an item, or why they won't take it.
pub fn sell_price(t: TraderId, item: &Item) -> Result<u64, String> {
    if item.kind == ItemKind::Roubles {
        return Err("That's money".into());
    }
    if !t.buys(item.kind.category()) {
        return Err(format!("{} doesn't buy that", t.name()));
    }
    if item.contents.as_ref().is_some_and(|g| !g.is_empty()) {
        return Err("Empty the container first".into());
    }
    Ok(((item.value() as f32 * t.buy_rate()).round() as u64).max(1))
}

/// Sell an item from the stash; the roubles go into the stash.
pub fn sell(state: &mut TradersState, stash: &mut Grid, t: TraderId, uid: u64) -> Result<u64, String> {
    let item = &stash.get(uid).ok_or("Item not found in stash")?.item;
    let price = sell_price(t, item)?;
    let mut trial = stash.clone();
    trial.remove(uid);
    let mut left = price;
    while left > 0 {
        let n = left.min(ItemKind::Roubles.max_stack() as u64) as u32;
        trial
            .insert(Item::stack(ItemKind::Roubles, n))
            .map_err(|_| "No space for the roubles".to_string())?;
        left -= n as u64;
    }
    *stash = trial;
    state.standing_mut(t).volume += price;
    Ok(price)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rich_stash() -> Grid {
        let mut g = Grid::new(10, 30);
        g.insert(Item::stack(ItemKind::Roubles, 400_000)).unwrap();
        g
    }

    #[test]
    fn loyalty_levels_need_standing_and_volume() {
        let mut s = TradersState::default();
        assert_eq!(s.loyalty(TraderId::Prapor), 1);
        s.standing_mut(TraderId::Prapor).standing = 0.25;
        assert_eq!(s.loyalty(TraderId::Prapor), 1, "volume still missing");
        s.standing_mut(TraderId::Prapor).volume = 200_000;
        assert_eq!(s.loyalty(TraderId::Prapor), 2);
        s.standing_mut(TraderId::Prapor).standing = 0.5;
        s.standing_mut(TraderId::Prapor).volume = 600_000;
        assert_eq!(s.loyalty(TraderId::Prapor), 3);
        assert_eq!(s.loyalty(TraderId::Fence), 1);
    }

    #[test]
    fn buying_pays_delivers_and_tracks_stock() {
        let mut s = TradersState::default();
        let mut stash = rich_stash();
        let offers = offers(TraderId::Prapor, 0);
        let grach = offers
            .iter()
            .find(|o| o.item.kind == ItemKind::Weapon(ReceiverId::Grach))
            .unwrap();
        buy(&mut s, &mut stash, grach, true).unwrap();
        assert_eq!(stash.count_kind(ItemKind::Roubles), 400_000 - 18_000);
        assert_eq!(stash.count_kind(ItemKind::Weapon(ReceiverId::Grach)), 1);
        assert_eq!(s.standing(TraderId::Prapor).volume, 18_000);

        // LL2 offer is refused at LL1.
        let ak = offers
            .iter()
            .find(|o| o.item.kind == ItemKind::Weapon(ReceiverId::Ak74n))
            .unwrap();
        assert!(buy(&mut s, &mut stash, ak, true).is_err());

        // Limited stock runs out and restocks.
        s.standing_mut(TraderId::Prapor).standing = 0.3;
        s.standing_mut(TraderId::Prapor).volume = 200_000;
        let bs = offers
            .iter()
            .find(|o| o.item.kind == ItemKind::Ammo(AmmoType::Bs545) && o.loyalty == 2)
            .unwrap();
        buy(&mut s, &mut stash, bs, true).unwrap();
        buy(&mut s, &mut stash, bs, true).unwrap();
        assert!(buy(&mut s, &mut stash, bs, true).is_err());
        s.restock();
        assert!(buy(&mut s, &mut stash, bs, true).is_ok());
        assert_eq!(stash.count_kind(ItemKind::Ammo(AmmoType::Bs545)), 180);
    }

    #[test]
    fn barter_offers_take_items() {
        let mut s = TradersState::default();
        s.standing_mut(TraderId::Mechanic).standing = 0.3;
        s.standing_mut(TraderId::Mechanic).volume = 200_000;
        let mut stash = Grid::new(6, 6);
        stash
            .insert(Item::stack(ItemKind::Barter(BarterKind::CircuitBoard), 1))
            .unwrap();
        stash
            .insert(Item::stack(ItemKind::Barter(BarterKind::CircuitBoard), 1))
            .unwrap();
        let o = offers(TraderId::Mechanic, 0)
            .into_iter()
            .find(|o| o.is_barter() && o.item.kind == ItemKind::Attachment(AttachmentId::Pbs4Suppressor))
            .unwrap();
        buy(&mut s, &mut stash, &o, true).unwrap();
        assert_eq!(stash.count_kind(ItemKind::Barter(BarterKind::CircuitBoard)), 0);
        assert_eq!(stash.count_kind(ItemKind::Attachment(AttachmentId::Pbs4Suppressor)), 1);
    }

    #[test]
    fn selling_respects_categories() {
        let mut s = TradersState::default();
        let mut stash = Grid::new(6, 6);
        stash.insert(Item::new(ItemKind::Barter(BarterKind::Bitcoin))).unwrap();
        let uid = stash.items[0].item.uid;
        assert!(sell(&mut s, &mut stash, TraderId::Ragman, uid).is_err());
        let price = sell(&mut s, &mut stash, TraderId::Therapist, uid).unwrap();
        assert_eq!(price, 150_000);
        assert_eq!(stash.count_kind(ItemKind::Roubles), 150_000);
        assert_eq!(s.standing(TraderId::Therapist).volume, 150_000);
        // Fence takes anything, cheaply.
        stash.insert(Item::new(ItemKind::Armor(ArmorKind::Paca))).unwrap();
        let uid = stash
            .items
            .iter()
            .find(|p| p.item.kind != ItemKind::Roubles)
            .unwrap()
            .item
            .uid;
        assert!(sell(&mut s, &mut stash, TraderId::Fence, uid).unwrap() < 12_000);
    }

    #[test]
    fn fence_rotates_stock() {
        let a: Vec<String> = offers(TraderId::Fence, 1).iter().map(|o| o.label()).collect();
        let b: Vec<String> = offers(TraderId::Fence, 2).iter().map(|o| o.label()).collect();
        assert_eq!(a.len(), 10);
        assert_ne!(a, b);
    }
}
