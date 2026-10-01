//! Screen overlay: title card, crosshair, exhibit information panel,
//! location, discoveries and hints.

use crate::{
    content::{self, MUSEUM_NAME},
    exhibits::ExhibitIndex,
    layout::{self, Area, Walls},
    player::{Control, Player},
    tour::Tour,
};
use bevy::{
    prelude::*,
    text::{FontStyle, LetterSpacing, LineHeight},
};

/// The exhibit the visitor is looking at, and how long they have looked.
#[derive(Resource, Default)]
pub struct Focus {
    pub current: Option<usize>,
    pub dwell: f32,
}

#[derive(Resource)]
pub struct Discovered(pub Vec<bool>);

#[derive(Component)]
pub struct Overlay;
#[derive(Component)]
pub struct OverlayPrompt;
#[derive(Component)]
pub struct InfoPanel;
#[derive(Component)]
pub enum InfoText {
    Gallery,
    Title,
    Date,
    Blurb,
    Status,
}
#[derive(Component)]
pub struct LocationText;
#[derive(Component)]
pub struct CounterText;
#[derive(Component)]
pub struct Toast {
    pub timer: f32,
}
#[derive(Component)]
pub struct HintText;

/// Assigns text only when it changed, so unchanged labels aren't re-laid out.
fn set(text: &mut Mut<Text>, value: impl Into<String>) {
    let value = value.into();
    if text.0 != value {
        text.0 = value;
    }
}

fn hex(h: u32) -> Color {
    Color::srgb_u8((h >> 16) as u8, (h >> 8) as u8, h as u8)
}

fn font(source: FontSource, size: f32) -> TextFont {
    TextFont {
        font: source,
        font_size: FontSize::Px(size),
        ..default()
    }
}

pub fn spawn(mut commands: Commands) {
    let n = content::exhibits().len();
    commands.insert_resource(Discovered(vec![false; n]));
    commands.insert_resource(Focus::default());

    // Crosshair
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            width: px(6),
            height: px(6),
            margin: UiRect {
                left: px(-3),
                top: px(-3),
                ..default()
            },
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
    ));

    // Location (top left) and discoveries (top right)
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(28),
            top: px(22),
            ..default()
        },
        Text::new(""),
        font(FontSource::Serif, 22.0),
        LetterSpacing::Px(1.5),
        TextColor(Color::srgba(1.0, 0.96, 0.88, 0.9)),
        TextShadow {
            offset: Vec2::splat(1.5),
            color: Color::srgba(0.0, 0.0, 0.0, 0.85),
        },
        LocationText,
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(28),
            top: px(22),
            ..default()
        },
        Text::new(""),
        font(FontSource::SansSerif, 17.0),
        TextColor(Color::srgba(1.0, 0.96, 0.88, 0.85)),
        TextShadow {
            offset: Vec2::splat(1.5),
            color: Color::srgba(0.0, 0.0, 0.0, 0.85),
        },
        CounterText,
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            bottom: px(18),
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            Text::new("T  guided tour     M  map     Esc  pause"),
            font(FontSource::SansSerif, 14.0),
            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.45)),
            TextShadow {
                offset: Vec2::splat(1.5),
                color: Color::srgba(0.0, 0.0, 0.0, 0.85)
            },
            HintText,
        )],
    ));
    // Toast (top center)
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(70),
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            Node {
                padding: UiRect::axes(px(22), px(10)),
                border_radius: BorderRadius::all(px(20)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.06, 0.05, 0.8)),
            Visibility::Hidden,
            Toast { timer: 0.0 },
            children![(Text::new(""), font(FontSource::Serif, 20.0), TextColor(hex(0xf0d79a)))],
        )],
    ));

    // Information panel (right side)
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(28),
            bottom: px(60),
            width: px(430),
            padding: UiRect::all(px(26)),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            border: UiRect::left(px(5)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.06, 0.055, 0.05, 0.86)),
        BorderColor::all(hex(0xd9b464)),
        Visibility::Hidden,
        InfoPanel,
        children![
            (
                Text::new(""),
                font(FontSource::SansSerif, 13.0),
                LetterSpacing::Px(2.0),
                TextColor(hex(0xd9b464)),
                InfoText::Gallery
            ),
            (
                Text::new(""),
                TextFont {
                    weight: FontWeight::BOLD,
                    ..font(FontSource::Serif, 30.0)
                },
                LineHeight::RelativeToFont(1.1),
                TextColor(hex(0xf6f0e4)),
                InfoText::Title
            ),
            (
                Text::new(""),
                TextFont {
                    style: FontStyle::Italic,
                    ..font(FontSource::Serif, 17.0)
                },
                TextColor(hex(0xc9bca6)),
                InfoText::Date
            ),
            (
                Text::new(""),
                font(FontSource::Serif, 17.0),
                LineHeight::RelativeToFont(1.45),
                TextColor(hex(0xe8e1d4)),
                InfoText::Blurb
            ),
            (
                Text::new(""),
                font(FontSource::SansSerif, 13.0),
                TextColor(hex(0x9fd0a0)),
                InfoText::Status
            ),
        ],
    ));

    // Title / pause card
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: px(18),
            ..default()
        },
        BackgroundColor(Color::srgba(0.03, 0.025, 0.02, 0.72)),
        Overlay,
        children![
            (
                Text::new("WELCOME TO"),
                font(FontSource::SansSerif, 16.0),
                LetterSpacing::Px(6.0),
                TextColor(hex(0xd9b464))
            ),
            (
                Text::new(MUSEUM_NAME),
                TextFont {
                    weight: FontWeight::BOLD,
                    ..font(FontSource::Serif, 58.0)
                },
                TextColor(hex(0xf6f0e4)),
                TextShadow {
                    offset: Vec2::splat(1.5),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.85)
                },
            ),
            (
                Text::new("Seven galleries  ·  36 exhibits  ·  3.3 million years of ingenuity"),
                TextFont {
                    style: FontStyle::Italic,
                    ..font(FontSource::Serif, 21.0)
                },
                TextColor(hex(0xcfc4b0)),
            ),
            (
                Node {
                    width: px(120),
                    height: px(2),
                    margin: UiRect::vertical(px(12)),
                    ..default()
                },
                BackgroundColor(hex(0xd9b464))
            ),
            (
                Text::new("Click to enter"),
                TextFont {
                    weight: FontWeight::SEMIBOLD,
                    ..font(FontSource::SansSerif, 22.0)
                },
                TextColor(hex(0xffffff)),
                OverlayPrompt,
            ),
            (
                Text::new(
                    "W A S D  walk     Shift  hurry     Mouse  look around\n\
                     T  guided tour     M  map of the museum     Esc  pause     [ ]  mouse sensitivity"
                ),
                font(FontSource::SansSerif, 15.0),
                LineHeight::RelativeToFont(1.8),
                TextLayout::justify(Justify::Center),
                TextColor(hex(0xa8a092)),
            ),
        ],
    ));
}

