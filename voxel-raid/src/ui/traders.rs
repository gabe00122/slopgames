//! Trader screen: trader list, loyalty, buy offers and tasks. (Selling reuses the
//! inventory grid and is drawn by the game.)

use egui::{Align2, Color32, FontId, RichText, Sense, Stroke, StrokeKind};

use super::inventory::value_label;
use super::style;
use crate::inventory::ItemKind;
use crate::quests::{self, Objective, QuestView};
use crate::save::Profile;
use crate::traders::{loyalty_requirement, Offer, TraderId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraderTab {
    Buy,
    Sell,
    Tasks,
}

pub struct TradersUi {
    pub selected: TraderId,
    pub tab: TraderTab,
    pub status: Option<(String, bool)>,
}

impl Default for TradersUi {
    fn default() -> Self {
        Self {
            selected: TraderId::Prapor,
            tab: TraderTab::Buy,
            status: None,
        }
    }
}

pub enum TraderAction {
    Buy(usize),
    Accept(&'static str),
    HandOver(&'static str, usize),
    TurnIn(&'static str),
}

fn c3(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn portrait(ui: &mut egui::Ui, t: TraderId, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 4.0, c3(t.color()));
    p.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, Color32::from_white_alpha(60)),
        StrokeKind::Inside,
    );
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        &t.name()[..1],
        FontId::proportional(size * 0.55),
        Color32::from_rgb(240, 236, 220),
    );
}

/// Count of active tasks that can be turned in for a trader (for the badge).
fn ready_tasks(profile: &Profile, t: TraderId) -> usize {
    quests::all()
        .iter()
        .filter(|q| q.trader == t)
        .filter(|q| profile.quests.view(q, &profile.traders, &profile.hideout) == QuestView::Ready)
        .count()
}

/// Top bar and trader list. Returns true when "back" is pressed.
pub fn frame(ui: &mut egui::Ui, st: &mut TradersUi, profile: &Profile) -> bool {
    let mut back = false;
    let balance = profile.stash.count_kind(ItemKind::Roubles) as u64;
    egui::Panel::top("traders_top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.button("◀ Main menu (Esc)").clicked() {
                back = true;
            }
            ui.separator();
            ui.label(RichText::new("TRADERS").strong().color(style::ACCENT));
            ui.label(format!("Roubles in stash: {}", value_label(balance)));
            ui.label(RichText::new("Limited stock restocks after every raid.").color(style::TEXT_DIM));
        });
    });
    egui::Panel::left("traders_list").exact_size(250.0).show(ui, |ui| {
        ui.add_space(8.0);
        for t in TraderId::ALL {
            let selected = st.selected == t;
            let fill = if selected {
                Color32::from_rgb(52, 50, 38)
            } else {
                Color32::from_rgb(26, 28, 27)
            };
            let resp = egui::Frame::NONE
                .fill(fill)
                .corner_radius(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_width(226.0);
                    ui.horizontal(|ui| {
                        portrait(ui, t, 46.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(t.name()).size(17.0).strong());
                            let s = profile.traders.standing(t);
                            ui.label(
                                RichText::new(format!(
                                    "LL {}  ·  standing {:.2}",
                                    profile.traders.loyalty(t),
                                    s.standing
                                ))
                                .size(12.0)
                                .color(style::TEXT_DIM),
                            );
                            let ready = ready_tasks(profile, t);
                            if ready > 0 {
                                ui.label(
                                    RichText::new(format!("{ready} task(s) ready"))
                                        .size(12.0)
                                        .color(style::GOOD),
                                );
                            }
                        });
                    });
                })
                .response
                .interact(Sense::click());
            if resp.clicked() && !selected {
                st.selected = t;
                st.status = None;
                if t == TraderId::Fence && st.tab == TraderTab::Tasks {
                    st.tab = TraderTab::Buy;
                }
            }
            ui.add_space(6.0);
        }
    });
    back
}

