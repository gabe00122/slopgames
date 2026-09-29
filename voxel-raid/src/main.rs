//! Voxel Raid — a Minecraft / Tarkov hybrid.
//!
//! Entry point: window + event loop (winit), renderer (wgpu), UI (egui).

mod ai;
mod effects;
mod game;
mod hideout;
mod input;
mod inventory;
mod player;
mod quests;
mod raid;
mod render;
mod rng;
mod save;
mod traders;
mod ui;
mod weapons;
mod world;

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{CursorGrabMode, Window, WindowId};

use game::Game;
use input::Input;
use render::{EguiFrame, Renderer};

#[derive(Default, Clone)]
struct Args {
    /// Exit automatically after this many frames (used for smoke tests).
    smoke_frames: Option<u64>,
    seed: Option<u64>,
    /// Save a PNG of the last smoke-test frame.
    screenshot: Option<String>,
    /// Debug camera pose "x,y,z,yaw,pitch" (enables noclip).
    cam: Option<[f32; 5]>,
    force_ads: bool,
    mod_demo: bool,
    /// Open a screen directly: stash, raid, loot, summary.
    screen: Option<String>,
    /// Override the save file location.
    save: Option<String>,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--smoke-frames" => args.smoke_frames = it.next().and_then(|v| v.parse().ok()),
            "--seed" => args.seed = it.next().and_then(|v| v.parse().ok()),
            "--screenshot" => args.screenshot = it.next(),
            "--force-ads" => args.force_ads = true,
            "--mod-demo" => args.mod_demo = true,
            "--screen" => args.screen = it.next(),
            "--save" => args.save = it.next(),
            "--cam" => {
                let v: Vec<f32> = it
                    .next()
                    .unwrap_or_default()
                    .split(',')
                    .filter_map(|x| x.trim().parse().ok())
                    .collect();
                if v.len() == 5 {
                    args.cam = Some([v[0], v[1], v[2], v[3], v[4]]);
                }
            }
            "--help" | "-h" => {
                println!(
                    "voxel-raid [--save FILE] [--seed N]\n\
                     debug: [--smoke-frames N] [--screenshot out.png] [--screen stash|raid|loot|summary|hideout|station|traders|tasks]\n\
                     \x20      [--cam x,y,z,yaw,pitch] [--force-ads] [--mod-demo]"
                );
                std::process::exit(0);
            }
            other => eprintln!("ignoring unknown argument {other}"),
        }
    }
    args
}

struct Running {
    window: Arc<Window>,
    renderer: Renderer,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    input: Input,
    game: Game,
    last_frame: Instant,
    grabbed: bool,
    frames: u64,
    frame_time_acc: f64,
}

struct App {
    args: Args,
    state: Option<Running>,
}

