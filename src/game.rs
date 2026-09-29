//! Top-level game state: screens, raid lifecycle, saving and scene assembly.

use std::path::PathBuf;

use glam::{Mat4, Vec2, Vec3};
use winit::keyboard::KeyCode;

use crate::ai::ScavState;
use crate::hideout::world::{HideoutInteract, HideoutSession};
use crate::input::Input;
use crate::inventory::{EquipSlot, GridParts, GridRef, Item, ItemKind};
use crate::player::MoveInput;
use crate::raid::{Interact, OutcomeKind, Raid, RaidOutcome};
use crate::render::models::{self, HumanoidLook, HumanoidPose, ViewmodelParams};
use crate::render::{FrameScene, MeshBuilder, Renderer};
use crate::rng::Rng;
use crate::save::{self, Profile};
use crate::ui;
use crate::ui::inventory::{InvAction, InvCtx, InventoryUi, Origin};
use crate::ui::menu::{MainMenuAction, PauseAction};
use crate::weapons::{AmmoType, ReceiverId, Weapon};

pub struct Settings {
    /// Radians per mouse count.
    pub sensitivity: f32,
    pub fov_deg: f32,
    pub show_debug: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sensitivity: 0.0022,
            fov_deg: 72.0,
            show_debug: false,
        }
    }
}

#[derive(Default)]
pub struct FrameStats {
    pub fps: f32,
    acc: f32,
    frames: u32,
    pub chunks_drawn: usize,
    pub triangles: usize,
}