/// Trader header with loyalty progress and tab selector (inside the central panel).
pub fn header(ui: &mut egui::Ui, st: &mut TradersUi, profile: &Profile) {
    let t = st.selected;
    ui.horizontal(|ui| {
        portrait(ui, t, 64.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(t.name())
                    .size(26.0)
                    .strong()
                    .color(Color32::from_rgb(232, 226, 205)),
            );
            ui.label(RichText::new(t.description()).color(style::TEXT_DIM));
            let ll = profile.traders.loyalty(t);
            let s = profile.traders.standing(t);
            let mut line = format!("Loyalty level {} / {}", ll, t.max_loyalty());
            if ll < t.max_loyalty() {
                let (need_s, need_v) = loyalty_requirement(ll + 1);
                line.push_str(&format!(
                    "   ·   next: standing {:.2} / {:.2}, trade volume {} / {}",
                    s.standing,
                    need_s,
                    value_label(s.volume),
                    value_label(need_v)
                ));
            }
            ui.label(RichText::new(line).color(style::ACCENT));
            ui.label(
                RichText::new(format!("Buys: {}", t.buys_label()))
                    .size(12.0)
                    .color(style::TEXT_DIM),
            );
        });
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let tabs: &[(TraderTab, &str)] = if t == TraderId::Fence {
            &[(TraderTab::Buy, "Buy"), (TraderTab::Sell, "Sell")]
        } else {
            &[
                (TraderTab::Buy, "Buy"),
                (TraderTab::Sell, "Sell"),
                (TraderTab::Tasks, "Tasks"),
            ]
        };
        for (tab, label) in tabs {
            let text = RichText::new(*label).size(16.0);
            if ui.selectable_label(st.tab == *tab, text).clicked() {
                st.tab = *tab;
                st.status = None;
            }
        }
        if let Some((msg, err)) = &st.status {
            ui.separator();
            ui.label(RichText::new(msg).color(if *err { style::BAD } else { style::GOOD }));
        }
    });
    ui.separator();
}

fn price_label(ui: &mut egui::Ui, profile: &Profile, o: &Offer) {
    for (k, n) in &o.price {
        let have = profile.stash.count_kind(*k);
        let ok = have >= *n;
        let text = if *k == ItemKind::Roubles {
            value_label(*n as u64)
        } else {
            format!("{}x {} ({}/{})", n, k.name(), have, n)
        };
        ui.label(RichText::new(text).color(if ok {
            Color32::from_rgb(230, 225, 200)
        } else {
            style::BAD
        }));
    }
}