impl Running {
    fn set_grab(&mut self, grab: bool) {
        if grab {
            let ok = self
                .window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Confined))
                .is_ok();
            if !ok {
                log::warn!("cursor grab not available");
            }
            self.window.set_cursor_visible(false);
        } else {
            let _ = self.window.set_cursor_grab(CursorGrabMode::None);
            self.window.set_cursor_visible(true);
        }
        self.grabbed = grab;
        self.input.reset();
    }

    fn frame(&mut self) {
        let now = Instant::now();
        let real_dt = (now - self.last_frame).as_secs_f32();
        self.frame_time_acc += real_dt as f64;
        let dt = real_dt.min(0.05);
        self.last_frame = now;

        self.game.update(dt, real_dt, &self.input, &mut self.renderer);

        let raw_input = self.egui_state.take_egui_input(&self.window);
        let game = &mut self.game;
        let full_output = self.egui_ctx.run_ui(raw_input, |ui| game.ui(ui));
        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);
        let primitives = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let aspect = self.renderer.aspect();
        let scene = self.game.build_scene(aspect);
        self.renderer.render(
            &scene,
            EguiFrame {
                primitives,
                textures_delta: full_output.textures_delta,
                pixels_per_point: full_output.pixels_per_point,
            },
        );

        self.input.end_frame();
        let want = self.game.wants_cursor_grab() && self.window.has_focus();
        if want != self.grabbed {
            self.set_grab(want);
        }
        self.frames += 1;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Voxel Raid")
            .with_inner_size(LogicalSize::new(1600.0, 900.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("Failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };
        let renderer = match pollster::block_on(Renderer::new(window.clone(), event_loop.owned_display_handle())) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to initialise renderer: {e}");
                event_loop.exit();
                return;
            }
        };
        let egui_ctx = egui::Context::default();
        ui::style::apply(&egui_ctx);
        let max_tex = renderer.device.limits().max_texture_dimension_2d as usize;
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &*window,
            Some(window.scale_factor() as f32),
            None,
            Some(max_tex),
        );
        let save_path = self
            .args
            .save
            .clone()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(save::save_path);
        let mut game = Game::new(&renderer, save_path);
        game.raid_seed = self.args.seed;
        game.debug_force_ads = self.args.force_ads;
        if self.args.mod_demo {
            game.debug_mod_demo();
        }
        if self.args.force_ads && self.args.mod_demo {
            // Look through the fitted scope in a raid instead of staying in the modding screen.
            game.debug_screen("raid");
        }
        if let Some(screen) = &self.args.screen {
            game.debug_screen(screen);
        }
        if let Some(c) = self.args.cam {
            game.debug_camera(c);
        }
        self.state = Some(Running {
            window,
            renderer,
            egui_ctx,
            egui_state,
            input: Input::default(),
            game,
            last_frame: Instant::now(),
            grabbed: false,
            frames: 0,
            frame_time_acc: 0.0,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(st) = self.state.as_mut() else { return };
        let _ = st.egui_state.on_window_event(&st.window, &event);
        match event {
            WindowEvent::CloseRequested => {
                st.game.on_exit();
                event_loop.exit();
            }
            WindowEvent::Resized(size) => st.renderer.resize(size.width, size.height),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    st.input.key_event(code, event.state.is_pressed(), event.repeat);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                st.input.mouse_button(button, state.is_pressed());
            }
            WindowEvent::MouseWheel { delta, .. } => {
                st.input.scroll += match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => (p.y / 40.0) as f32,
                };
            }
            WindowEvent::Focused(false) => {
                st.input.reset();
                if st.grabbed {
                    st.set_grab(false);
                }
            }
            WindowEvent::RedrawRequested => {
                if let (Some(n), Some(path)) = (self.args.smoke_frames, &self.args.screenshot) {
                    if st.frames + 1 == n {
                        st.renderer.screenshot_request = Some(path.into());
                    }
                }
                if st.input.pressed(winit::keyboard::KeyCode::F12) {
                    let name = format!(
                        "screenshot-{}.png",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0)
                    );
                    st.renderer.screenshot_request = Some(name.into());
                }
                st.frame();
                if st.game.quit_requested {
                    st.game.on_exit();
                    event_loop.exit();
                }
                if let Some(n) = self.args.smoke_frames {
                    if st.frames >= n {
                        println!(
                            "SMOKE OK: rendered {} frames, avg {:.2} ms/frame ({:.0} FPS) on {}",
                            st.frames,
                            st.frame_time_acc * 1000.0 / st.frames as f64,
                            st.frames as f64 / st.frame_time_acc.max(1e-6),
                            st.renderer.adapter_info
                        );
                        st.game.on_exit();
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let Some(st) = self.state.as_mut() {
            if let DeviceEvent::MouseMotion { delta } = event {
                if st.grabbed {
                    st.input.mouse_motion(delta.0, delta.1);
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(st) = &self.state {
            st.window.request_redraw();
        }
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn,voxel_raid=info")).init();
    let args = parse_args();
    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("Failed to create event loop: {e}");
            std::process::exit(1);
        }
    };
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App { args, state: None };
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("Event loop error: {e}");
    }
}
