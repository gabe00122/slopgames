//! HELLCAST — a raycasting Doom-like built on Rust + wgpu + bytemuck.
//!
//! Run `cargo run --release`. See README.md for controls and command-line
//! options (headless screenshots, asset dumps, level select).

mod font;
mod game;
mod hud;
mod levels;
mod map;
mod math;
mod png;
mod render;
mod sprites;
mod textures;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

use game::{Game, Input, Phase};
use math::V2;
use render::Renderer;
use sprites::Sprites;

const DEFAULT_SENSITIVITY: f32 = 0.0022;

#[derive(Default)]
struct Args {
    dump_assets: Option<String>,
    shot: Option<String>,
    size: (u32, u32),
    level: Option<usize>,
    pos: Option<(f32, f32, f32)>,
    frames: u32,
    fire: bool,
    forward: bool,
    god: bool,
    automap: bool,
    title: bool,
    arsenal: bool,
    trace: bool,
    use_key: bool,
    novsync: bool,
    weapon: Option<usize>,
    res: Option<usize>,
}

fn parse_args() -> Args {
    let mut a = Args { size: (1280, 800), ..Default::default() };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| panic!("missing value for {arg}"));
        match arg.as_str() {
            "--dump-assets" => a.dump_assets = Some(val()),
            "--shot" => a.shot = Some(val()),
            "--size" => {
                let v = val();
                let (w, h) = v.split_once('x').expect("--size WxH");
                a.size = (w.parse().unwrap(), h.parse().unwrap());
            }
            "--level" => a.level = Some(val().parse::<usize>().expect("--level N").saturating_sub(1)),
            "--pos" => {
                let v: Vec<f32> = val().split(',').map(|s| s.parse().expect("--pos x,y,angle_degrees")).collect();
                a.pos = Some((v[0], v[1], v.get(2).copied().unwrap_or(0.0).to_radians()));
            }
            "--frames" => a.frames = val().parse().unwrap(),
            "--weapon" => a.weapon = Some(val().parse::<usize>().unwrap().saturating_sub(1)),
            "--res" => a.res = Some(val().parse().unwrap()),
            "--fire" => a.fire = true,
            "--forward" => a.forward = true,
            "--god" => a.god = true,
            "--automap" => a.automap = true,
            "--title" => a.title = true,
            "--arsenal" => a.arsenal = true,
            "--trace" => a.trace = true,
            "--use" => a.use_key = true,
            "--novsync" => a.novsync = true,
            "-h" | "--help" => {
                println!(
                    "hellcast [--level N] [--god] [--res 0-3] [--novsync]\n\
                     \x20 --shot FILE.png [--size WxH] [--level N] [--pos x,y,deg] [--frames N]\n\
                     \x20                 [--fire] [--forward] [--arsenal] [--weapon N] [--automap] [--title] [--trace]\n\
                     \x20 --dump-assets DIR"
                );
                std::process::exit(0);
            }
            other => panic!("unknown argument {other} (try --help)"),
        }
    }
    a
}

fn setup_game(args: &Args) -> Game {
    let mut game = Game::new(args.level.unwrap_or(0));
    if args.level.is_some() || (args.shot.is_some() && !args.title) {
        game.start_new_game(args.level.unwrap_or(0));
    }
    game.god = args.god;
    if let Some((x, y, a)) = args.pos {
        game.player.pos = V2::new(x, y);
        game.player.angle = a;
    }
    if args.arsenal {
        game.player.owned = [true; 4];
        game.player.ammo = [200, 50, 50];
    }
    if let Some(w) = args.weapon {
        game.player.weapon = game::Weapon::ALL[w.min(3)];
    }
    game.show_automap = args.automap;
    game
}

struct Controls {
    held: HashSet<KeyCode>,
    pressed: Vec<KeyCode>,
    mouse_dx: f64,
    lmb: bool,
    clicked: bool,
    wheel: i32,
    sensitivity: f32,
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            held: HashSet::new(),
            pressed: Vec::new(),
            mouse_dx: 0.0,
            lmb: false,
            clicked: false,
            wheel: 0,
            sensitivity: DEFAULT_SENSITIVITY,
        }
    }
}