/// Finds the exhibit under the crosshair.
pub fn update_focus(
    time: Res<Time>,
    index: Res<ExhibitIndex>,
    walls: Res<Walls>,
    control: Res<Control>,
    tour: Res<Tour>,
    mut focus: ResMut<Focus>,
    mut discovered: ResMut<Discovered>,
    player: Single<&Transform, With<Player>>,
    mut toast: Query<(&mut Toast, &mut Visibility, &Children)>,
    mut texts: Query<&mut Text>,
) {
    let eye = player.translation;
    let fwd = player.forward().as_vec3();
    let mut best: Option<(usize, f32)> = None;
    for (i, e) in index.0.iter().enumerate() {
        let to = e.focus - eye;
        let d = to.length();
        let reach = (e.radius * 6.0).max(6.5);
        if d > reach || d < 0.01 {
            continue;
        }
        let angle = fwd.dot(to / d).clamp(-1.0, 1.0).acos();
        let size = (e.radius / d).atan().max(0.05);
        let score = angle - size;
        if score < 0.1 && best.is_none_or(|(_, s)| score < s) && !layout::blocked(&walls, eye, e.focus) {
            best = Some((i, score));
        }
    }
    let now = best.map(|b| b.0);
    // On the tour, the current stop is always in focus.
    let now = if tour.active { tour.focus.or(now) } else { now };
    if now != focus.current {
        focus.current = now;
        focus.dwell = 0.0;
    } else {
        focus.dwell += time.delta_secs();
    }
    if !control.started {
        return;
    }
    if let Some(i) = focus.current
        && focus.dwell > 1.2
        && !discovered.0[i]
    {
        discovered.0[i] = true;
        let count = discovered.0.iter().filter(|d| **d).count();
        let total = discovered.0.len();
        let title = content::exhibits()[i].title;
        let msg = if count == total {
            "You have seen every exhibit. Thank you for visiting!".to_string()
        } else {
            format!("Discovered: {title}   ({count} of {total})")
        };
        if let Ok((mut t, mut vis, children)) = toast.single_mut() {
            t.timer = if count == total { 8.0 } else { 3.0 };
            *vis = Visibility::Inherited;
            if let Ok(mut text) = texts.get_mut(children[0]) {
                text.0 = msg;
            }
        }
    }
}

