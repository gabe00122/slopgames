//! Top-level game state: screens, simulation update and scene assembly.

use glam::{Vec2, Vec3};
use winit::keyboard::KeyCode;

use crate::ai::ScavState;
use crate::input::Input;
use crate::player::MoveInput;
use crate::raid::{PlayerKit, Raid, WeaponSlot};
use crate::weapons::PartsBin;
use crate::render::models::{self, HumanoidLook, HumanoidPose, ViewmodelParams};
use crate::render::{FrameScene, MeshBuilder, Renderer};
use crate::rng::Rng;
use crate::ui;

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

pub struct Game {
    pub raid: Option<Raid>,
    pub settings: Settings,
    pub paused: bool,
    pub quit_requested: bool,
    pub stats: FrameStats,
    pub adapter_info: String,
    dynamic: MeshBuilder,
    viewmodel: MeshBuilder,
    world_dirty_reload: bool,
    pub rng: Rng,
    pub time: f32,
    /// Smoothed mouse delta for weapon sway.
    sway: Vec2,
    /// Debug: keep the weapon aimed down sights (screenshots).
    pub debug_force_ads: bool,
    pub modding: Option<ModdingSession>,
    /// Loose attachments available to the modding screen.
    pub parts: PartsBin,
}

pub struct ModdingSession {
    pub slot: WeaponSlot,
    pub ui: ui::modding::ModdingUi,
}

impl Game {
    pub fn new(renderer: &mut Renderer, seed: Option<u64>) -> Self {
        let mut rng = Rng::from_time();
        let seed = seed.unwrap_or_else(|| rng.next_u64());
        let mut game = Self {
            raid: None,
            settings: Settings::default(),
            paused: false,
            quit_requested: false,
            stats: FrameStats::default(),
            adapter_info: renderer.adapter_info.clone(),
            dynamic: MeshBuilder::new(),
            viewmodel: MeshBuilder::new(),
            world_dirty_reload: false,
            rng,
            time: 0.0,
            sway: Vec2::ZERO,
            debug_force_ads: false,
            modding: None,
            parts: PartsBin::everything(),
        };
        game.start_raid(seed);
        game
    }

    pub fn start_raid(&mut self, seed: u64) {
        let t0 = std::time::Instant::now();
        self.raid = Some(Raid::new(seed, PlayerKit::default_kit()));
        log::info!(
            "Generated raid map (seed {seed}) in {:.1} ms",
            t0.elapsed().as_secs_f32() * 1000.0
        );
        self.world_dirty_reload = true;
        self.paused = false;
    }

    /// Should the OS cursor be captured for mouse-look?
    pub fn wants_cursor_grab(&self) -> bool {
        match &self.raid {
            Some(r) => !self.paused && self.modding.is_none() && r.dead.is_none(),
            None => false,
        }
    }