impl FrameStats {
    fn tick(&mut self, dt: f32) {
        self.acc += dt;
        self.frames += 1;
        if self.acc >= 0.5 {
            self.fps = self.frames as f32 / self.acc;
            self.acc = 0.0;
            self.frames = 0;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    MainMenu,
    Stash,
    Raid,
    Summary,
    Hideout,
    Traders,
}

/// A weapon taken out of the stash/equipment while it is being modded.
pub struct ModdingSession {
    pub item: Item,
    pub origin: Origin,
    pub ui: ui::modding::ModdingUi,
}

pub struct Game {
    pub profile: Profile,
    save_path: PathBuf,
    pub screen: Screen,
    pub raid: Option<Raid>,
    pub hideout: Option<HideoutSession>,
    pub last_outcome: Option<RaidOutcome>,
    pub settings: Settings,
    pub paused: bool,
    pub quit_requested: bool,
    pub stats: FrameStats,
    pub adapter_info: String,
    dynamic: MeshBuilder,
    viewmodel: MeshBuilder,
    world_reload: bool,
    world_clear: bool,
    pub rng: Rng,
    pub time: f32,
    /// Smoothed mouse delta for weapon sway.
    sway: Vec2,
    /// Debug: keep the weapon aimed down sights (screenshots).
    pub debug_force_ads: bool,
    pub inv: InventoryUi,
    pub traders_ui: ui::traders::TradersUi,
    pub raid_inventory: bool,
    pub modding: Option<ModdingSession>,
    pub notice: Option<String>,
    confirm_reset: bool,
    /// Fixed seed for the next raid (from the command line).
    pub raid_seed: Option<u64>,
}

impl Game {
    pub fn new(renderer: &Renderer, save_path: PathBuf) -> Self {
        let (profile, warning) = save::load_or_new(&save_path);
        if let Some(w) = &warning {
            log::warn!("{w}");
        }
        let settings = Settings {
            sensitivity: profile.settings.sensitivity,
            fov_deg: profile.settings.fov,
            show_debug: false,
        };
        let mut game = Self {
            profile,
            save_path,
            screen: Screen::MainMenu,
            raid: None,
            hideout: None,
            last_outcome: None,
            settings,
            paused: false,
            quit_requested: false,
            stats: FrameStats::default(),
            adapter_info: renderer.adapter_info.clone(),
            dynamic: MeshBuilder::new(),
            viewmodel: MeshBuilder::new(),
            world_reload: false,
            world_clear: false,
            rng: Rng::from_time(),
            time: 0.0,
            sway: Vec2::ZERO,
            debug_force_ads: false,
            inv: InventoryUi::default(),
            traders_ui: Default::default(),
            raid_inventory: false,
            modding: None,
            notice: warning,
            confirm_reset: false,
            raid_seed: None,
        };
        game.save();
        game
    }

    pub fn save(&mut self) {
        self.profile.settings.sensitivity = self.settings.sensitivity;
        self.profile.settings.fov = self.settings.fov_deg;
        if let Err(e) = save::save(&self.profile, &self.save_path) {
            log::error!("Failed to save profile to {}: {e}", self.save_path.display());
            self.notice = Some(format!("Could not save: {e}"));
        }
    }

    /// Should the OS cursor be captured for mouse-look?
    pub fn wants_cursor_grab(&self) -> bool {
        match (&self.raid, self.screen) {
            (Some(r), Screen::Raid) => !self.paused && !self.raid_inventory && r.dead.is_none() && r.finished.is_none(),
            (_, Screen::Hideout) => match &self.hideout {
                Some(h) => !self.paused && !h.stash_open && h.open_station.is_none() && self.modding.is_none(),
                None => false,
            },
            _ => false,
        }
    }

    // ------------------------------------------------------------------
    // Raid lifecycle
    // ------------------------------------------------------------------

    pub fn start_raid(&mut self) {
        self.close_modding();
        // Save first so a crash mid-raid never costs the player their gear.
        self.save();
        let equipment = std::mem::take(&mut self.profile.equipment);
        let seed = self.raid_seed.take().unwrap_or_else(|| self.rng.next_u64());
        let t0 = std::time::Instant::now();
        let tracker = self.profile.quests.tracker();
        self.raid = Some(Raid::new(seed, equipment, tracker));
        log::info!(
            "Generated raid (seed {seed}) in {:.1} ms",
            t0.elapsed().as_secs_f32() * 1000.0
        );
        self.screen = Screen::Raid;
        self.world_reload = true;
        self.paused = false;
        self.raid_inventory = false;
        self.notice = None;
    }

    fn close_raid_inventory(&mut self) {
        if let Some(raid) = self.raid.as_mut() {
            let (equipment, loot) = raid.equipment_and_loot();
            let mut ctx = InvCtx {
                equipment,
                stash: None,
                loot,
                loot_name: String::new(),
                in_raid: true,
                sell_to: None,
            };
            self.inv.cancel(&mut ctx);
            raid.open_loot = None;
        }
        self.raid_inventory = false;
    }

    fn end_raid(&mut self) {
        self.close_raid_inventory();
        let Some(raid) = self.raid.take() else { return };
        let outcome = raid.finished.clone().unwrap_or(RaidOutcome {
            kind: OutcomeKind::MissingInAction,
            time: raid.time,
            kills: raid.kills,
            value_in: raid.value_in,
            value_out: 0,
            tasks: raid.quests.summary(),
        });
        // Task progress (kills, searches, destruction) counts even if you don't make it out.
        self.profile.quests.merge(&raid.quests);
        save::apply_raid_result(&mut self.profile, raid.equipment, &outcome);
        self.last_outcome = Some(outcome);
        self.screen = Screen::Summary;
        self.world_clear = true;
        self.paused = false;
        self.save();
    }

    // ------------------------------------------------------------------
    // Hideout
    // ------------------------------------------------------------------

    pub fn enter_hideout(&mut self) {
        self.hideout = Some(HideoutSession::new(&self.profile.hideout));
        self.screen = Screen::Hideout;
        self.world_reload = true;
        self.paused = false;
        self.notice = None;
    }

    fn save_hideout_edits(&mut self) {
        if let Some(h) = &self.hideout {
            self.profile.hideout.edits = h.edits_vec();
        }
    }

    fn leave_hideout(&mut self) {
        self.close_modding();
        self.close_stash_overlay();
        self.save_hideout_edits();
        self.hideout = None;
        self.world_clear = true;
        self.screen = Screen::MainMenu;
        self.paused = false;
        self.save();
    }

    fn close_stash_overlay(&mut self) {
        let mut ctx = InvCtx {
            equipment: &mut self.profile.equipment,
            stash: Some(&mut self.profile.stash),
            loot: None,
            loot_name: String::new(),
            in_raid: false,
            sell_to: None,
        };
        self.inv.cancel(&mut ctx);
        if let Some(h) = self.hideout.as_mut() {
            if h.stash_open {
                h.stash_open = false;
                self.save_hideout_edits();
                self.save();
            }
        }
    }

    fn update_hideout(&mut self, dt: f32, input: &Input, renderer: &mut Renderer) {
        let Some(h) = self.hideout.as_mut() else {
            self.screen = Screen::MainMenu;
            return;
        };
        if self.world_reload {
            renderer.load_world(&mut h.world);
            self.world_reload = false;
        }
        let mut close_stash = false;
        let mut close_mod = false;
        if input.pressed(KeyCode::Escape) {
            if self.modding.is_some() {
                close_mod = true;
            } else if h.stash_open {
                close_stash = true;
            } else if h.open_station.is_some() {
                h.open_station = None;
            } else {
                self.paused = !self.paused;
            }
        } else if !self.paused && self.modding.is_none() {
            if input.pressed(KeyCode::KeyE) || input.pressed(KeyCode::KeyF) {
                if h.open_station.is_some() {
                    h.open_station = None;
                } else if !h.stash_open {
                    match h.interaction() {
                        Some(HideoutInteract::Stash) => h.stash_open = true,
                        Some(HideoutInteract::Station(s)) => {
                            h.open_station = Some(s);
                            h.station_status = None;
                        }
                        None => {}
                    }
                }
            } else if input.pressed(KeyCode::Tab) && h.open_station.is_none() {
                if h.stash_open {
                    close_stash = true;
                } else {
                    h.stash_open = true;
                }
            }
        }
        let accept = !self.paused && !h.stash_open && h.open_station.is_none() && self.modding.is_none();
        if !self.paused {
            h.update(dt, input, &self.settings, accept);
        }
        renderer.sync_world(&mut h.world);
        if close_mod {
            self.close_modding();
        }
        if close_stash {
            self.close_stash_overlay();
        }
    }

    fn hideout_ui(&mut self, ui: &mut egui::Ui) {
        if self.modding.is_some() {
            self.modding_ui(ui);
            return;
        }
        let stash_open = self.hideout.as_ref().is_some_and(|h| h.stash_open);
        if stash_open {
            if self.stash_ui(ui, "◀ Back to hideout (Esc)") {
                self.close_stash_overlay();
            }
            return;
        }
        let Some(h) = self.hideout.as_mut() else { return };
        ui::hideout::draw_hud(ui, h, &self.profile.hideout);
        if let Some(station) = h.open_station {
            let action = ui::hideout::station_window(
                ui,
                station,
                &self.profile.hideout,
                &self.profile.stash,
                h.station_status.as_ref(),
            );
            match action {
                Some(ui::hideout::StationAction::Upgrade) => {
                    match crate::hideout::upgrade(&mut self.profile.hideout, &mut self.profile.stash, station) {
                        Ok(level) => {
                            crate::hideout::world::apply_station_decor(&mut h.world, station, level);
                            h.station_status = Some((format!("{} upgraded to level {level}", station.name()), false));
                            h.message(format!("{} upgraded to level {level}!", station.name()));
                        }
                        Err(e) => h.station_status = Some((e, true)),
                    }
                    self.save_hideout_edits();
                    self.save();
                }
                Some(ui::hideout::StationAction::Craft(i)) => {
                    let all = crate::hideout::recipes();
                    if let Some(r) = all.get(i) {
                        match crate::hideout::craft(&self.profile.hideout, &mut self.profile.stash, r) {
                            Ok(()) => h.station_status = Some((format!("Crafted {} (sent to stash)", r.name()), false)),
                            Err(e) => h.station_status = Some((e, true)),
                        }
                    }
                    self.save();
                }
                Some(ui::hideout::StationAction::Close) => {
                    if let Some(h) = self.hideout.as_mut() {
                        h.open_station = None;
                    }
                }
                None => {}
            }
        }
        if self.paused {
            match ui::menu::pause_menu(ui, false, &mut self.settings) {
                Some(PauseAction::Resume) => self.paused = false,
                Some(PauseAction::BackToMenu) => self.leave_hideout(),
                Some(PauseAction::Quit) => self.quit_requested = true,
                _ => {}
            }
        }
    }

    // ------------------------------------------------------------------
    // Modding (out of raid)
    // ------------------------------------------------------------------

    fn open_modding(&mut self, item: Item, origin: Origin) {
        self.modding = Some(ModdingSession {
            item,
            origin,
            ui: Default::default(),
        });
    }

    fn close_modding(&mut self) {
        let Some(session) = self.modding.take() else { return };
        let item = session.item;
        let result = match session.origin {
            Origin::Grid {
                grid: GridRef::Stash,
                x,
                y,
                rotated,
            } => self.profile.stash.place(item, x, y, rotated),
            Origin::Slot(s) if self.profile.equipment.slot(s).is_none() => {
                *self.profile.equipment.slot_mut(s) = Some(item);
                Ok(())
            }
            _ => Err(item),
        };
        if let Err(item) = result {
            if let Err(item) = self.profile.stash.insert(item) {
                // Never lose the weapon: force it into the stash list.
                self.profile.stash.items.push(crate::inventory::Placed {
                    item,
                    x: 0,
                    y: 0,
                    rotated: false,
                });
            }
        }
        self.save();
    }

    fn stash_ammo(&mut self, ammo: AmmoType, mut n: u32) {
        while n > 0 {
            let chunk = n.min(60);
            n -= chunk;
            let it = Item::stack(ItemKind::Ammo(ammo), chunk);
            if let Err(it) = self.profile.stash.insert(it) {
                let _ = self.profile.equipment.stow(it);
            }
        }
    }

    // ------------------------------------------------------------------
    // Update
    // ------------------------------------------------------------------

    pub fn update(&mut self, dt: f32, real_dt: f32, input: &Input, renderer: &mut Renderer) {
        self.time += dt;
        self.stats.tick(real_dt);
        self.stats.chunks_drawn = renderer.stats_chunks_drawn;
        self.stats.triangles = renderer.stats_triangles;
        if input.pressed(KeyCode::F3) {
            self.settings.show_debug = !self.settings.show_debug;
        }
        let target_sway = Vec2::new(-input.mouse_delta.x, input.mouse_delta.y) * 0.00035;
        self.sway +=
            (target_sway.clamp(Vec2::splat(-0.03), Vec2::splat(0.03)) - self.sway) * (1.0 - (-10.0 * dt).exp());

        if self.world_clear {
            renderer.clear_world();
            self.world_clear = false;
        }

        match self.screen {
            Screen::Raid => self.update_raid(dt, input, renderer),
            Screen::Hideout => self.update_hideout(dt, input, renderer),
            Screen::Traders => {
                if input.pressed(KeyCode::Escape) {
                    self.leave_traders();
                }
            }
            Screen::Stash => {
                if input.pressed(KeyCode::Escape) {
                    if self.modding.is_some() {
                        self.close_modding();
                    } else {
                        self.leave_stash();
                    }
                }
            }
            Screen::MainMenu | Screen::Summary => {}
        }
    }

    fn fence_seed(&self) -> u64 {
        self.profile.stats.raids as u64 + 1
    }

    fn leave_traders(&mut self) {
        let mut ctx = InvCtx {
            equipment: &mut self.profile.equipment,
            stash: Some(&mut self.profile.stash),
            loot: None,
            loot_name: String::new(),
            in_raid: false,
            sell_to: None,
        };
        self.inv.cancel(&mut ctx);
        self.screen = Screen::MainMenu;
        self.save();
    }

    fn traders_screen_ui(&mut self, ui: &mut egui::Ui) {
        use ui::traders::{TraderAction, TraderTab};
        if ui::traders::frame(ui, &mut self.traders_ui, &self.profile) {
            self.leave_traders();
            return;
        }
        let trader = self.traders_ui.selected;
        let offers = crate::traders::offers(trader, self.fence_seed());
        let mut action = None;
        let mut sold = Vec::new();
        let tab = self.traders_ui.tab;
        egui::CentralPanel::default_margins().show(ui, |ui| {
            ui::traders::header(ui, &mut self.traders_ui, &self.profile);
            match tab {
                TraderTab::Buy => action = ui::traders::buy_tab(ui, &self.profile, &offers),
                TraderTab::Tasks => action = ui::traders::tasks_tab(ui, &self.profile, trader),
                TraderTab::Sell => {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} buys {}. Right-click an item to sell it, or Ctrl+click to sell instantly.",
                            trader.name(),
                            trader.buys_label()
                        ))
                        .color(ui::style::TEXT_DIM),
                    );
                    let mut ctx = InvCtx {
                        equipment: &mut self.profile.equipment,
                        stash: Some(&mut self.profile.stash),
                        loot: None,
                        loot_name: String::new(),
                        in_raid: false,
                        sell_to: Some(trader),
                    };
                    let inv = &mut self.inv;
                    egui::ScrollArea::vertical().id_salt("sell_scroll").show(ui, |ui| {
                        inv.grid(ui, &ctx, GridRef::Stash);
                    });
                    for a in inv.end(ui.ctx(), &mut ctx) {
                        if let InvAction::Sell(uid) = a {
                            sold.push(uid);
                        }
                    }
                }
            }
        });
        for uid in sold {
            let name = self.profile.stash.get(uid).map(|p| p.item.name()).unwrap_or("item");
            match crate::traders::sell(&mut self.profile.traders, &mut self.profile.stash, trader, uid) {
                Ok(p) => {
                    self.traders_ui.status = Some((format!("Sold {name} for {}", ui::inventory::value_label(p)), false))
                }
                Err(e) => self.traders_ui.status = Some((e, true)),
            }
            self.save();
        }
        let Some(action) = action else { return };
        let status = match action {
            TraderAction::Buy(i) => match offers.get(i) {
                Some(o) => {
                    let unlocked = o.unlocked_by.is_none_or(|q| self.profile.quests.is_completed(q));
                    crate::traders::buy(&mut self.profile.traders, &mut self.profile.stash, o, unlocked)
                        .map(|_| format!("Bought {} - delivered to your stash", o.label()))
                }
                None => Err("Offer no longer available".into()),
            },
            TraderAction::Accept(id) => match crate::quests::get(id) {
                Some(q) => {
                    self.profile.quests.accept(q);
                    Ok(format!("Task \"{}\" accepted", q.name))
                }
                None => Err("Unknown task".into()),
            },
            TraderAction::HandOver(id, i) => match crate::quests::get(id) {
                Some(q) => self.profile.quests.hand_over(q, i, &mut self.profile.stash),
                None => Err("Unknown task".into()),
            },
            TraderAction::TurnIn(id) => match crate::quests::get(id) {
                Some(q) => self
                    .profile
                    .quests
                    .turn_in(
                        q,
                        &mut self.profile.stash,
                        &mut self.profile.traders,
                        &self.profile.hideout,
                    )
                    .map(|_| format!("Task \"{}\" complete! Rewards sent to your stash.", q.name)),
                None => Err("Unknown task".into()),
            },
        };
        self.traders_ui.status = Some(match status {
            Ok(m) => (m, false),
            Err(e) => (e, true),
        });
        self.save();
    }

    fn leave_stash(&mut self) {
        let mut ctx = InvCtx {
            equipment: &mut self.profile.equipment,
            stash: Some(&mut self.profile.stash),
            loot: None,
            loot_name: String::new(),
            in_raid: false,
            sell_to: None,
        };
        self.inv.cancel(&mut ctx);
        self.screen = Screen::MainMenu;
        self.save();
    }

    fn update_raid(&mut self, dt: f32, input: &Input, renderer: &mut Renderer) {
        let Some(raid) = self.raid.as_mut() else {
            self.screen = Screen::MainMenu;
            return;
        };
        if self.world_reload {
            renderer.load_world(&mut raid.world);
            self.world_reload = false;
        }
        let mut close_inventory = false;
        if input.pressed(KeyCode::Escape) {
            if self.raid_inventory {
                close_inventory = true;
            } else {
                self.paused = !self.paused;
            }
        }
        if !self.paused && raid.dead.is_none() {
            if input.pressed(KeyCode::Tab) {
                if self.raid_inventory {
                    close_inventory = true;
                } else {
                    self.raid_inventory = true;
                    raid.open_loot = None;
                }
            } else if !self.raid_inventory && input.pressed(KeyCode::KeyF) {
                if let Some(i) = raid.interaction() {
                    raid.open(i);
                    self.raid_inventory = true;
                }
            }
        }
        if !self.paused {
            raid.update(dt, input, &self.settings, !self.raid_inventory);
            if self.debug_force_ads {
                raid.gun.ads = 1.0;
            }
        }
        renderer.sync_world(&mut raid.world);
        let dead = raid.dead.is_some();
        let finished = raid.finished.is_some();
        if close_inventory || (dead && self.raid_inventory) {
            self.close_raid_inventory();
        }
        if finished {
            self.end_raid();
        }
    }

    // ------------------------------------------------------------------
    // UI
    // ------------------------------------------------------------------

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        match self.screen {
            Screen::MainMenu => self.main_menu_ui(ui),
            Screen::Stash => {
                if self.modding.is_some() {
                    self.modding_ui(ui);
                } else if self.stash_ui(ui, "◀ Main menu (Esc)") {
                    self.leave_stash();
                }
            }
            Screen::Raid => self.raid_ui(ui),
            Screen::Hideout => self.hideout_ui(ui),
            Screen::Traders => self.traders_screen_ui(ui),
            Screen::Summary => match &self.last_outcome {
                Some(o) => {
                    if ui::menu::raid_summary(ui, o) {
                        self.screen = Screen::MainMenu;
                    }
                }
                None => self.screen = Screen::MainMenu,
            },
        }
    }

    fn main_menu_ui(&mut self, ui: &mut egui::Ui) {
        let action = ui::menu::main_menu(ui, &self.profile, self.notice.as_deref(), &mut self.confirm_reset, true);
        match action {
            Some(MainMenuAction::StartRaid) => self.start_raid(),
            Some(MainMenuAction::Traders) => {
                self.screen = Screen::Traders;
                self.traders_ui.status = None;
                self.notice = None;
            }
            Some(MainMenuAction::Stash) => {
                self.screen = Screen::Stash;
                self.notice = None;
            }
            Some(MainMenuAction::Hideout) => self.enter_hideout(),
            Some(MainMenuAction::Quit) => self.quit_requested = true,
            Some(MainMenuAction::EmergencyKit) => {
                let kit = [
                    Item::with_weapon(Weapon::new(ReceiverId::Grach).loaded_with(AmmoType::Pst9, 17)),
                    Item::stack(ItemKind::Ammo(AmmoType::Pst9), 51),
                    Item::new(ItemKind::Rig(crate::inventory::RigKind::ScavVest)),
                    Item::new(ItemKind::Med(crate::inventory::MedKind::Ai2)),
                ];
                for it in kit {
                    let _ = self.profile.stash.insert(it);
                }
                self.notice = Some("Emergency kit delivered to your stash.".into());
                self.save();
            }
            Some(MainMenuAction::ResetProfile) => {
                self.profile = Profile::new_player();
                self.confirm_reset = false;
                self.notice = Some("Profile wiped. Fresh start!".into());
                self.save();
            }
            None => {}
        }
    }

    /// Stash + equipment screen. Returns true when the back button is pressed.
    fn stash_ui(&mut self, ui: &mut egui::Ui, back_label: &str) -> bool {
        let mut back = false;
        let mut sort = false;
        egui::Panel::top("stash_top").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(back_label).clicked() {
                    back = true;
                }
                ui.separator();
                ui.label(egui::RichText::new("STASH").strong().color(ui::style::ACCENT));
                ui.label(format!(
                    "Stash value: {}",
                    ui::inventory::value_label(self.profile.stash.total_value())
                ));
                ui.label(format!(
                    "Gear value: {}",
                    ui::inventory::value_label(self.profile.equipment.total_value())
                ));
                if ui.button("Sort stash").clicked() {
                    sort = true;
                }
            });
        });
        if sort {
            self.profile.stash.sort();
        }
        let mut ctx = InvCtx {
            equipment: &mut self.profile.equipment,
            stash: Some(&mut self.profile.stash),
            loot: None,
            loot_name: String::new(),
            in_raid: false,
            sell_to: None,
        };
        let inv = &mut self.inv;
        egui::Panel::left("stash_equipment").exact_size(720.0).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("eq_scroll").show(ui, |ui| {
                ui::inventory::equipment_column(ui, inv, &ctx);
            });
        });
        egui::CentralPanel::default_margins().show(ui, |ui| {
            ui.label(egui::RichText::new("STASH (10 x 30)").color(ui::style::ACCENT));
            ui.label(
                egui::RichText::new(
                    "Right-click a weapon to modify or load it. Ctrl+click moves items between stash and gear.",
                )
                .size(11.5)
                .color(ui::style::TEXT_DIM),
            );
            egui::ScrollArea::vertical().id_salt("stash_scroll").show(ui, |ui| {
                inv.grid(ui, &ctx, GridRef::Stash);
            });
        });
        let actions = inv.end(ui.ctx(), &mut ctx);
        let mut modding = None;
        for a in actions {
            if let InvAction::Modify(origin, uid) = a {
                if let Some(item) = ctx.take(origin, uid) {
                    modding = Some((item, origin));
                }
            }
        }
        if let Some((item, origin)) = modding {
            self.open_modding(item, origin);
        }
        back
    }

    fn modding_ui(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.modding.as_mut() else { return };
        let reserved = match session.origin {
            Origin::Grid {
                grid: GridRef::Stash,
                x,
                y,
                rotated,
            } => {
                let (w, h) = session.item.size();
                let (w, h) = if rotated { (h, w) } else { (w, h) };
                Some((x, y, w, h))
            }
            _ => None,
        };
        let Some(weapon) = session.item.weapon.as_mut() else {
            self.close_modding();
            return;
        };
        let mut parts = GridParts {
            grid: &mut self.profile.stash,
            reserved,
        };
        let events = ui::modding::modding_screen(ui, &mut session.ui, weapon, &mut parts);
        let mut close = false;
        let mut unloaded = Vec::new();
        for e in events {
            match e {
                ui::modding::ModdingEvent::Close => close = true,
                ui::modding::ModdingEvent::Unloaded(a, n) => unloaded.push((a, n)),
            }
        }
        for (a, n) in unloaded {
            self.stash_ammo(a, n);
        }
        if close {
            self.close_modding();
        }
    }

    fn raid_ui(&mut self, ui: &mut egui::Ui) {
        let Some(raid) = self.raid.as_mut() else { return };
        ui::hud::draw_hud(
            ui,
            raid,
            &self.settings,
            &self.stats,
            &self.adapter_info,
            self.raid_inventory,
        );
        if raid.dead.is_some() {
            if ui::hud::death_overlay(ui, raid) {
                raid.accept_death();
            }
            return;
        }
        if self.raid_inventory {
            let loot_name = raid.loot_name().unwrap_or_default();
            let (equipment, loot) = raid.equipment_and_loot();
            let has_loot = loot.is_some();
            let mut ctx = InvCtx {
                equipment,
                stash: None,
                loot,
                loot_name,
                in_raid: true,
                sell_to: None,
            };
            let inv = &mut self.inv;
            egui::CentralPanel::no_frame()
                .frame(
                    egui::Frame::NONE
                        .fill(egui::Color32::from_black_alpha(185))
                        .inner_margin(18.0),
                )
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(700.0);
                            egui::ScrollArea::vertical().id_salt("raid_eq").show(ui, |ui| {
                                ui::inventory::equipment_column(ui, inv, &ctx);
                            });
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            if has_loot {
                                ui.label(
                                    egui::RichText::new(ctx.loot_name.to_uppercase())
                                        .size(16.0)
                                        .color(ui::style::ACCENT),
                                );
                                inv.grid(ui, &ctx, GridRef::Loot);
                                ui.label(
                                    egui::RichText::new("Ctrl+click to grab quickly · Tab / Esc to close")
                                        .size(11.5)
                                        .color(ui::style::TEXT_DIM),
                                );
                            } else {
                                ui.label(egui::RichText::new("INVENTORY").size(16.0).color(ui::style::ACCENT));
                                ui.label("Look at a container or body and press F to search it.");
                                ui.label(egui::RichText::new("Tab / Esc to close").color(ui::style::TEXT_DIM));
                            }
                        });
                    });
                });
            let actions = inv.end(ui.ctx(), &mut ctx);
            drop(ctx);
            for a in actions {
                if let InvAction::UseMed(g, uid) = a {
                    raid.start_heal_with(g, uid);
                }
            }
        }
        if self.paused {
            match ui::menu::pause_menu(ui, true, &mut self.settings) {
                Some(PauseAction::Resume) => self.paused = false,
                Some(PauseAction::LeaveRaid) => {
                    raid.abandon();
                    self.paused = false;
                }
                Some(PauseAction::BackToMenu) => {}
                Some(PauseAction::Quit) => self.quit_requested = true,
                None => {}
            }
        }
    }

    // ------------------------------------------------------------------
    // Scene
    // ------------------------------------------------------------------

    pub fn build_scene(&mut self, aspect: f32) -> FrameScene<'_> {
        self.dynamic.clear();
        self.viewmodel.clear();
        let mut view_proj = Mat4::IDENTITY;
        let mut cam_pos = Vec3::ZERO;
        let mut draw_world = false;
        let mut vm_proj = None;
        let mut ambient = 0.0;
        let showcase_proj = crate::render::perspective(42f32.to_radians(), aspect, 0.05, 10.0);

        match self.screen {
            Screen::Raid => {
                if let Some(raid) = &self.raid {
                    draw_world = true;
                    let fov = (self.settings.fov_deg / raid.zoom()).to_radians();
                    let proj = crate::render::perspective(fov, aspect, 0.05, 400.0);
                    view_proj = proj * raid.player.view_matrix();
                    cam_pos = raid.player.eye_pos();
                    build_raid_entities(&mut self.dynamic, raid, self.time);
                    raid.effects.build(&mut self.dynamic);
                    if raid.dead.is_none() && !raid.is_scoped() && !self.raid_inventory {
                        if let Some(w) = raid.weapon() {
                            let g = &raid.gun;
                            let params = ViewmodelParams {
                                ads: g.ads,
                                kick: g.kick,
                                reload: g.reload.map(|r| 1.0 - r.remaining / r.total.max(0.01)),
                                draw: g.draw + if raid.heal.is_some() { 0.35 } else { 0.0 },
                                sprint: if raid.player.sprinting { 1.0 } else { 0.0 },
                                walk_phase: raid.player.walk_phase,
                                moving: (raid.player.horizontal_speed() / 4.0).min(1.5),
                                flash: g.flash > 0.0,
                                sway: self.sway,
                                time: self.time,
                            };
                            models::viewmodel(&mut self.viewmodel, w, &params);
                            vm_proj = Some(crate::render::perspective(62f32.to_radians(), aspect, 0.01, 10.0));
                        }
                    }
                }
            }
            Screen::Stash => {
                if let Some(w) = self.modding.as_ref().and_then(|m| m.item.weapon.as_ref()) {
                    models::showcase(&mut self.viewmodel, w, self.time);
                    vm_proj = Some(showcase_proj);
                    ambient = 0.9;
                }
            }
            Screen::MainMenu => {
                let w = self
                    .profile
                    .equipment
                    .weapon(EquipSlot::Primary)
                    .or(self.profile.equipment.weapon(EquipSlot::Holster));
                if let Some(w) = w {
                    models::showcase(&mut self.viewmodel, w, self.time);
                    vm_proj = Some(showcase_proj);
                    ambient = 0.9;
                }
            }
            Screen::Hideout => {
                if let Some(w) = self.modding.as_ref().and_then(|m| m.item.weapon.as_ref()) {
                    models::showcase(&mut self.viewmodel, w, self.time);
                    vm_proj = Some(showcase_proj);
                    ambient = 0.9;
                } else if let Some(h) = &self.hideout {
                    draw_world = true;
                    ambient = 0.42;
                    let proj = crate::render::perspective(self.settings.fov_deg.to_radians(), aspect, 0.05, 200.0);
                    view_proj = proj * h.player.view_matrix();
                    cam_pos = h.player.eye_pos();
                    let overlay = h.stash_open || h.open_station.is_some();
                    if !overlay {
                        if let Some(hit) = h.target() {
                            block_outline(&mut self.dynamic, hit.pos);
                        }
                        // The selected block, held in hand.
                        let b = h.selected_block();
                        let bob = (h.player.walk_phase * 3.2).sin().abs()
                            * 0.015
                            * (h.player.horizontal_speed() / 4.0).min(1.0);
                        let m = Mat4::from_translation(Vec3::new(0.34, -0.32 - bob, -0.6))
                            * Mat4::from_rotation_y(0.6)
                            * Mat4::from_rotation_x(0.25)
                            * Mat4::from_scale(Vec3::splat(0.24));
                        self.viewmodel
                            .add_cube_tiled(m, [255, 255, 255, 255], b.info().emissive, b.info().tiles[1]);
                        vm_proj = Some(crate::render::perspective(62f32.to_radians(), aspect, 0.01, 10.0));
                    }
                }
            }
            Screen::Summary | Screen::Traders => {}
        }
        FrameScene {
            draw_world,
            view_proj,
            cam_pos,
            sun_dir: Vec3::new(0.35, 0.85, 0.25),
            sky_color: if draw_world {
                [0.58, 0.68, 0.78]
            } else {
                [0.15, 0.16, 0.17]
            },
            fog_start: 70.0,
            fog_end: 185.0,
            ambient_boost: ambient,
            time: self.time,
            dynamic: &self.dynamic,
            viewmodel: vm_proj.map(|p| (&self.viewmodel, p)),
        }
    }

    /// Save on exit. Quitting mid-raid counts as MIA.
    pub fn on_exit(&mut self) {
        if self.hideout.is_some() {
            self.leave_hideout();
        }
        self.close_modding();
        if self.screen == Screen::Stash {
            self.leave_stash();
        }
        if self.screen == Screen::Traders {
            self.leave_traders();
        }
        if let Some(raid) = self.raid.as_mut() {
            raid.abandon();
            self.end_raid();
        }
        self.save();
    }

    // ------------------------------------------------------------------
    // Debug helpers (command-line flags used for smoke tests/screenshots)
    // ------------------------------------------------------------------

    /// Place the camera at a fixed pose (debug / screenshots). y <= 0 means "on the ground".
    pub fn debug_camera(&mut self, c: [f32; 5]) {
        if let (Screen::Hideout, Some(h)) = (self.screen, self.hideout.as_mut()) {
            h.player.pos = Vec3::new(c[0], if c[1] <= 0.0 { 4.0 } else { c[1] }, c[2]);
            h.player.yaw = c[3];
            h.player.pitch = c[4];
            h.player.noclip = c[1] > 0.0;
            return;
        }
        if self.raid.is_none() {
            self.start_raid();
        }
        let Some(raid) = self.raid.as_mut() else { return };
        let ground = c[1] <= 0.0;
        let y = if ground {
            raid.world.top_solid_y(c[0].floor() as i32, c[2].floor() as i32) as f32 + 1.0
        } else {
            c[1]
        };
        raid.player.pos = Vec3::new(c[0], y, c[2]);
        raid.player.yaw = c[3];
        raid.player.pitch = c[4];
        raid.player.noclip = !ground;
        // Bring a few scavs in front of the camera for inspection.
        let fwd = raid.player.forward_flat();
        let right = raid.player.right_flat();
        for (k, s) in raid.scavs.iter_mut().take(3).enumerate() {
            let p = raid.player.pos + fwd * (6.0 + k as f32 * 3.0) + right * (k as f32 - 1.0) * 2.5;
            let y = raid.world.top_solid_y(p.x.floor() as i32, p.z.floor() as i32) + 1;
            s.pos = Vec3::new(p.x, y as f32, p.z);
            s.home = s.pos;
            let d = raid.player.pos - s.pos;
            s.yaw = (-d.x).atan2(-d.z);
        }
    }

    /// Debug: stock the stash with parts, fit a set of attachments and open the modding screen.
    pub fn debug_mod_demo(&mut self) {
        use crate::weapons::{swap_attachment, AttachmentId, Slot};
        for a in AttachmentId::ALL {
            let _ = self.profile.stash.insert(Item::new(ItemKind::Attachment(a)));
        }
        let Some(item) = self.profile.equipment.primary.take() else {
            return;
        };
        self.open_modding(item, Origin::Slot(EquipSlot::Primary));
        self.screen = Screen::Stash;
        if let Some(w) = self.modding.as_mut().and_then(|m| m.item.weapon.as_mut()) {
            let mut parts = GridParts {
                grid: &mut self.profile.stash,
                reserved: None,
            };
            for (slot, a) in [
                (Slot::Muzzle, AttachmentId::Pbs4Suppressor),
                (Slot::Sight, AttachmentId::Pso1Scope),
                (Slot::Magazine, AttachmentId::Rpk16Drum95),
                (Slot::Grip, AttachmentId::Rk2Grip),
                (Slot::Stock, AttachmentId::ZhukovStock),
            ] {
                let _ = swap_attachment(w, slot, Some(a), &mut parts);
            }
        }
    }

    /// Debug: open a screen directly.
    pub fn debug_screen(&mut self, name: &str) {
        match name {
            "stash" => self.screen = Screen::Stash,
            "traders" => self.screen = Screen::Traders,
            "sell" => {
                self.screen = Screen::Traders;
                self.traders_ui.tab = ui::traders::TraderTab::Sell;
                self.traders_ui.selected = crate::traders::TraderId::Therapist;
            }
            "tasks" => {
                self.screen = Screen::Traders;
                self.traders_ui.tab = ui::traders::TraderTab::Tasks;
                self.traders_ui.selected = crate::traders::TraderId::Mechanic;
            }
            "hideout" => self.enter_hideout(),
            "station" => {
                self.enter_hideout();
                if let Some(h) = self.hideout.as_mut() {
                    h.open_station = Some(crate::hideout::StationKind::Workbench);
                }
            }
            "raid" => self.start_raid(),
            "loot" => {
                self.start_raid();
                if let Some(raid) = self.raid.as_mut() {
                    if let Some((p, k)) = raid
                        .map
                        .containers
                        .iter()
                        .find(|(_, k)| *k == crate::world::ContainerKind::WeaponBox)
                        .copied()
                    {
                        raid.open(Interact::Container(p, k));
                        self.raid_inventory = true;
                    }
                }
            }
            "summary" => {
                self.last_outcome = Some(RaidOutcome {
                    kind: OutcomeKind::Survived("Road to Customs".into()),
                    time: 612.0,
                    kills: 3,
                    value_in: 180_000,
                    value_out: 342_000,
                    tasks: vec!["Debut: Scavs killed 3/5 (+3)".into()],
                });
                self.screen = Screen::Summary;
            }
            _ => {}
        }
    }
}

