//! The title screen (over a drifting view of the swamp), the pause screen,
//! and the end-of-night screen.

use crate::{
    audio::{Sfx, Sound},
    game::*,
    hud::{AMBER, BILE, BLOOD, DIM, INK, WATER, font, text},
    util::{format_time, hex, hexa},
};
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct Menu {
    pub index: usize,
    pub help: bool,
    pub paused: bool,
}

const ITEMS: usize = 3;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum MenuText {
    Item(usize),
    Help,
}

#[derive(Component)]
pub struct TitleRoot;

#[derive(Component)]
pub struct OverRoot;

#[derive(Component)]
pub struct PauseRoot;

fn overlay(alpha: f32) -> impl Bundle {
    (
        Node {
            width: percent(100),
            height: percent(100),
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::VMin(1.6),
            ..default()
        },
        BackgroundColor(hexa(0x030605, alpha)),
    )
}

pub const HELP: &str = "\
You are the thing in the swamp. The hunters followed you in.\n\
\n\
In the water you are fast and nearly invisible. On land you are slow,\n\
you dry out, and every lantern finds you.\n\
\n\
Strike a hunter who hasn't seen you (or burst up out of the water under\n\
one) and it's over in one blow. Grab one and hold them under. Throw\n\
them, or a barrel, at their friends.\n\
\n\
Hang off a boat's side and pull down to roll it over. Ram it from below.\n\
Claw a rope bridge to drop whoever's on it. Put out lanterns: dunk them,\n\
or smash them.\n\
\n\
Lightning shows you to everyone who's looking. Stay under when it flashes.\n\
Reeds hide you. Mushrooms bounce you.";

pub fn title_enter(mut commands: Commands, mut menu: ResMut<Menu>) {
    menu.index = 0;
    menu.help = false;
    commands.spawn((overlay(0.35), TitleRoot)).with_children(|c| {
        c.spawn((text("BOGWIGHT", 13.0, BILE), TextShadow::default()));
        c.spawn(text(
            "the monster hunters followed you into the swamp. that was their mistake.",
            2.3,
            DIM,
        ));
        c.spawn(Node {
            height: Val::VMin(4.0),
            ..default()
        });
        for i in 0..ITEMS {
            c.spawn((text("", 3.4, INK), MenuText::Item(i)));
        }
        c.spawn(Node {
            height: Val::VMin(3.0),
            ..default()
        });
        c.spawn(text(
            "keys: WASD   Space   J   K        pad: stick   A   X   B        F11 fullscreen   M mute",
            1.8,
            DIM,
        ));
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::all(Val::VMin(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(hexa(0x050a07, 0.94)),
            BorderColor::all(hex(0x3a4a38)),
            Visibility::Hidden,
            MenuText::Help,
        ))
        .with_child((text(HELP, 2.2, INK), TextLayout::justify(Justify::Left)));
    });
}

pub fn despawn<T: Component>(mut commands: Commands, q: Query<Entity, With<T>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Menu directions from the keyboard and every gamepad (edges only).
fn nav(keys: &ButtonInput<KeyCode>, pads: &Query<&Gamepad>, held: &mut i32) -> (i32, bool, bool) {
    let mut dy = 0;
    if keys.any_just_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dy = -1;
    }
    if keys.any_just_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dy = 1;
    }
    let mut go = keys.any_just_pressed([KeyCode::Enter, KeyCode::Space, KeyCode::NumpadEnter]);
    let mut back = keys.just_pressed(KeyCode::Escape);
    let mut stick = 0;
    for pad in pads {
        if pad.just_pressed(GamepadButton::DPadUp) {
            dy = -1;
        }
        if pad.just_pressed(GamepadButton::DPadDown) {
            dy = 1;
        }
        go |= pad.any_just_pressed([GamepadButton::South, GamepadButton::Start]);
        back |= pad.just_pressed(GamepadButton::East);
        let l = pad.left_stick();
        stick = (l.y < -0.6) as i32 - (l.y > 0.6) as i32;
    }
    if stick != *held && stick != 0 {
        dy = stick;
    }
    *held = stick;
    (dy, go, back)
}

