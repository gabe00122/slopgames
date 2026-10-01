//! The heads-up display: health and moisture, the hunters left, how exposed
//! the bogwight is, "?" and "!" over hunters' heads, floating kill words,
//! context prompts, and the hurt and lightning flashes.

use crate::{
    fx::Fx,
    game::*,
    hunters::{Hunter, State},
    input::Controls,
    player::{Bogwight, MAX_HP, Medium, Stealth},
    util::{hex, hexa},
    weather::Storm,
};
use avian2d::prelude::Position;
use bevy::prelude::*;

pub const INK: u32 = 0xdde6d0;
pub const DIM: u32 = 0x8a9a88;
pub const BILE: u32 = 0xc8ff4a;
pub const BLOOD: u32 = 0xff5a44;
pub const AMBER: u32 = 0xffc450;
pub const WATER: u32 = 0x5ad8ff;

pub fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::VMin(size),
        ..default()
    }
}

pub fn text(s: impl Into<String>, size: f32, color: u32) -> (Text, TextFont, TextColor) {
    (Text::new(s), font(size), TextColor(hex(color)))
}

#[derive(Component)]
pub struct HudRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudText {
    Night,
    Hunters,
    Status,
    Banner,
    Prompt,
    Hint,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudNode {
    HpFill,
    DampFill,
    VisFill,
    Hurt,
    Flash,
    DampLabel,
}

#[derive(Component)]
pub struct Marker;

#[derive(Component)]
pub struct PopupText(usize);

const MARKERS: usize = 24;
const POPUPS: usize = 10;

struct Floating {
    pos: Vec2,
    text: String,
    color: u32,
    big: bool,
    age: f32,
}

#[derive(Resource, Default)]
pub struct Popups {
    live: Vec<Floating>,
}

impl Popups {
    pub fn clear(&mut self) {
        self.live.clear();
    }
}

/// A line of guidance at the top of the screen that fades after a while.
#[derive(Resource, Default)]
pub struct Banner {
    pub text: String,
    pub t: f32,
}

impl Banner {
    pub fn show(&mut self, text: impl Into<String>, secs: f32) {
        self.text = text.into();
        self.t = secs;
    }
}

fn bar(fill: HudNode, color: u32, width: f32) -> impl Bundle {
    (
        Node {
            width: Val::VMin(width),
            height: Val::VMin(1.6),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(hexa(0x0a0f0a, 0.7)),
        BorderColor::all(hexa(0x3a4a38, 0.9)),
        children![(
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(hex(color)),
            fill,
        )],
    )
}

pub fn spawn(mut commands: Commands) {
    let abs = |node: Node| Node {
        position_type: PositionType::Absolute,
        ..node
    };
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
            HudRoot,
            Visibility::Hidden,
        ))
        .with_children(|root| {
            // Full-screen flashes.
            root.spawn((
                abs(Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                }),
                BackgroundColor(Color::NONE),
                HudNode::Flash,
            ));
            root.spawn((
                abs(Node {
                    width: percent(100),
                    height: percent(100),
                    border: UiRect::all(Val::VMin(6.0)),
                    ..default()
                }),
                BorderColor::all(Color::NONE),
                BackgroundColor(Color::NONE),
                HudNode::Hurt,
            ));
            // Top left: vitals.
            root.spawn(abs(Node {
                left: Val::VMin(2.5),
                top: Val::VMin(2.5),
                flex_direction: FlexDirection::Column,
                row_gap: Val::VMin(0.8),
                ..default()
            }))
            .with_children(|c| {
                c.spawn(text("BOGWIGHT", 2.2, BILE));
                c.spawn(bar(HudNode::HpFill, 0x7fbf3a, 26.0));
                c.spawn((text("damp", 1.6, DIM), HudNode::DampLabel));
                c.spawn(bar(HudNode::DampFill, 0x3aa8d8, 26.0));
            });
            // Top right: the night and the quarry.
            root.spawn(abs(Node {
                right: Val::VMin(2.5),
                top: Val::VMin(2.5),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::VMin(0.6),
                ..default()
            }))
            .with_children(|c| {
                c.spawn((text("NIGHT 1", 2.2, DIM), HudText::Night));
                c.spawn((text("HUNTERS 0", 3.2, INK), HudText::Hunters));
            });
            // Bottom: how visible you are.
            root.spawn(abs(Node {
                bottom: Val::VMin(3.0),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::VMin(0.6),
                ..default()
            }))
            .with_children(|c| {
                c.spawn((text("HIDDEN", 2.4, DIM), HudText::Status));
                c.spawn(bar(HudNode::VisFill, 0xffc450, 18.0));
            });
            // Banner and control hint.
            root.spawn(abs(Node {
                top: Val::VMin(9.0),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::VMin(1.0),
                ..default()
            }))
            .with_children(|c| {
                c.spawn((
                    text("", 3.0, INK),
                    HudText::Banner,
                    TextLayout::justify(Justify::Center),
                ));
                c.spawn((text("", 2.0, DIM), HudText::Hint, TextLayout::justify(Justify::Center)));
            });
            root.spawn((abs(Node::default()), text("", 2.2, BILE), HudText::Prompt));
            for _ in 0..MARKERS {
                root.spawn((abs(Node::default()), text("?", 4.0, AMBER), Marker, Visibility::Hidden));
            }
            for i in 0..POPUPS {
                root.spawn((
                    abs(Node::default()),
                    text("", 3.0, INK),
                    PopupText(i),
                    Visibility::Hidden,
                ));
            }
        });
}

