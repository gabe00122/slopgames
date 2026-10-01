//! The screens around the race: title, lobby, course select, results and the
//! pause overlay. All of them are driven by gamepad or keyboard.

use crate::{
    audio::{Sfx, Sound},
    beast::COURSES,
    course::LoadCourse,
    font::{Label, label_node},
    game::{CLASSES, COLORS, Cup, LAP_CHOICES, MAX_PLAYERS, MAX_RACERS, PlayerSlot, Results, Roster, Screen, Settings},
    input::Inputs,
    race::Race,
    util::{format_time, hex, ordinal},
};
use bevy::prelude::*;

const GOLD: Color = Color::srgb(1.0, 0.82, 0.2);
const SOFT: Color = Color::srgb(0.88, 0.93, 1.0);
const DIM: Color = Color::srgba(0.06, 0.05, 0.14, 0.62);

fn screen_root() -> Node {
    Node {
        width: percent(100),
        height: percent(100),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        padding: UiRect::all(Val::VMin(4.0)),
        ..default()
    }
}

fn band() -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::VMin(4.0), Val::VMin(2.0)),
            row_gap: Val::VMin(1.2),
            border_radius: BorderRadius::all(Val::VMin(3.0)),
            ..default()
        },
        BackgroundColor(DIM),
    )
}

/// Breaks `text` into lines of at most `width` characters.
fn wrap(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut line = 0;
    for word in text.split(' ') {
        if line > 0 && line + word.len() + 1 > width {
            out.push('\n');
            line = 0;
        } else if line > 0 {
            out.push(' ');
            line += 1;
        }
        out.push_str(word);
        line += word.len();
    }
    out
}

// ---------------------------------------------------------------------------
// Title
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct Blink;

pub fn title_enter(
    mut commands: Commands,
    mut load: MessageWriter<LoadCourse>,
    settings: Res<Settings>,
    args: Res<crate::Args>,
) {
    load.write(LoadCourse(settings.course));
    if args.pose.is_some() {
        // Debug camera: leave the view clear.
        return;
    }
    commands
        .spawn((screen_root(), DespawnOnExit(Screen::Title)))
        .with_children(|p| {
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::VMin(0.5),
                ..default()
            })
            .with_children(|c| {
                c.spawn((label_node(), Label::new("MONSTER RACE", 12.5, GOLD)));
                c.spawn((
                    label_node(),
                    Label::new("KART RACING ON THE BACKS OF GIANTS", 2.8, Color::WHITE),
                ));
            });
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::VMin(4.0)),
                row_gap: Val::VMin(1.5),
                ..default()
            })
            .with_children(|c| {
                c.spawn((label_node(), Label::new("PRESS A OR ENTER", 4.6, Color::WHITE), Blink));
                c.spawn((
                    label_node(),
                    Label::new("1-4 PLAYERS   GAMEPADS OR KEYBOARD", 2.2, SOFT),
                ));
            });
        });
}

pub fn title_update(
    time: Res<Time<Real>>,
    inputs: Res<Inputs>,
    mut next: ResMut<NextState<Screen>>,
    mut sfx: MessageWriter<Sfx>,
    mut blink: Query<&mut Visibility, With<Blink>>,
) {
    for mut vis in &mut blink {
        *vis = if (time.elapsed_secs() * 1.4).fract() < 0.7 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let nav = inputs.any_nav();
    if nav.confirm || nav.pause {
        sfx.write(Sfx(Sound::Confirm));
        next.set(Screen::Lobby);
    }
}

// ---------------------------------------------------------------------------
// Lobby
// ---------------------------------------------------------------------------

#[derive(Component)]
pub enum LobbyText {
    Title(usize),
    Device(usize),
    Color(usize),
    Status(usize),
    Footer,
}

#[derive(Component)]
pub struct LobbyCard(usize);

pub fn lobby_enter(mut commands: Commands, mut roster: ResMut<Roster>) {
    for p in &mut roster.players {
        p.ready = false;
    }
    commands
        .spawn((screen_root(), DespawnOnExit(Screen::Lobby)))
        .with_children(|p| {
            p.spawn(band()).with_children(|b| {
                b.spawn((label_node(), Label::new("WHO'S RACING?", 7.0, GOLD)));
            });
            p.spawn(Node {
                column_gap: Val::VMin(2.5),
                ..default()
            })
            .with_children(|row| {
                for i in 0..MAX_PLAYERS {
                    row.spawn((
                        Node {
                            width: Val::VMin(36.0),
                            height: Val::VMin(40.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceEvenly,
                            border: UiRect::all(Val::VMin(0.8)),
                            border_radius: BorderRadius::all(Val::VMin(3.0)),
                            ..default()
                        },
                        BackgroundColor(DIM),
                        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.25)),
                        LobbyCard(i),
                    ))
                    .with_children(|card| {
                        card.spawn((
                            label_node(),
                            Label::new(format!("P{}", i + 1), 8.0, Color::WHITE),
                            LobbyText::Title(i),
                        ));
                        card.spawn((label_node(), Label::new("", 2.0, SOFT), LobbyText::Device(i)));
                        card.spawn((label_node(), Label::new("", 3.4, Color::WHITE), LobbyText::Color(i)));
                        card.spawn((label_node(), Label::new("", 2.6, Color::WHITE), LobbyText::Status(i)));
                    });
                }
            });
            p.spawn(band()).with_children(|b| {
                b.spawn((label_node(), Label::new("", 2.6, Color::WHITE), LobbyText::Footer));
            });
        });
}

