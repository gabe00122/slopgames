//! Top-level game state: screens, simulation update and scene assembly.

use glam::{Mat4, Vec2, Vec3};
use winit::keyboard::KeyCode;

use crate::input::Input;
use crate::player::MoveInput;
use crate::raid::Raid;
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
        };
        game.start_raid(seed);
        game
    }

    pub fn start_raid(&mut self, seed: u64) {
        let t0 = std::time::Instant::now();
        self.raid = Some(Raid::new(seed));
        log::info!(
            "Generated raid map (seed {seed}) in {:.1} ms",
            t0.elapsed().as_secs_f32() * 1000.0
        );
        self.world_dirty_reload = true;
        self.paused = false;
    }

    /// Should the OS cursor be captured for mouse-look?
    pub fn wants_cursor_grab(&self) -> bool {
        self.raid.is_some() && !self.paused
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
            self.paused = !self.paused;
        }

        if let Some(raid) = self.raid.as_mut() {
            if self.world_dirty_reload {
                renderer.load_world(&mut raid.world);
                self.world_dirty_reload = false;
            }
            if !self.paused {
                raid.time += dt;
                let look = input.mouse_delta * self.settings.sensitivity;
                raid.player.look(-look.x, -look.y);
                if input.pressed(KeyCode::KeyN) {
                    raid.player.noclip = !raid.player.noclip;
                }
                let mv = gather_move_input(input);
                raid.player.update(&raid.world, &mv, dt);
            }
            renderer.sync_world(&mut raid.world);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if let Some(raid) = &self.raid {
            ui::hud::draw_hud(ui, raid, &self.settings, &self.stats, &self.adapter_info);
        }
        if self.paused {
            match ui::menu::pause_menu(ui) {
                Some(ui::menu::PauseAction::Resume) => self.paused = false,
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

    pub fn build_scene(&mut self, aspect: f32) -> FrameScene<'_> {
        self.dynamic.clear();
        self.viewmodel.clear();
        let fov = self.settings.fov_deg.to_radians();
        let (view_proj, cam_pos, draw_world) = match &self.raid {
            Some(raid) => {
                let proj = crate::render::perspective(fov, aspect, 0.05, 400.0);
                (proj * raid.player.view_matrix(), raid.player.eye_pos(), true)
            }
            None => (Mat4::IDENTITY, Vec3::ZERO, false),
        };
        FrameScene {
            draw_world,
            view_proj,
            cam_pos,
            sun_dir: Vec3::new(0.35, 0.85, 0.25),
            sky_color: [0.58, 0.68, 0.78],
            fog_start: 70.0,
            fog_end: 185.0,
            ambient_boost: 0.0,
            time: self.time,
            dynamic: &self.dynamic,
            viewmodel: None,
        }
    }

    pub fn on_exit(&mut self) {}

    /// Place the camera at a fixed pose with noclip (debug / screenshots).
    pub fn debug_camera(&mut self, c: [f32; 5]) {
        if let Some(raid) = self.raid.as_mut() {
            raid.player.pos = Vec3::new(c[0], c[1], c[2]);
            raid.player.yaw = c[3];
            raid.player.pitch = c[4];
            raid.player.noclip = true;
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
