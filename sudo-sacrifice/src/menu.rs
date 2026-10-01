//! Title screen, pause overlay and the end-of-session report.

use crate::{
    audio::{Sfx, Sound},
    game::{Difficulty, Match, Screen, Settings, Team},
    hud::{DIM, EDGE, GREEN, INK, PANEL, RED, YELLOW, font, text, vm},
    player::Control,
    util::{hex, hexa},
};
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct Menu {
    pub index: usize,
    pub help: bool,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum MenuText {
    Prompt,
    Item(usize),
    Help,
    Report,
}

const ITEMS: usize = 4;

const HELP: &str = "\
You are a coding agent. So is the other one.

SOFTWARE is what you sacrifice. Floppy disks of old code lie around the
island: walk over them to pick them up. Each subagent costs tokens plus
one or more pieces of software, burned on your altar.

TOKENS regenerate. Provision DATACENTERS on compute wells [F] to earn
faster, and stay near your buildings for a bonus.

When a subagent dies it drops its software. Yours floats back to you;
the enemy's must be hauled to your altar by a GARBAGE COLLECTOR.

To win, DEPRECATE the enemy's altar: stand near it with a Garbage
Collector at its base and press [F]. Survive 12 seconds.

Spells reshape the ground: FIREWALL raises walls, FORCE PUSH leaves
craters, FORK BOMB shatters everything into spikes.

  WASD move    SPACE jump    MOUSE look    LMB cast    1-0/WHEEL select
  RMB send subagents to a point    R regroup    F interact    M music
  [ ] mouse sensitivity    I invert look
  GAMEPAD  sticks move/look  RT cast  LT send  LB/RB select  A jump
           X interact  Y regroup  START pause";

fn difficulty_index(d: Difficulty) -> usize {
    Difficulty::ALL.iter().position(|x| *x == d).unwrap_or(1)
}

pub fn title_enter(mut commands: Commands, mut menu: ResMut<Menu>) {
    menu.help = false;
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                padding: UiRect::left(vm(9.0)),
                row_gap: vm(1.2),
                ..default()
            },
            BackgroundColor(hexa(0x05060a, 0.25)),
            DespawnOnExit(Screen::Title),
            GlobalZIndex(5),
        ))
        .with_children(|p| {
            p.spawn((
                text("", 7.0, INK),
                TextShadow {
                    offset: Vec2::splat(3.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.9),
                },
                MenuText::Prompt,
            ));
            p.spawn((
                text(
                    "coding agents sacrifice old software at their altars to summon subagents",
                    2.0,
                    0xc9d1d9,
                ),
                TextShadow {
                    offset: Vec2::splat(2.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.9),
                },
            ));
            p.spawn(Node {
                height: vm(2.5),
                ..default()
            });
            p.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: vm(0.8),
                    padding: UiRect::all(vm(1.8)),
                    width: vm(58.0),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(vm(0.8)),
                    ..default()
                },
                BackgroundColor(hexa(PANEL, 0.82)),
                BorderColor::all(hex(EDGE)),
            ))
            .with_children(|m| {
                for i in 0..ITEMS {
                    m.spawn((text("", 2.4, INK), MenuText::Item(i), Button));
                }
                m.spawn(Node {
                    height: vm(1.0),
                    ..default()
                });
                m.spawn(text("[W/S] select   [ENTER] confirm   [A/D] change", 1.5, DIM));
            });
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: vm(5.0),
                    top: percent(44),
                    width: vm(78.0),
                    padding: UiRect::all(vm(2.0)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(vm(0.8)),
                    ..default()
                },
                BackgroundColor(hexa(PANEL, 0.92)),
                BorderColor::all(hex(EDGE)),
                Visibility::Hidden,
                MenuText::Help,
            ))
            .with_child((text(HELP, 1.65, 0xc9d1d9), TextLayout::no_wrap()));
        });
}

/// Menu directions from every gamepad: d-pad, or the left stick pushed past
/// halfway (edges only).
fn pad_nav(pads: &Query<&Gamepad>, held: &mut (i32, i32)) -> (i32, i32, bool, bool) {
    let (mut dx, mut dy, mut go, mut back) = (0, 0, false, false);
    let mut stick = (0, 0);
    for pad in pads {
        if pad.just_pressed(GamepadButton::DPadUp) {
            dy = -1;
        }
        if pad.just_pressed(GamepadButton::DPadDown) {
            dy = 1;
        }
        if pad.just_pressed(GamepadButton::DPadLeft) {
            dx = -1;
        }
        if pad.just_pressed(GamepadButton::DPadRight) {
            dx = 1;
        }
        go |= pad.any_just_pressed([GamepadButton::South, GamepadButton::Start]);
        back |= pad.just_pressed(GamepadButton::East);
        let l = pad.left_stick();
        stick = (
            (l.x > 0.6) as i32 - (l.x < -0.6) as i32,
            (l.y < -0.6) as i32 - (l.y > 0.6) as i32,
        );
    }
    if stick.0 != held.0 && stick.0 != 0 {
        dx = stick.0;
    }
    if stick.1 != held.1 && stick.1 != 0 {
        dy = stick.1;
    }
    *held = stick;
    (dx, dy, go, back)
}