pub fn lobby_update(
    inputs: Res<Inputs>,
    mut roster: ResMut<Roster>,
    mut next: ResMut<NextState<Screen>>,
    mut sfx: MessageWriter<Sfx>,
    mut texts: Query<(&LobbyText, &mut Label)>,
    mut cards: Query<(&LobbyCard, &mut BorderColor)>,
) {
    let all_ready = !roster.players.is_empty() && roster.players.iter().all(|p| p.ready);
    for (device, nav) in inputs.nav.clone() {
        if !nav.any() {
            continue;
        }
        match roster.slot_of(device) {
            None => {
                if nav.confirm && roster.players.len() < MAX_PLAYERS {
                    let color = roster.free_color();
                    roster.players.push(PlayerSlot {
                        device,
                        color,
                        ready: false,
                    });
                    sfx.write(Sfx(Sound::Join));
                } else if nav.back && roster.players.is_empty() {
                    sfx.write(Sfx(Sound::Back));
                    next.set(Screen::Title);
                }
            }
            Some(slot) => {
                if nav.dx != 0 && !roster.players[slot].ready {
                    // Step to the next livery nobody else has.
                    let mut color = roster.players[slot].color;
                    for _ in 0..MAX_RACERS {
                        color = (color as i32 + nav.dx).rem_euclid(MAX_RACERS as i32) as usize;
                        if !roster.color_taken(color, slot) {
                            break;
                        }
                    }
                    roster.players[slot].color = color;
                    sfx.write(Sfx(Sound::Blip));
                }
                if nav.confirm || nav.pause {
                    if all_ready {
                        sfx.write(Sfx(Sound::Confirm));
                        next.set(Screen::Select);
                    } else if !roster.players[slot].ready {
                        roster.players[slot].ready = true;
                        sfx.write(Sfx(Sound::Confirm));
                    }
                }
                if nav.back {
                    sfx.write(Sfx(Sound::Back));
                    if roster.players[slot].ready {
                        roster.players[slot].ready = false;
                    } else {
                        roster.players.remove(slot);
                    }
                }
            }
        }
    }

    for (text, mut label) in &mut texts {
        match *text {
            LobbyText::Title(i) => {
                let color = roster
                    .players
                    .get(i)
                    .map_or(Color::srgba(1.0, 1.0, 1.0, 0.35), |p| hex(COLORS[p.color].1));
                Label::tint(&mut label, color);
            }
            LobbyText::Device(i) => Label::set(&mut label, roster.players.get(i).map_or("", |p| p.device.label())),
            LobbyText::Color(i) => match roster.players.get(i) {
                Some(p) => {
                    let name = COLORS[p.color].0;
                    Label::set(
                        &mut label,
                        if p.ready {
                            name.to_string()
                        } else {
                            format!("< {name} >")
                        },
                    );
                    Label::tint(&mut label, hex(COLORS[p.color].1));
                }
                None => Label::set(&mut label, ""),
            },
            LobbyText::Status(i) => match roster.players.get(i) {
                Some(p) if p.ready => {
                    Label::set(&mut label, "READY!");
                    Label::tint(&mut label, Color::srgb(0.45, 1.0, 0.5));
                }
                Some(_) => {
                    Label::set(&mut label, "PICK A COLOR\nTHEN PRESS A");
                    Label::tint(&mut label, Color::WHITE);
                }
                None => {
                    Label::set(&mut label, "PRESS A\nTO JOIN");
                    Label::tint(&mut label, SOFT);
                }
            },
            LobbyText::Footer => {
                let text = if all_ready {
                    "EVERYONE'S READY - PRESS A TO PICK A BEAST"
                } else if roster.players.is_empty() {
                    "GAMEPAD: A     KEYBOARD: SPACE (WASD) OR ENTER (ARROWS)"
                } else {
                    "A: READY     B: BACK     LEFT/RIGHT: COLOR"
                };
                Label::set(&mut label, text);
                Label::tint(&mut label, if all_ready { GOLD } else { Color::WHITE });
            }
        }
    }
    for (card, mut border) in &mut cards {
        let color = match roster.players.get(card.0) {
            Some(p) => hex(COLORS[p.color].1),
            None => Color::srgba(1.0, 1.0, 1.0, 0.25),
        };
        *border = BorderColor::all(color);
    }
}