pub fn show(mut q: Query<&mut Visibility, With<HudRoot>>) {
    for mut v in &mut q {
        *v = Visibility::Inherited;
    }
}

pub fn hide(mut q: Query<&mut Visibility, With<HudRoot>>) {
    for mut v in &mut q {
        *v = Visibility::Hidden;
    }
}

pub fn collect_popups(mut reader: MessageReader<Popup>, mut popups: ResMut<Popups>) {
    for p in reader.read() {
        popups.live.push(Floating {
            pos: p.pos,
            text: p.text.clone(),
            color: p.color,
            big: p.big,
            age: 0.0,
        });
        if popups.live.len() > POPUPS {
            popups.live.remove(0);
        }
    }
}

pub fn update(
    time: Res<Time>,
    real: Res<Time<Real>>,
    session: Res<Session>,
    stealth: Res<Stealth>,
    storm: Res<Storm>,
    fx: Res<Fx>,
    controls: Res<Controls>,
    mut banner: ResMut<Banner>,
    level: Res<crate::world::LevelInfo>,
    player: Query<&Bogwight>,
    hunters: Query<&Hunter>,
    mut texts: Query<(&HudText, &mut Text, &mut TextColor)>,
    mut nodes: Query<(
        &HudNode,
        &mut Node,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
    )>,
) {
    banner.t -= real.delta_secs();
    let bw = player.single().ok();
    let near = |h: &&Hunter| h.last_seen.distance(stealth.pos) < 30.0;
    let hunting = hunters.iter().filter(near).any(|h| h.state == State::Hunt);
    let wary = hunters.iter().filter(near).any(|h| h.state == State::Suspicious);
    let blink = (time.elapsed_secs() * 3.0).fract() < 0.5;
    for (kind, mut t, mut c) in &mut texts {
        let (s, col) = match kind {
            HudText::Night => {
                let place = level.section_at(stealth.pos.x).map_or("", |s| s.kind.name());
                (format!("{place}   NIGHT {}", session.night), DIM)
            }
            HudText::Hunters => (format!("HUNTERS LEFT  {}", session.hunters_left), INK),
            HudText::Status => {
                if bw.is_some_and(|b| b.dead) {
                    ("".into(), DIM)
                } else if hunting {
                    ("HUNTED".into(), BLOOD)
                } else if wary {
                    ("SOMETHING'S WATCHING".into(), AMBER)
                } else if stealth.in_reeds && stealth.visibility < 0.2 {
                    ("IN THE REEDS".into(), 0x6a8a6a)
                } else if stealth.visibility < 0.12 {
                    ("HIDDEN".into(), 0x6a8a6a)
                } else if stealth.visibility < 0.35 {
                    ("IN THE GLOOM".into(), DIM)
                } else {
                    ("EXPOSED".into(), AMBER)
                }
            }
            HudText::Banner => (
                if banner.t > 0.0 {
                    banner.text.clone()
                } else {
                    String::new()
                },
                INK,
            ),
            HudText::Hint => {
                let dry = bw.is_some_and(|b| b.damp < 0.3 && b.medium != Medium::Water && !b.dead);
                let s = if dry && blink {
                    "drying out: get back in the water".to_string()
                } else if banner.t > 0.0 {
                    let pad = controls.pad;
                    if pad {
                        "stick move   A burst/jump   X claw   B grab/throw".to_string()
                    } else {
                        "WASD move   Space burst/jump   J claw   K grab/throw".to_string()
                    }
                } else {
                    String::new()
                };
                (s, if dry { WATER } else { DIM })
            }
            HudText::Prompt => continue,
        };
        if t.0 != s {
            t.0 = s;
        }
        c.0 = hex(col);
    }
    for (kind, mut node, bg, border) in &mut nodes {
        match kind {
            HudNode::HpFill => {
                let hp = bw.map_or(0.0, |b| b.hp / MAX_HP);
                node.width = percent(hp * 100.0);
                if let Some(mut bg) = bg {
                    bg.0 = if hp < 0.3 && blink { hex(BLOOD) } else { hex(0x7fbf3a) };
                }
            }
            HudNode::DampFill => {
                node.width = percent(bw.map_or(0.0, |b| b.damp) * 100.0);
            }
            HudNode::VisFill => {
                node.width = percent(stealth.visibility * 100.0);
            }
            HudNode::Hurt => {
                if let Some(mut b) = border {
                    *b = BorderColor::all(hexa(0x9a0a00, fx.hurt * 0.45));
                }
            }
            HudNode::Flash => {
                if let Some(mut bg) = bg {
                    bg.0 = Color::srgba(0.8, 0.85, 1.0, (storm.flash * 0.05).min(0.12));
                }
            }
            HudNode::DampLabel => {}
        }
    }
}