pub fn title_update(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut held: Local<(i32, i32)>,
    time: Res<Time<Real>>,
    mut menu: ResMut<Menu>,
    mut settings: ResMut<Settings>,
    mut next: ResMut<NextState<Screen>>,
    mut exit: MessageWriter<AppExit>,
    mut sfx: MessageWriter<Sfx>,
    mut texts: Query<(&MenuText, &mut Text, &mut TextColor)>,
    mut help: Query<(&MenuText, &mut Visibility), Without<Text>>,
    clicks: Query<(&MenuText, &Interaction), Changed<Interaction>>,
) {
    let (dx, dy, pad_go, pad_back) = pad_nav(&pads, &mut held);
    let up = keys.any_just_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) || dy < 0;
    let down = keys.any_just_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) || dy > 0;
    let left = keys.any_just_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) || dx < 0;
    let right = keys.any_just_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) || dx > 0;
    let mut go = keys.any_just_pressed([KeyCode::Enter, KeyCode::Space, KeyCode::NumpadEnter]) || pad_go;
    // The mouse: hovering picks an item, clicking chooses it.
    for (kind, interaction) in &clicks {
        let MenuText::Item(i) = *kind else { continue };
        match interaction {
            Interaction::Hovered if menu.index != i && !menu.help => {
                menu.index = i;
                sfx.write(Sfx::ui(Sound::Tick));
            }
            Interaction::Pressed => {
                menu.index = i;
                go = true;
            }
            _ => {}
        }
    }
    if menu.help {
        if go || pad_back || keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyH) {
            menu.help = false;
            sfx.write(Sfx::ui(Sound::Back));
        }
    } else {
        if up {
            menu.index = (menu.index + ITEMS - 1) % ITEMS;
            sfx.write(Sfx::ui(Sound::Tick));
        }
        if down {
            menu.index = (menu.index + 1) % ITEMS;
            sfx.write(Sfx::ui(Sound::Tick));
        }
        if (left || right) && menu.index == 1 {
            let i = difficulty_index(settings.difficulty);
            let j = if right { (i + 1) % 3 } else { (i + 2) % 3 };
            settings.difficulty = Difficulty::ALL[j];
            sfx.write(Sfx::ui(Sound::Tick));
        }
        if keys.just_pressed(KeyCode::KeyH) {
            menu.help = true;
        }
        if go {
            match menu.index {
                0 => {
                    settings.seed = settings
                        .seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407)
                        ^ (time.elapsed_secs_f64() * 1000.0) as u64;
                    sfx.write(Sfx::ui(Sound::Confirm));
                    next.set(Screen::Playing);
                }
                1 => {
                    let i = difficulty_index(settings.difficulty);
                    settings.difficulty = Difficulty::ALL[(i + 1) % 3];
                    sfx.write(Sfx::ui(Sound::Tick));
                }
                2 => {
                    menu.help = true;
                    sfx.write(Sfx::ui(Sound::Confirm));
                }
                _ => {
                    exit.write(AppExit::Success);
                }
            }
        }
    }
    let blink = (time.elapsed_secs() * 1.6).fract() < 0.55;
    for (kind, mut t, mut c) in &mut texts {
        match *kind {
            MenuText::Prompt => {
                let s = format!("$ sudo sacrifice{}", if blink { "_" } else { " " });
                if t.0 != s {
                    t.0 = s;
                }
            }
            MenuText::Item(i) => {
                let label = match i {
                    0 => "start session".to_string(),
                    1 => format!("rival: < {} >", settings.difficulty.name()),
                    2 => "how to play".to_string(),
                    _ => "exit".to_string(),
                };
                let sel = i == menu.index;
                let s = format!("{} {label}", if sel { ">" } else { " " });
                if t.0 != s {
                    t.0 = s;
                }
                c.0 = hex(if sel { GREEN } else { 0x9da7b3 });
            }
            _ => {}
        }
    }
    for (kind, mut vis) in &mut help {
        if *kind == MenuText::Help {
            vis.set_if_neq(if menu.help {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    }
}

/// Esc pauses a session; Q from the pause screen quits to the title.
pub fn pause(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut control: ResMut<Control>,
    mut time: ResMut<Time<Virtual>>,
    mut next: ResMut<NextState<Screen>>,
    mut commands: Commands,
    mut sfx: MessageWriter<Sfx>,
    overlay: Query<Entity, With<PauseOverlay>>,
) {
    let pad = |b: GamepadButton| pads.iter().any(|p| p.just_pressed(b));
    if keys.just_pressed(KeyCode::Escape) || pad(GamepadButton::Start) {
        control.paused = !control.paused;
        sfx.write(Sfx::ui(if control.paused { Sound::Back } else { Sound::Confirm }));
    }
    if control.paused && (keys.just_pressed(KeyCode::KeyQ) || pad(GamepadButton::Select)) {
        control.paused = false;
        next.set(Screen::Title);
    }
    if control.paused != time.is_paused() {
        if control.paused {
            time.pause();
        } else {
            time.unpause();
        }
    }
    let shown = !overlay.is_empty();
    if control.paused && !shown {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: vm(1.5),
                    ..default()
                },
                BackgroundColor(hexa(0x05060a, 0.6)),
                PauseOverlay,
                DespawnOnExit(Screen::Playing),
                GlobalZIndex(10),
            ))
            .with_children(|p| {
                p.spawn(text("^Z  session suspended", 4.0, INK));
                p.spawn(text(
                    "[ESC / START] resume     [Q / BACK] quit to title     [M] music",
                    2.0,
                    DIM,
                ));
            });
    } else if !control.paused && shown {
        for e in &overlay {
            commands.entity(e).despawn();
        }
    }
}