pub fn buy_tab(ui: &mut egui::Ui, profile: &Profile, offers: &[Offer]) -> Option<TraderAction> {
    let mut action = None;
    let t = offers.first().map(|o| o.trader);
    let ll = t.map(|t| profile.traders.loyalty(t)).unwrap_or(1);
    egui::ScrollArea::vertical().id_salt("offers").show(ui, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        egui::Grid::new("offer_grid")
            .num_columns(5)
            .striped(true)
            .spacing([18.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("ITEM").color(style::TEXT_DIM));
                ui.label(RichText::new("PRICE").color(style::TEXT_DIM));
                ui.label(RichText::new("LL").color(style::TEXT_DIM));
                ui.label(RichText::new("STOCK").color(style::TEXT_DIM));
                ui.label("");
                ui.end_row();
                for (i, o) in offers.iter().enumerate() {
                    let quest_ok = o.unlocked_by.is_none_or(|q| profile.quests.is_completed(q));
                    let ll_ok = ll >= o.loyalty;
                    let left = o.stock.map(|s| s.saturating_sub(profile.traders.purchased(&o.key)));
                    let name_col = if ll_ok && quest_ok {
                        Color32::from_rgb(232, 228, 210)
                    } else {
                        style::TEXT_DIM
                    };
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(o.label()).color(name_col).strong())
                                .on_hover_text(o.item.kind.description());
                            if o.is_barter() {
                                ui.label(RichText::new("BARTER").size(10.5).color(style::WARN));
                            }
                        });
                        if let Some(w) = &o.item.weapon {
                            let s = w.stats();
                            ui.label(
                                RichText::new(format!(
                                    "ergo {:.0} · recoil {:.2}° · {} rds",
                                    s.ergonomics, s.vertical_recoil, s.mag_size
                                ))
                                .size(11.5)
                                .color(style::TEXT_DIM),
                            );
                        } else if let Some(d) = o.item.durability {
                            ui.label(
                                RichText::new(format!("durability {d:.0}"))
                                    .size(11.5)
                                    .color(style::TEXT_DIM),
                            );
                        }
                    });
                    ui.vertical(|ui| price_label(ui, profile, o));
                    ui.label(RichText::new(format!("{}", o.loyalty)).color(if ll_ok {
                        style::GOOD
                    } else {
                        style::BAD
                    }));
                    ui.label(match left {
                        Some(n) => {
                            RichText::new(format!("{n} left")).color(if n > 0 { style::WARN } else { style::BAD })
                        }
                        None => RichText::new("-").color(style::TEXT_DIM),
                    });
                    if !quest_ok {
                        let name = o.unlocked_by.and_then(quests::get).map(|q| q.name).unwrap_or("?");
                        ui.label(RichText::new(format!("Task: {name}")).color(style::WARN));
                    } else if !ll_ok {
                        ui.label(RichText::new(format!("Loyalty {} required", o.loyalty)).color(style::TEXT_DIM));
                    } else {
                        let can = crate::traders::can_afford(&profile.stash, &o.price) && left != Some(0);
                        if ui.add_enabled(can, egui::Button::new("Buy")).clicked() {
                            action = Some(TraderAction::Buy(i));
                        }
                    }
                    ui.end_row();
                }
            });
    });
    action
}

fn status_badge(ui: &mut egui::Ui, view: &QuestView) {
    let (text, col) = match view {
        QuestView::Locked(_) => ("LOCKED", style::TEXT_DIM),
        QuestView::Available => ("AVAILABLE", style::ACCENT),
        QuestView::Active => ("ACTIVE", Color32::from_rgb(120, 170, 230)),
        QuestView::Ready => ("READY", style::GOOD),
        QuestView::Completed => ("COMPLETED", Color32::from_rgb(110, 130, 110)),
    };
    egui::Frame::NONE
        .fill(col.gamma_multiply(0.25))
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.5).strong().color(col));
        });
}

fn progress_bar(ui: &mut egui::Ui, cur: u32, target: u32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 8.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, Color32::from_rgb(40, 42, 40));
    let mut fill = rect;
    fill.set_width(rect.width() * (cur as f32 / target.max(1) as f32).min(1.0));
    p.rect_filled(fill, 2.0, if cur >= target { style::GOOD } else { style::ACCENT_DIM });
}