/// Thin dark edges around the targeted block.
fn block_outline(mb: &mut MeshBuilder, p: glam::IVec3) {
    let o = p.as_vec3() - Vec3::splat(0.004);
    let s = 1.008;
    let c = [16, 16, 16, 255];
    let corners = |x: f32, y: f32, z: f32| o + Vec3::new(x, y, z) * s;
    let edges = [
        ((0., 0., 0.), (1., 0., 0.)),
        ((0., 1., 0.), (1., 1., 0.)),
        ((0., 0., 1.), (1., 0., 1.)),
        ((0., 1., 1.), (1., 1., 1.)),
        ((0., 0., 0.), (0., 1., 0.)),
        ((1., 0., 0.), (1., 1., 0.)),
        ((0., 0., 1.), (0., 1., 1.)),
        ((1., 0., 1.), (1., 1., 1.)),
        ((0., 0., 0.), (0., 0., 1.)),
        ((1., 0., 0.), (1., 0., 1.)),
        ((0., 1., 0.), (0., 1., 1.)),
        ((1., 1., 0.), (1., 1., 1.)),
    ];
    for ((ax, ay, az), (bx, by, bz)) in edges {
        mb.add_beam(corners(ax, ay, az), corners(bx, by, bz), 0.018, c, true);
    }
}