impl Controls {
    fn frame_input(&mut self) -> Input {
        let k = |c: KeyCode| self.held.contains(&c);
        let p = |c: KeyCode| self.pressed.contains(&c);
        let axis = |pos: bool, neg: bool| pos as i32 as f32 - neg as i32 as f32;
        let use_ = p(KeyCode::KeyE) || p(KeyCode::Space);
        let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
        let input = Input {
            forward: axis(k(KeyCode::KeyW) || k(KeyCode::ArrowUp), k(KeyCode::KeyS) || k(KeyCode::ArrowDown)),
            strafe: axis(k(KeyCode::KeyD), k(KeyCode::KeyA)),
            turn: axis(k(KeyCode::ArrowRight), k(KeyCode::ArrowLeft)),
            mouse_dx: self.mouse_dx as f32 * self.sensitivity,
            run: k(KeyCode::ShiftLeft) || k(KeyCode::ShiftRight),
            fire: self.lmb || k(KeyCode::ControlLeft) || k(KeyCode::ControlRight),
            use_,
            confirm: use_ || p(KeyCode::Enter) || p(KeyCode::NumpadEnter) || self.clicked,
            automap: p(KeyCode::Tab),
            weapon: digits.iter().position(|&d| p(d)),
            cycle: self.wheel.signum(),
        };
        self.pressed.clear();
        self.mouse_dx = 0.0;
        self.clicked = false;
        self.wheel = 0;
        input
    }
}

struct State {
    window: Arc<Window>,
    renderer: Renderer,
    game: Game,
    sprites: Sprites,
    controls: Controls,
    last: Instant,
    captured: bool,
    fps_t: f32,
    fps_frames: u32,
}

impl State {
    fn set_capture(&mut self, on: bool) {
        if on {
            let grab = self
                .window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Confined));
            if let Err(e) = grab {
                eprintln!("hellcast: cursor grab failed: {e}");
            }
            self.window.set_cursor_visible(false);
        } else {
            let _ = self.window.set_cursor_grab(CursorGrabMode::None);
            self.window.set_cursor_visible(true);
        }
        self.captured = on;
    }

    fn frame(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32();
        self.last = now;
        let input = self.controls.frame_input();
        self.game.update(dt, &input);

        let view = self.renderer.view(self.game.shows_statusbar());
        let frame = self.game.build_frame(&view, &self.sprites);
        if self.game.map_dirty {
            self.renderer.upload_map(&self.game.map);
            self.game.map_dirty = false;
        }
        self.renderer.render(&frame, &self.game.map, &view);

        self.fps_t += dt;
        self.fps_frames += 1;
        if self.fps_t >= 1.0 {
            self.window.set_title(&format!("HELLCAST - {} fps", self.fps_frames));
            self.fps_t = 0.0;
            self.fps_frames = 0;
        }
    }
}

struct App {
    args: Args,
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("HELLCAST")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        let window = Arc::new(el.create_window(attrs).expect("create window"));
        let sprites = sprites::build();
        let mut renderer = pollster::block_on(Renderer::new_windowed(window.clone(), &sprites, !self.args.novsync));
        if let Some(r) = self.args.res {
            renderer.set_resolution(r);
        }
        let game = setup_game(&self.args);
        self.state = Some(State {
            window,
            renderer,
            game,
            sprites,
            controls: Controls::default(),
            last: Instant::now(),
            captured: false,
            fps_t: 0.0,
            fps_frames: 0,
        });
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(st) = self.state.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => st.renderer.resize(s.width, s.height),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                if event.state == ElementState::Released {
                    st.controls.held.remove(&code);
                    return;
                }
                st.controls.held.insert(code);
                if event.repeat {
                    return;
                }
                st.controls.pressed.push(code);
                match code {
                    KeyCode::Escape => {
                        if st.game.phase == Phase::Title || st.game.paused {
                            el.exit();
                        } else {
                            if st.game.phase == Phase::Playing {
                                st.game.paused = true;
                            }
                            st.set_capture(false);
                        }
                    }
                    KeyCode::F2 => {
                        let h = st.renderer.cycle_resolution();
                        st.game.msg(format!("Resolution: {h} lines"));
                    }
                    KeyCode::BracketLeft | KeyCode::BracketRight => {
                        let k = if code == KeyCode::BracketLeft { 1.0 / 1.15 } else { 1.15 };
                        st.controls.sensitivity = (st.controls.sensitivity * k).clamp(0.0003, 0.02);
                        st.game.msg(format!("Mouse sensitivity: {:.0}%", st.controls.sensitivity / DEFAULT_SENSITIVITY * 100.0));
                    }
                    KeyCode::F11 => {
                        let fs = if st.window.fullscreen().is_some() { None } else { Some(Fullscreen::Borderless(None)) };
                        st.window.set_fullscreen(fs);
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let down = state == ElementState::Pressed;
                if down && !st.captured {
                    // First click grabs the mouse (and resumes) without firing.
                    st.set_capture(true);
                    st.game.paused = false;
                    st.controls.clicked = st.game.phase != Phase::Playing;
                } else {
                    st.controls.lmb = down;
                    st.controls.clicked |= down;
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 30.0,
                };
                if y.abs() > 0.0 {
                    st.controls.wheel += if y > 0.0 { 1 } else { -1 };
                }
            }
            WindowEvent::Focused(false) => {
                if st.captured {
                    st.set_capture(false);
                    if st.game.phase == Phase::Playing {
                        st.game.paused = true;
                    }
                }
                st.controls.held.clear();
                st.controls.lmb = false;
            }
            WindowEvent::RedrawRequested => st.frame(),
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let (Some(st), DeviceEvent::MouseMotion { delta }) = (self.state.as_mut(), event)
            && st.captured {
                st.controls.mouse_dx += delta.0;
            }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(st) = &self.state {
            st.window.request_redraw();
        }
    }
}