pub fn title_update(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    real: Res<Time<Real>>,
    mut held: Local<i32>,
    mut menu: ResMut<Menu>,
    mut session: ResMut<Session>,
    mut next: ResMut<NextState<Screen>>,
    mut exit: MessageWriter<AppExit>,
    mut sfx: MessageWriter<Sfx>,
    mut texts: Query<(&MenuText, &mut Text, &mut TextColor)>,
    mut help: Query<(&MenuText, &mut Visibility), Without<Text>>,
) {
    let (dy, go, back) = nav(&keys, &pads, &mut held);
    if menu.help {
        if go || back || keys.just_pressed(KeyCode::KeyH) {
            menu.help = false;
            sfx.write(Sfx::ui(Sound::Back));
        }
    } else {
        if dy != 0 {
            menu.index = (menu.index as i32 + dy).rem_euclid(ITEMS as i32) as usize;
            sfx.write(Sfx::ui(Sound::Tick));
        }
        if go {
            match menu.index {
                0 => {
                    session.night = 1;
                    session.seed = (real.elapsed_secs_f64() * 1000.0) as u64 ^ 0x05ee_db09;
                    sfx.write(Sfx::ui(Sound::Confirm));
                    next.set(Screen::Playing);
                }
                1 => {
                    menu.help = true;
                    sfx.write(Sfx::ui(Sound::Confirm));
                }
                _ => {
                    exit.write(AppExit::Success);
                }
            }
        }
    }
    for (kind, mut t, mut c) in &mut texts {
        if let MenuText::Item(i) = *kind {
            let label = ["hunt the hunters", "how to play", "leave the swamp"][i];
            let sel = i == menu.index;
            let s = if sel { format!("> {label} <") } else { label.to_string() };
            if t.0 != s {
                t.0 = s;
            }
            c.0 = hex(if sel { BILE } else { 0x8a9a88 });
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

/// Esc or Start pauses; from the pause screen Q or Select quits to the title.
pub fn pause(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut menu: ResMut<Menu>,
    mut time: ResMut<Time<Virtual>>,
    mut next: ResMut<NextState<Screen>>,
    mut controls: ResMut<crate::input::Controls>,
    roots: Query<Entity, With<PauseRoot>>,
    mut sfx: MessageWriter<Sfx>,
) {
    let pad = |b: GamepadButton| pads.iter().any(|p| p.just_pressed(b));
    if keys.just_pressed(KeyCode::Escape) || pad(GamepadButton::Start) {
        menu.paused = !menu.paused;
        sfx.write(Sfx::ui(if menu.paused { Sound::Back } else { Sound::Confirm }));
        if menu.paused {
            time.pause();
            commands.spawn((overlay(0.6), PauseRoot)).with_children(|c| {
                c.spawn(text("PAUSED", 8.0, BILE));
                c.spawn(text(
                    "Esc / Start: back to the hunt      Q / Select: quit to title",
                    2.2,
                    DIM,
                ));
            });
        } else {
            time.unpause();
            controls.clear_presses();
            for e in &roots {
                commands.entity(e).despawn();
            }
        }
    }
    if menu.paused && (keys.just_pressed(KeyCode::KeyQ) || pad(GamepadButton::Select)) {
        menu.paused = false;
        time.unpause();
        for e in &roots {
            commands.entity(e).despawn();
        }
        next.set(Screen::Title);
    }
}

pub fn over_enter(mut commands: Commands, session: Res<Session>) {
    let won = session.outcome == Some(Outcome::Won);
    let s = session.stats;
    commands.spawn((overlay(0.55), OverRoot)).with_children(|c| {
        if won {
            c.spawn(text("THE SWAMP IS QUIET", 8.0, BILE));
            c.spawn(text(
                format!("night {} is over. none of them made it out.", session.night),
                2.4,
                DIM,
            ));
        } else {
            c.spawn(text("YOU WERE BAGGED", 8.0, BLOOD));
            c.spawn(text("your head will hang over a fireplace in town.", 2.4, DIM));
        }
        c.spawn(Node {
            height: Val::VMin(2.0),
            ..default()
        });
        let lines = [
            (format!("time in the swamp   {}", format_time(session.time)), INK),
            (
                format!(
                    "hunters taken       {} of {}",
                    session.hunters_total - session.hunters_left,
                    session.hunters_total
                ),
                INK,
            ),
            (format!("ambushes            {}", s.ambushes), BILE),
            (format!("drowned             {}", s.drowned), WATER),
            (format!("thrown              {}", s.thrown), AMBER),
            (format!("clawed              {}", s.clawed), INK),
            (format!("boats capsized      {}", s.boats_flipped), WATER),
            (format!("lanterns doused     {}", s.lanterns_doused), AMBER),
            (format!("times spotted       {}", s.spotted), BLOOD),
        ];
        c.spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            row_gap: Val::VMin(0.8),
            ..default()
        })
        .with_children(|c| {
            for (l, col) in lines {
                c.spawn((Text::new(l), font(2.3), TextColor(hex(col))));
            }
        });
        c.spawn(Node {
            height: Val::VMin(3.0),
            ..default()
        });
        c.spawn(text(
            if won {
                "Enter / A: the next night      Esc / B: title"
            } else {
                "Enter / A: try again      Esc / B: title"
            },
            2.4,
            INK,
        ));
    });
}

pub fn over_update(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    real: Res<Time<Real>>,
    mut held: Local<i32>,
    mut session: ResMut<Session>,
    mut next: ResMut<NextState<Screen>>,
    mut sfx: MessageWriter<Sfx>,
) {
    let (_, go, back) = nav(&keys, &pads, &mut held);
    if go {
        if session.outcome == Some(Outcome::Won) {
            session.night += 1;
            session.seed = session
                .seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407)
                ^ (real.elapsed_secs_f64() * 1000.0) as u64;
        }
        sfx.write(Sfx::ui(Sound::Confirm));
        next.set(Screen::Playing);
    } else if back {
        sfx.write(Sfx::ui(Sound::Back));
        next.set(Screen::Title);
    }
}