/// "?" and "!" over hunters, floating words, and the context prompt.
pub fn world_labels(
    real: Res<Time<Real>>,
    stealth: Res<Stealth>,
    controls: Res<Controls>,
    mut popups: ResMut<Popups>,
    cam: Query<(&Camera, &GlobalTransform)>,
    hunters: Query<(&Hunter, &Position)>,
    player: Query<&Bogwight>,
    mut markers: Query<(&mut Node, &mut Text, &mut TextColor, &mut Visibility), (With<Marker>, Without<PopupText>)>,
    mut pops: Query<
        (
            &PopupText,
            &mut Node,
            &mut Text,
            &mut TextColor,
            &mut TextFont,
            &mut Visibility,
        ),
        (Without<Marker>, Without<HudText>),
    >,
    mut prompt: Query<(&HudText, &mut Node, &mut Text), (Without<Marker>, Without<PopupText>)>,
) {
    let Ok((cam, cgt)) = cam.single() else { return };
    let dt = real.delta_secs();
    let mut shown = vec![];
    for (h, pos) in &hunters {
        let (s, color) = match h.state {
            State::Suspicious => ("?", AMBER),
            State::Hunt => ("!", BLOOD),
            State::Flounder => ("~", WATER),
            State::Held if h.drown > 0.05 => ("~", WATER),
            _ => continue,
        };
        shown.push((pos.0 + Vec2::Y * 1.35 * h.scale, s, color, h.alert));
    }
    let mut it = shown.into_iter();
    for (mut node, mut t, mut c, mut vis) in &mut markers {
        match it.next() {
            Some((p, s, color, alert)) => {
                if let Ok(sp) = cam.world_to_viewport(cgt, p.extend(0.0)) {
                    node.left = Val::Px(sp.x - 6.0);
                    node.top = Val::Px(sp.y - 20.0);
                    if t.0 != s {
                        t.0 = s.to_string();
                    }
                    let a = if s == "?" { 0.4 + alert.min(1.0) * 0.6 } else { 1.0 };
                    c.0 = hexa(color, a);
                    *vis = Visibility::Inherited;
                } else {
                    *vis = Visibility::Hidden;
                }
            }
            None => *vis = Visibility::Hidden,
        }
    }
    for f in popups.live.iter_mut() {
        f.age += dt;
    }
    popups.live.retain(|f| f.age < 1.6);
    for (PopupText(i), mut node, mut t, mut c, mut font_, mut vis) in &mut pops {
        let Some(f) = popups.live.get(*i) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let p = f.pos + Vec2::Y * f.age * 0.8;
        if let Ok(sp) = cam.world_to_viewport(cgt, p.extend(0.0)) {
            let size = if f.big { 3.6 } else { 2.4 };
            node.left = Val::Px(sp.x - f.text.len() as f32 * size * 3.0);
            node.top = Val::Px(sp.y);
            if t.0 != f.text {
                t.0 = f.text.clone();
            }
            font_.font_size = FontSize::VMin(size * (1.0 + (0.2 - f.age).max(0.0) * 2.0));
            c.0 = hexa(f.color, (1.6 - f.age).min(1.0));
            *vis = Visibility::Inherited;
        } else {
            *vis = Visibility::Hidden;
        }
    }
    // Context prompt: what the claw or grab would do right now.
    let bw = player.single().ok();
    for (kind, mut node, mut t) in &mut prompt {
        if *kind != HudText::Prompt {
            continue;
        }
        let mut s = String::new();
        if let Some(bw) = bw
            && !bw.dead
        {
            let pad = controls.pad;
            let near = hunters
                .iter()
                .filter(|(h, p)| h.state != State::Dead && p.0.distance(stealth.pos) < 1.6)
                .min_by(|a, b| a.1.0.distance(stealth.pos).total_cmp(&b.1.0.distance(stealth.pos)));
            if bw.held.is_some() {
                s = if pad { "B throw" } else { "K throw" }.into();
            } else if bw.grip.is_some() {
                s = if pad {
                    "down: pull it over   B let go"
                } else {
                    "S: pull it over   K let go"
                }
                .into();
            } else if let Some((h, _)) = near {
                let unaware = matches!(h.state, State::Patrol | State::Suspicious | State::Flounder);
                s = match (unaware, pad) {
                    (true, true) => "X AMBUSH   B grab",
                    (true, false) => "J AMBUSH   K grab",
                    (false, true) => "X claw   B grab",
                    (false, false) => "J claw   K grab",
                }
                .into();
            }
        }
        if t.0 != s {
            t.0 = s;
        }
        if let Ok(sp) = cam.world_to_viewport(cgt, (stealth.pos + Vec2::new(0.0, -0.9)).extend(0.0)) {
            node.left = Val::Px(sp.x - t.0.len() as f32 * 4.0);
            node.top = Val::Px(sp.y);
        }
    }
}