fn build_raid_entities(mb: &mut MeshBuilder, raid: &Raid, time: f32) {
    let cam = raid.player.eye_pos();
    for s in &raid.scavs {
        if s.pos.distance(cam) > 190.0 {
            continue;
        }
        let dead = s.state == ScavState::Dead;
        let pose = HumanoidPose {
            feet: s.pos,
            yaw: s.yaw,
            crouch: s.crouch,
            walk_phase: s.walk_phase,
            moving: Vec3::new(s.vel.x, 0.0, s.vel.z).length() > 0.3,
            aiming: s.state == ScavState::Engage,
            aim_pitch: s.aim_pitch,
            dead,
            hit_flash: s.hit_flash > 0.0,
        };
        let look = HumanoidLook {
            jacket: s.look.jacket,
            pants: s.look.pants,
            skin: s.look.skin,
            hat: s.look.hat,
            helmet: s.helmet.is_some(),
            armor: s.armor.is_some(),
        };
        // Searched bodies no longer show the gun (it's in the loot grid).
        let weapon = if dead && s.searched { None } else { Some(&s.weapon) };
        models::humanoid(mb, &pose, &look, weapon);
    }
    // Extraction beacons: a green flare column and a ring on the ground.
    for e in &raid.extracts {
        let pulse = 0.75 + 0.25 * (time * 3.0).sin();
        let c = [(80.0 * pulse) as u8, (255.0 * pulse) as u8, (120.0 * pulse) as u8, 255];
        mb.add_beam(e.pos - Vec3::Y, e.pos + Vec3::Y * 40.0, 0.18, c, true);
        for k in 0..12 {
            let a = k as f32 / 12.0 * std::f32::consts::TAU + time * 0.3;
            let p = e.pos + Vec3::new(a.cos() * e.radius, 0.05, a.sin() * e.radius);
            mb.add_aabb(
                p - Vec3::new(0.12, 0.0, 0.12),
                p + Vec3::new(0.12, 0.08, 0.12),
                [90, 240, 120, 255],
                true,
            );
        }
    }
}

pub fn gather_move_input(input: &Input) -> MoveInput {
    let mut axis = Vec2::ZERO;
    if input.down(KeyCode::KeyW) {
        axis.y += 1.0;
    }
    if input.down(KeyCode::KeyS) {
        axis.y -= 1.0;
    }
    if input.down(KeyCode::KeyD) {
        axis.x += 1.0;
    }
    if input.down(KeyCode::KeyA) {
        axis.x -= 1.0;
    }
    MoveInput {
        axis,
        jump: input.down(KeyCode::Space),
        crouch: input.down(KeyCode::KeyC) || input.down(KeyCode::ControlLeft),
        sprint: input.down(KeyCode::ShiftLeft),
        speed_mult: 1.0,
    }
}