// ---------------------------------------------------------------------------
// Course select
// ---------------------------------------------------------------------------

#[derive(Component)]
pub enum SelectText {
    Name,
    Beast,
    Blurb,
    Row(usize),
}

#[derive(Resource, Default)]
pub struct SelectCursor(usize);

const ROWS: usize = 6;

pub fn select_enter(mut commands: Commands, mut load: MessageWriter<LoadCourse>, settings: Res<Settings>) {
    load.write(LoadCourse(settings.course));
    commands.insert_resource(SelectCursor(0));
    commands
        .spawn((screen_root(), DespawnOnExit(Screen::Select)))
        .with_children(|p| {
            p.spawn(band()).with_children(|b| {
                b.spawn((label_node(), Label::new("", 7.0, GOLD), SelectText::Name));
                b.spawn((label_node(), Label::new("", 2.8, Color::WHITE), SelectText::Beast));
                b.spawn((label_node(), Label::new("", 1.9, SOFT), SelectText::Blurb));
            });
            p.spawn(band()).with_children(|b| {
                for row in 0..ROWS {
                    b.spawn((label_node(), Label::new("", 2.7, Color::WHITE), SelectText::Row(row)));
                }
                b.spawn((
                    Node {
                        margin: UiRect::top(Val::VMin(0.8)),
                        ..label_node()
                    },
                    Label::new(
                        "UP/DOWN: CHOOSE   LEFT/RIGHT: CHANGE   A: GO   B: BACK\n\
                     GAMEPAD   RT OR A: GAS   LT OR B: BRAKE   RB OR X: HOP AND DRIFT   LB OR Y: ITEM\n\
                     KEYS   W A S D + SPACE + E   OR   ARROWS + RIGHT SHIFT + ENTER",
                        1.6,
                        SOFT,
                    ),
                ));
            });
        });
}