fn screenshot(args: &Args, path: &str) {
    let sprites = sprites::build();
    let mut renderer = pollster::block_on(Renderer::new_headless(args.size.0, args.size.1, &sprites));
    if let Some(r) = args.res {
        renderer.set_resolution(r);
    }
    let mut game = setup_game(args);
    let input = Input { fire: args.fire, forward: if args.forward { 1.0 } else { 0.0 }, ..Default::default() };
    for f in 0..args.frames {
        // `--use` presses the use key once, after the weapon is raised.
        let input = Input { use_: args.use_key && f == 12, ..input.clone() };
        game.update(1.0 / 60.0, &input);
        if args.trace && f % 60 == 59 {
            let p = &game.player;
            eprintln!("t={:5.1}s player ({:.1},{:.1}) hp {} armor {} ammo {:?}", game.time, p.pos.x, p.pos.y, p.health, p.armor, p.ammo);
            for m in &game.monsters {
                eprintln!("    {:?} {:?} ({:.1},{:.1}) hp {}", m.kind, m.ai, m.pos.x, m.pos.y, m.hp);
            }
            for (msg, _) in &game.messages {
                eprintln!("    msg: {msg}");
            }
        }
    }
    let view = renderer.view(game.shows_statusbar());
    let frame = game.build_frame(&view, &sprites);
    renderer.upload_map(&game.map);
    renderer.render(&frame, &game.map, &view);
    let (w, h, px) = renderer.read_pixels().expect("offscreen target");
    png::write(path, w, h, &px).expect("write png");
    eprintln!(
        "wrote {path} ({w}x{h}); player {:.2},{:.2} hp {} | monsters alive {}",
        game.player.pos.x,
        game.player.pos.y,
        game.player.health,
        game.monsters.iter().filter(|m| !matches!(m.ai, game::Ai::Dying | game::Ai::Dead)).count()
    );
}

fn dump_assets(dir: &str) {
    use textures::Image;
    let sheet = |imgs: &[Image], cols: usize, zoom: usize, ignore_alpha: bool| {
        let (cw, ch) = (imgs[0].w * zoom + 2, imgs[0].h * zoom + 2);
        let rows = imgs.len().div_ceil(cols);
        let mut out = Image::new(cols * cw, rows * ch);
        for (i, img) in imgs.iter().enumerate() {
            let (ox, oy) = ((i % cols) * cw + 1, (i / cols) * ch + 1);
            for y in 0..img.h * zoom {
                for x in 0..img.w * zoom {
                    let p = img.get(x / zoom, y / zoom);
                    let c = if ignore_alpha || p[3] >= 0.5 { [p[0], p[1], p[2], 1.0] } else { [0.3, 0.25, 0.3, 1.0] };
                    out.set(ox + x, oy + y, c);
                }
            }
        }
        out
    };
    std::fs::create_dir_all(dir).expect("create dir");
    let save = |name: &str, img: Image| {
        let path = format!("{dir}/{name}");
        png::write(&path, img.w as u32, img.h as u32, &img.to_rgba8()).expect("write png");
        eprintln!("wrote {path}");
    };
    save("surfaces.png", sheet(&textures::build_surfaces(), 8, 2, true));
    save("sky.png", sheet(&[textures::build_sky()], 1, 2, true));
    save("sprites.png", sheet(&sprites::build().layers, 12, 1, false));
}

fn main() {
    let args = parse_args();
    if let Some(dir) = &args.dump_assets {
        dump_assets(dir);
        return;
    }
    if let Some(path) = &args.shot {
        screenshot(&args, path);
        return;
    }
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App { args, state: None };
    event_loop.run_app(&mut app).expect("run app");
}