    pub fn update(&mut self, dt: f32, real_dt: f32, input: &Input, renderer: &mut Renderer) {
        self.time += dt;
        self.stats.tick(real_dt);
        self.stats.chunks_drawn = renderer.stats_chunks_drawn;
        self.stats.triangles = renderer.stats_triangles;

        if input.pressed(KeyCode::F3) {
            self.settings.show_debug = !self.settings.show_debug;
        }
        if input.pressed(KeyCode::Escape) {
            if self.modding.is_some() {
                self.modding = None;
            } else {
                self.paused = !self.paused;
            }
        }
        let target_sway = Vec2::new(-input.mouse_delta.x, input.mouse_delta.y) * 0.00035;
        self.sway += (target_sway.clamp(Vec2::splat(-0.03), Vec2::splat(0.03)) - self.sway) * (1.0 - (-10.0 * dt).exp());

        if let Some(raid) = self.raid.as_mut() {
            if self.world_dirty_reload {
                renderer.load_world(&mut raid.world);
                self.world_dirty_reload = false;
            }
            if !self.paused && self.modding.is_none() {
                raid.update(dt, input, &self.settings, true);
                if self.debug_force_ads {
                    raid.gun.ads = 1.0;
                }
            }
            renderer.sync_world(&mut raid.world);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if self.modding.is_some() {
            self.modding_ui(ui);
            return;
        }
        let mut restart = false;
        if let Some(raid) = &self.raid {
            ui::hud::draw_hud(ui, raid, &self.settings, &self.stats, &self.adapter_info);
            if raid.dead.is_some() && ui::hud::death_overlay(ui, raid) {
                restart = true;
            }
        }
        if restart {
            let seed = self.rng.next_u64();
            self.start_raid(seed);
        }
        if self.paused {
            match ui::menu::pause_menu(ui) {
                Some(ui::menu::PauseAction::Resume) => self.paused = false,
                Some(ui::menu::PauseAction::ModWeapons) => {
                    self.modding = Some(ModdingSession {
                        slot: WeaponSlot::Primary,
                        ui: Default::default(),
                    });
                }
                Some(ui::menu::PauseAction::Quit) => self.quit_requested = true,
                None => {}
            }
            egui::Window::new("Settings")
                .anchor(egui::Align2::RIGHT_TOP, [-20.0, 20.0])
                .resizable(false)
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    ui.add(
                        egui::Slider::new(&mut self.settings.sensitivity, 0.0005..=0.006)
                            .text("Mouse sensitivity"),
                    );
                    ui.add(egui::Slider::new(&mut self.settings.fov_deg, 55.0..=100.0).text("FOV"));
                    ui.checkbox(&mut self.settings.show_debug, "Debug overlay (F3)");
                });
        }
    }

    fn modding_ui(&mut self, ui: &mut egui::Ui) {
        let (Some(raid), Some(session)) = (self.raid.as_mut(), self.modding.as_mut()) else {
            return;
        };
        egui::Panel::top("mod_tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (slot, label) in [(WeaponSlot::Primary, "Primary"), (WeaponSlot::Holster, "Holster")] {
                    let name = match slot {
                        WeaponSlot::Primary => raid.kit.primary.as_ref(),
                        WeaponSlot::Holster => raid.kit.holster.as_ref(),
                    }
                    .map(|w| w.name())
                    .unwrap_or("empty");
                    if ui.selectable_label(session.slot == slot, format!("{label}: {name}")).clicked() {
                        session.slot = slot;
                    }
                }
            });
        });
        let weapon = match session.slot {
            WeaponSlot::Primary => raid.kit.primary.as_mut(),
            WeaponSlot::Holster => raid.kit.holster.as_mut(),
        };
        let Some(weapon) = weapon else {
            if ui.button("Back").clicked() {
                self.modding = None;
            }
            return;
        };
        let events = ui::modding::modding_screen(ui, &mut session.ui, weapon, &mut self.parts);
        for e in events {
            match e {
                ui::modding::ModdingEvent::Close => self.modding = None,
                ui::modding::ModdingEvent::Unloaded(ammo, n) => raid.kit.return_ammo(ammo, n),
            }
        }
    }

    pub fn build_scene(&mut self, aspect: f32) -> FrameScene<'_> {
        self.dynamic.clear();
        self.viewmodel.clear();
        let mut view_proj = glam::Mat4::IDENTITY;
        let mut cam_pos = Vec3::ZERO;
        let mut draw_world = false;
        let mut vm_proj = None;
        let mut ambient = 0.0;
        if let (Some(raid), Some(session)) = (&self.raid, &self.modding) {
            let w = match session.slot {
                WeaponSlot::Primary => raid.kit.primary.as_ref(),
                WeaponSlot::Holster => raid.kit.holster.as_ref(),
            };
            if let Some(w) = w {
                models::showcase(&mut self.viewmodel, w, self.time);
                vm_proj = Some(crate::render::perspective(42f32.to_radians(), aspect, 0.05, 10.0));
                ambient = 0.9;
            }
        } else if let Some(raid) = &self.raid {
            draw_world = true;
            let fov = (self.settings.fov_deg / raid.zoom()).to_radians();
            let proj = crate::render::perspective(fov, aspect, 0.05, 400.0);
            view_proj = proj * raid.player.view_matrix();
            cam_pos = raid.player.eye_pos();
            build_raid_entities(&mut self.dynamic, raid);
            raid.effects.build(&mut self.dynamic);
            if raid.dead.is_none() && !raid.is_scoped() {
                if let Some(w) = raid.weapon() {
                    let g = &raid.gun;
                    let params = ViewmodelParams {
                        ads: g.ads,
                        kick: g.kick,
                        reload: g.reload.map(|r| 1.0 - r.remaining / r.total.max(0.01)),
                        draw: g.draw,
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
        FrameScene {
            draw_world,
            view_proj,
            cam_pos,
            sun_dir: Vec3::new(0.35, 0.85, 0.25),
            sky_color: if draw_world { [0.58, 0.68, 0.78] } else { [0.15, 0.16, 0.17] },
            fog_start: 70.0,
            fog_end: 185.0,
            ambient_boost: ambient,
            time: self.time,
            dynamic: &self.dynamic,
            viewmodel: vm_proj.map(|p| (&self.viewmodel, p)),
        }
    }

    pub fn on_exit(&mut self) {}

    /// Debug: fit a set of attachments and open the modding screen.
    pub fn debug_mod_demo(&mut self) {
        use crate::weapons::{swap_attachment, AttachmentId, Slot};
        if let Some(w) = self.raid.as_mut().and_then(|r| r.kit.primary.as_mut()) {
            for (slot, a) in [
                (Slot::Muzzle, AttachmentId::Pbs4Suppressor),
                (Slot::Sight, AttachmentId::Pso1Scope),
                (Slot::Magazine, AttachmentId::Rpk16Drum95),
                (Slot::Grip, AttachmentId::Rk2Grip),
                (Slot::Stock, AttachmentId::ZhukovStock),
            ] {
                let _ = swap_attachment(w, slot, Some(a), &mut self.parts);
            }
        }
        self.modding = Some(ModdingSession {
            slot: WeaponSlot::Primary,
            ui: Default::default(),
        });
    }

    /// Place the camera at a fixed pose with noclip (debug / screenshots).
    pub fn debug_camera(&mut self, c: [f32; 5]) {
        if let Some(raid) = self.raid.as_mut() {
            // A non-positive y means "stand on the ground here" (normal physics).
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
    }
}

fn build_raid_entities(mb: &mut MeshBuilder, raid: &Raid) {
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
        models::humanoid(mb, &pose, &look, Some(&s.weapon));
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