#[derive(Component)]
pub struct PauseOverlay;

pub fn over_enter(mut commands: Commands, game: Res<Match>, mut sfx: MessageWriter<Sfx>) {
    let won = game.winner == Some(Team::Blue);
    sfx.write(Sfx::ui(if won { Sound::Victory } else { Sound::Defeat }));
    let (title, sub, color) = if won {
        (
            "BUILD PASSED",
            format!(
                "{}'s altar has been deprecated. you ship on time.",
                Team::Red.agent_name()
            ),
            GREEN,
        )
    } else {
        (
            "BUILD FAILED",
            format!("your altar was deprecated by {}. rolling back.", Team::Red.agent_name()),
            RED,
        )
    };
    let s = &game.stats;
    let row = |label: &str, a: u32, b: u32| format!("{label:<22}{a:>8}{b:>14}");
    let report = [
        format!("{:<22}{:>8}{:>14}", "", "YOU", Team::Red.agent_name()),
        row("subagents summoned", s[0].summoned, s[1].summoned),
        row("subagents lost", s[0].lost, s[1].lost),
        row("spells cast", s[0].spells, s[1].spells),
        row("software sacrificed", s[0].sacrificed, s[1].sacrificed),
        row("datacenters built", s[0].datacenters, s[1].datacenters),
        row("compactions", s[0].compactions, s[1].compactions),
        format!("session length        {}", crate::util::format_time(game.time)),
    ]
    .join("\n");
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: vm(1.4),
                ..default()
            },
            BackgroundColor(hexa(0x05060a, 0.35)),
            DespawnOnExit(Screen::Over),
            GlobalZIndex(10),
        ))
        .with_children(|p| {
            p.spawn((
                text(title, 8.0, color),
                TextShadow {
                    offset: Vec2::splat(3.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.9),
                },
            ));
            p.spawn((
                text(sub, 2.2, INK),
                TextShadow {
                    offset: Vec2::splat(2.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.9),
                },
            ));
            p.spawn((
                Node {
                    padding: UiRect::all(vm(1.8)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(vm(0.8)),
                    margin: UiRect::top(vm(1.5)),
                    ..default()
                },
                BackgroundColor(hexa(PANEL, 0.88)),
                BorderColor::all(hex(EDGE)),
            ))
            .with_child((Text::new(report), font(1.8), TextColor(hex(0xc9d1d9)), MenuText::Report));
            p.spawn(text("[ENTER / CLICK / A] new session     [ESC / B] title", 2.0, YELLOW));
        });
}

pub fn over_update(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    mut next: ResMut<NextState<Screen>>,
    mut game: ResMut<Match>,
    real: Res<Time<Real>>,
) {
    game.over_for += real.delta_secs();
    if game.over_for < 1.0 {
        return;
    }
    let pad = |b: GamepadButton| pads.iter().any(|p| p.just_pressed(b));
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::Space, KeyCode::NumpadEnter])
        || mouse.just_pressed(MouseButton::Left)
        || pad(GamepadButton::South)
        || pad(GamepadButton::Start)
    {
        next.set(Screen::Playing);
    } else if keys.just_pressed(KeyCode::Escape) || pad(GamepadButton::East) {
        next.set(Screen::Title);
    }
}