pub fn update_panel(
    focus: Res<Focus>,
    discovered: Res<Discovered>,
    control: Res<Control>,
    mut last: Local<Option<usize>>,
    mut panel: Query<(&mut Visibility, &mut BorderColor), With<InfoPanel>>,
    mut texts: Query<(&mut Text, &mut TextColor, &InfoText)>,
) {
    let Ok((mut vis, mut border)) = panel.single_mut() else {
        return;
    };
    let show = control.started && !control.paused && focus.current.is_some();
    vis.set_if_neq(if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    let Some(i) = focus.current else { return };
    let galleries = content::galleries();
    let e = &content::exhibits()[i];
    let accent = e.gallery.map(|g| galleries[g].accent).unwrap_or(hex(0xd9b464));
    let status = if discovered.0[i] {
        "Discovered"
    } else {
        "Keep looking to add it to your discoveries"
    };
    for (mut text, mut color, kind) in &mut texts {
        match kind {
            InfoText::Status => {
                set(&mut text, status);
                color.set_if_neq(TextColor(if discovered.0[i] { hex(0x9fd0a0) } else { hex(0x8a8276) }));
            }
            _ if *last == Some(i) => {}
            InfoText::Gallery => {
                text.0 = e
                    .gallery
                    .map(|g| galleries[g].name)
                    .unwrap_or("The Rotunda")
                    .to_uppercase();
                color.0 = accent;
            }
            InfoText::Title => text.0 = e.title.to_string(),
            InfoText::Date => text.0 = e.date.to_string(),
            InfoText::Blurb => text.0 = e.blurb.to_string(),
        }
    }
    border.set_if_neq(BorderColor::all(accent));
    *last = Some(i);
}

pub fn update_status(
    time: Res<Time>,
    control: Res<Control>,
    tour: Res<Tour>,
    discovered: Res<Discovered>,
    player: Single<&Transform, With<Player>>,
    mut overlay: Query<&mut Visibility, (With<Overlay>, Without<Toast>)>,
    mut prompt: Query<
        &mut Text,
        (
            With<OverlayPrompt>,
            Without<LocationText>,
            Without<CounterText>,
            Without<HintText>,
        ),
    >,
    mut location: Query<&mut Text, (With<LocationText>, Without<CounterText>, Without<HintText>)>,
    mut counter: Query<&mut Text, (With<CounterText>, Without<LocationText>, Without<HintText>)>,
    mut hint: Query<&mut Text, (With<HintText>, Without<LocationText>, Without<CounterText>)>,
    mut toast: Query<(&mut Toast, &mut Visibility), Without<Overlay>>,
) {
    if let Ok(mut v) = overlay.single_mut() {
        v.set_if_neq(if !control.started || control.paused {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    if let Ok(mut t) = prompt.single_mut() {
        set(
            &mut t,
            if control.paused {
                "Paused  ·  click to continue"
            } else {
                "Click to enter"
            },
        );
    }
    let galleries = content::galleries();
    if let Ok(mut t) = location.single_mut() {
        let here = match layout::locate(&galleries, player.translation) {
            Area::Rotunda => "The Rotunda".to_string(),
            Area::Vestibule => "Entrance Hall".to_string(),
            Area::Gallery(g) => format!("{}  ·  {}", galleries[g].name, galleries[g].era),
        };
        set(&mut t, here);
    }
    if let Ok(mut t) = counter.single_mut() {
        let count = discovered.0.iter().filter(|d| **d).count();
        set(&mut t, format!("Discoveries  {count} / {}", discovered.0.len()));
    }
    if let Ok(mut t) = hint.single_mut() {
        set(
            &mut t,
            if tour.active {
                "Guided tour   ·   N / →  next stop     P / ←  previous     T  end tour"
            } else {
                "T  guided tour     M  map     Esc  pause"
            },
        );
    }
    if let Ok((mut t, mut v)) = toast.single_mut() {
        if t.timer > 0.0 {
            t.timer -= time.delta_secs();
            if t.timer <= 0.0 {
                *v = Visibility::Hidden;
            }
        }
    }
}