pub fn tasks_tab(ui: &mut egui::Ui, profile: &Profile, t: TraderId) -> Option<TraderAction> {
    let mut action = None;
    egui::ScrollArea::vertical().id_salt("tasks").show(ui, |ui| {
        for q in quests::all().iter().filter(|q| q.trader == t) {
            let view = profile.quests.view(q, &profile.traders, &profile.hideout);
            let active = matches!(view, QuestView::Active | QuestView::Ready);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width().min(900.0));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(q.name).size(17.0).strong());
                    status_badge(ui, &view);
                    if q.loyalty > 1 {
                        ui.label(
                            RichText::new(format!("LL{}", q.loyalty))
                                .size(11.5)
                                .color(style::TEXT_DIM),
                        );
                    }
                });
                if view == QuestView::Completed {
                    return;
                }
                ui.label(RichText::new(q.description).color(style::TEXT_DIM));
                if let QuestView::Locked(reason) = &view {
                    ui.label(RichText::new(reason).color(style::WARN));
                    return;
                }
                ui.add_space(4.0);
                for (i, o) in q.objectives.iter().enumerate() {
                    let cur = profile.quests.progress(q, i, &profile.hideout);
                    let target = o.target();
                    ui.horizontal(|ui| {
                        let done = cur >= target;
                        let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
                        if done {
                            ui.painter().rect_filled(r, 2.0, style::GOOD);
                        } else {
                            ui.painter()
                                .rect_stroke(r, 2.0, Stroke::new(1.5, style::TEXT_DIM), StrokeKind::Inside);
                        }
                        ui.label(o.describe());
                        progress_bar(ui, cur, target);
                        ui.label(RichText::new(format!("{cur}/{target}")).monospace());
                        if active && !done {
                            match o {
                                Objective::HandOver { item, .. } => {
                                    let have = profile.stash.count_kind(*item);
                                    if ui
                                        .add_enabled(have > 0, egui::Button::new(format!("Hand over (have {have})")))
                                        .clicked()
                                    {
                                        action = Some(TraderAction::HandOver(q.id, i));
                                    }
                                }
                                Objective::HandOverWeapon { receiver, req } => {
                                    let ready = profile.stash.items.iter().any(|p| {
                                        p.item
                                            .weapon
                                            .as_ref()
                                            .is_some_and(|w| w.receiver == *receiver && req.satisfied(w))
                                    });
                                    if ui
                                        .add_enabled(ready, egui::Button::new("Hand over weapon"))
                                        .on_disabled_hover_text(
                                            "Build a weapon in your stash that meets every requirement",
                                        )
                                        .clicked()
                                    {
                                        action = Some(TraderAction::HandOver(q.id, i));
                                    }
                                }
                                o if o.is_raid() => {
                                    ui.label(RichText::new("(in raid)").size(11.5).color(style::TEXT_DIM));
                                }
                                _ => {}
                            }
                        }
                    });
                    // Gunsmith requirements checked against your best candidate in the stash.
                    if let Objective::HandOverWeapon { receiver, req } = o {
                        if cur < target {
                            let candidates: Vec<&crate::weapons::Weapon> = profile
                                .stash
                                .items
                                .iter()
                                .filter_map(|p| p.item.weapon.as_ref())
                                .filter(|w| w.receiver == *receiver)
                                .collect();
                            let best = candidates
                                .iter()
                                .copied()
                                .find(|w| req.satisfied(w))
                                .or_else(|| candidates.first().copied());
                            if best.is_none() {
                                ui.label(
                                    RichText::new(format!("   No {} in your stash", receiver.def().name))
                                        .size(12.0)
                                        .color(style::WARN),
                                );
                            }
                            for (line, ok) in req.lines(best) {
                                let col = match ok {
                                    Some(true) => style::GOOD,
                                    Some(false) => style::BAD,
                                    None => style::TEXT_DIM,
                                };
                                ui.label(RichText::new(format!("      · {line}")).size(12.5).color(col));
                            }
                        }
                    }
                }
                ui.add_space(4.0);
                let rewards: Vec<String> = q.rewards.iter().map(|r| r.describe()).collect();
                ui.label(RichText::new(format!("Rewards: {}", rewards.join(", "))).color(style::ACCENT));
                match view {
                    QuestView::Available => {
                        if ui.button(RichText::new("Accept task").strong()).clicked() {
                            action = Some(TraderAction::Accept(q.id));
                        }
                    }
                    QuestView::Ready => {
                        if ui
                            .button(
                                RichText::new("Complete task & collect rewards")
                                    .strong()
                                    .color(style::GOOD),
                            )
                            .clicked()
                        {
                            action = Some(TraderAction::TurnIn(q.id));
                        }
                    }
                    _ => {}
                }
            });
            ui.add_space(6.0);
        }
    });
    action
}