pub fn select_update(
    mut commands: Commands,
    inputs: Res<Inputs>,
    roster: Res<Roster>,
    mut settings: ResMut<Settings>,
    mut cursor: ResMut<SelectCursor>,
    mut load: MessageWriter<LoadCourse>,
    mut next: ResMut<NextState<Screen>>,
    mut sfx: MessageWriter<Sfx>,
    mut texts: Query<(&SelectText, &mut Label)>,
) {
    let humans = roster.players.len().max(1);
    for p in &roster.players {
        let nav = inputs.nav(p.device);
        if nav.dy != 0 {
            cursor.0 = (cursor.0 as i32 + nav.dy).rem_euclid(ROWS as i32) as usize;
            sfx.write(Sfx(Sound::Blip));
        }
        if nav.dx != 0 {
            let step = |value: usize, count: usize| (value as i32 + nav.dx).rem_euclid(count as i32) as usize;
            match cursor.0 {
                0 => settings.grand_prix = !settings.grand_prix,
                1 if !settings.grand_prix => {
                    settings.course = step(settings.course, COURSES.len());
                    load.write(LoadCourse(settings.course));
                }
                2 => {
                    let i = LAP_CHOICES.iter().position(|&l| l == settings.laps).unwrap_or(2);
                    settings.laps = LAP_CHOICES[step(i, LAP_CHOICES.len())];
                }
                3 => {
                    let span = MAX_RACERS - humans + 1;
                    settings.racers = humans + step(settings.racers.max(humans) - humans, span);
                }
                4 => settings.class = step(settings.class, CLASSES.len()),
                _ => {}
            }
            sfx.write(Sfx(Sound::Blip));
        }
        if nav.confirm || nav.pause {
            sfx.write(Sfx(Sound::Confirm));
            if settings.grand_prix {
                // A Grand Prix visits every beast in order.
                settings.course = 0;
                commands.insert_resource(Cup::start());
            } else {
                commands.insert_resource(Cup::default());
            }
            next.set(Screen::Race);
            break;
        }
        if nav.back {
            sfx.write(Sfx(Sound::Back));
            next.set(Screen::Lobby);
        }
    }
    if settings.grand_prix && settings.course != 0 {
        settings.course = 0;
        load.write(LoadCourse(0));
    }
    settings.racers = settings.racers.clamp(humans, MAX_RACERS);

    let info = &COURSES[settings.course];
    for (text, mut label) in &mut texts {
        match *text {
            SelectText::Name => Label::set(&mut label, info.name),
            SelectText::Beast => Label::set(&mut label, info.beast),
            SelectText::Blurb => Label::set(&mut label, wrap(info.blurb, 52)),
            SelectText::Row(row) => {
                let text = match row {
                    0 => format!(
                        "MODE  < {} >",
                        if settings.grand_prix {
                            "GRAND PRIX"
                        } else {
                            "SINGLE RACE"
                        }
                    ),
                    1 if settings.grand_prix => format!("BEASTS  ALL {} IN TURN", COURSES.len()),
                    1 => format!("BEAST  < {}/{} >", settings.course + 1, COURSES.len()),
                    2 => format!("LAPS  < {} >", settings.laps),
                    3 => format!("RACERS  < {} >", settings.racers),
                    4 => format!("SPEED  < {} >", CLASSES[settings.class].0),
                    _ => if settings.grand_prix {
                        "START GRAND PRIX"
                    } else {
                        "START RACE"
                    }
                    .to_string(),
                };
                Label::set(&mut label, text);
                Label::tint(
                    &mut label,
                    if row == cursor.0 {
                        GOLD
                    } else {
                        Color::srgba(1.0, 1.0, 1.0, 0.75)
                    },
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------

pub fn results_enter(mut commands: Commands, results: Res<Results>, mut cup: ResMut<Cup>) {
    if cup.active {
        cup.score(&results.standings);
    }
    let over = cup.active && cup.round >= COURSES.len();
    let title = if !cup.active {
        "RESULTS".to_string()
    } else if over {
        "GRAND PRIX FINAL".to_string()
    } else {
        format!("GRAND PRIX  RACE {} OF {}", cup.round, COURSES.len())
    };
    commands
        .spawn((screen_root(), DespawnOnExit(Screen::Results)))
        .with_children(|p| {
            p.spawn(band()).with_children(|b| {
                b.spawn((label_node(), Label::new(title, 7.0, GOLD)));
                if over && let Some(winner) = cup.table.first() {
                    b.spawn((
                        label_node(),
                        Label::new(
                            format!("{} WINS THE GRAND PRIX!", winner.name),
                            3.4,
                            hex(COLORS[winner.color].1),
                        ),
                    ));
                }
            });
            p.spawn(band()).with_children(|b| {
                let row_node = || Node {
                    width: Val::VMin(118.0),
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::axes(Val::VMin(2.0), Val::VMin(0.3)),
                    border_radius: BorderRadius::all(Val::VMin(1.5)),
                    ..default()
                };
                let highlight = |human: bool| {
                    BackgroundColor(if human {
                        Color::srgba(1.0, 1.0, 1.0, 0.14)
                    } else {
                        Color::NONE
                    })
                };
                let cell = |w: f32| Node {
                    width: Val::VMin(w),
                    ..default()
                };
                if cup.active {
                    // The cup table: overall points, and what the last race added.
                    for (i, e) in cup.table.iter().enumerate() {
                        let size = if e.human { 3.4 } else { 2.9 };
                        b.spawn((row_node(), highlight(e.human))).with_children(|row| {
                            row.spawn((
                                cell(16.0),
                                Label::new(format!("{}{}", i + 1, ordinal(i + 1)), size, Color::WHITE),
                            ));
                            row.spawn((cell(44.0), Label::new(e.name.clone(), size, hex(COLORS[e.color].1))));
                            row.spawn((cell(22.0), Label::new(format!("+{}", e.last), size, SOFT)));
                            row.spawn(Label::new(format!("{} PTS", e.points), size, Color::WHITE));
                        });
                    }
                } else {
                    for (i, s) in results.standings.iter().enumerate() {
                        let size = if s.human { 3.6 } else { 3.0 };
                        let time = s.time.map_or("---".to_string(), format_time);
                        b.spawn((row_node(), highlight(s.human))).with_children(|row| {
                            row.spawn((
                                cell(16.0),
                                Label::new(format!("{}{}", i + 1, ordinal(i + 1)), size, Color::WHITE),
                            ));
                            row.spawn((cell(50.0), Label::new(s.name.clone(), size, hex(COLORS[s.color].1))));
                            row.spawn(Label::new(time, size, Color::WHITE));
                        });
                    }
                }
            });
            let footer = if !cup.active {
                "A: RACE AGAIN     B: CHANGE BEAST"
            } else if over {
                "A: CONTINUE"
            } else {
                "A: NEXT BEAST     B: QUIT GRAND PRIX"
            };
            p.spawn(band()).with_children(|b| {
                b.spawn((label_node(), Label::new(footer, 2.8, Color::WHITE)));
            });
        });
}

pub fn results_update(
    time: Res<Time<Real>>,
    args: Res<crate::Args>,
    mut shown: Local<Option<f32>>,
    inputs: Res<Inputs>,
    mut cup: ResMut<Cup>,
    mut settings: ResMut<Settings>,
    mut next: ResMut<NextState<Screen>>,
    mut sfx: MessageWriter<Sfx>,
) {
    let mut nav = inputs.any_nav();
    // Unattended runs move on by themselves.
    let since = *shown.get_or_insert(time.elapsed_secs());
    if args.autopilot > 0 && time.elapsed_secs() - since > 4.0 {
        nav.confirm = true;
    }
    let over = cup.active && cup.round >= COURSES.len();
    if nav.confirm || nav.pause {
        *shown = None;
        sfx.write(Sfx(Sound::Confirm));
        if over {
            cup.active = false;
            next.set(Screen::Select);
        } else {
            if cup.active {
                settings.course = cup.round;
            }
            next.set(Screen::Race);
        }
    } else if nav.back && !over {
        *shown = None;
        sfx.write(Sfx(Sound::Back));
        cup.active = false;
        next.set(Screen::Select);
    }
}

// ---------------------------------------------------------------------------
// Pause overlay
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct PauseRoot;

#[derive(Component)]
pub struct PauseRow(usize);

pub fn pause_overlay(
    mut commands: Commands,
    race: Option<Res<Race>>,
    roots: Query<Entity, With<PauseRoot>>,
    mut rows: Query<(&PauseRow, &mut Label)>,
) {
    let paused = race.as_ref().is_some_and(|r| r.paused);
    if !paused {
        for e in &roots {
            commands.entity(e).despawn();
        }
        return;
    }
    let cursor = race.map_or(0, |r| r.pause_cursor);
    if roots.is_empty() {
        commands
            .spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.02, 0.08, 0.6)),
                GlobalZIndex(50),
                PauseRoot,
            ))
            .with_children(|p| {
                p.spawn(band()).with_children(|b| {
                    b.spawn((label_node(), Label::new("PAUSED", 9.0, GOLD)));
                    for (i, text) in ["RESUME", "RESTART RACE", "QUIT RACE"].into_iter().enumerate() {
                        b.spawn((label_node(), Label::new(text, 4.0, Color::WHITE), PauseRow(i)));
                    }
                });
            });
    }
    for (row, mut label) in &mut rows {
        Label::tint(
            &mut label,
            if row.0 == cursor {
                GOLD
            } else {
                Color::srgba(1.0, 1.0, 1.0, 0.7)
            },
        );
    }
}
