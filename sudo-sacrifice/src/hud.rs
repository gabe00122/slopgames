//! The heads-up display, styled like a terminal: the agent's context and
//! tokens, the spell bar, the minimap, the session log, and health bars
//! floating over everything that has been hurt.

use crate::{
    agent::{Agent, MAX_TOKENS, can_afford},
    game::{Health, Log, Match, RITUAL_TIME, SLOTS, Screen, Slot, Team},
    player::{Look, MainCam},
    software::{SoftState, Software},
    structures::{Altar, Datacenter, Interaction, Well, interaction},
    terrain::{CELLS, HALF, Terrain},
    units::{Unit, UnitKind},
    util::{format_time, hex, hexa, xz},
};
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PrimaryWindow,
};

pub const INK: u32 = 0xe6edf3;
pub const DIM: u32 = 0x8b949e;
pub const GREEN: u32 = 0x7ee787;
pub const YELLOW: u32 = 0xf2cc60;
pub const RED: u32 = 0xff6b6b;
pub const PANEL: u32 = 0x0d1117;
pub const EDGE: u32 = 0x30363d;

pub fn vm(v: f32) -> Val {
    Val::VMin(v)
}

pub fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::VMin(size),
        ..default()
    }
}

pub fn text(s: impl Into<String>, size: f32, color: u32) -> (Text, TextFont, TextColor) {
    (Text::new(s), font(size), TextColor(hex(color)))
}

/// A dark terminal panel.
pub fn panel() -> (BackgroundColor, BorderColor) {
    (BackgroundColor(hexa(PANEL, 0.82)), BorderColor::all(hex(EDGE)))
}

#[derive(Component)]
pub struct HudRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudText {
    AgentName,
    ContextNum,
    TokensNum,
    SoftwareNum,
    SoftwareNext,
    SlotName(usize),
    SlotCost(usize),
    TipTitle,
    TipBlurb,
    TipCost,
    Army,
    Clock,
    VsBlue,
    VsRed,
    RitualLabel,
    LogLine(usize),
    Prompt,
    Error,
    Banner,
    Dead,
    Tag(usize),
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudNode {
    ContextFill,
    TokensFill,
    Slot(usize),
    SlotCd(usize),
    Disk(usize),
    RitualBox,
    RitualFill,
    Cross,
    Bar(usize),
    BarFill(usize),
    Tag(usize),
}

const LOG_LINES: usize = 9;
const BARS: usize = 70;
const TAGS: usize = 14;
const DISKS: usize = 14;

#[derive(Resource)]
pub struct MiniMap {
    pub image: Handle<Image>,
    base: Vec<u8>,
    version: u64,
    timer: f32,
}

pub fn setup_minimap(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let n = CELLS as u32;
    let mut image = Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0u8; (n * n * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    commands.insert_resource(MiniMap {
        image: images.add(image),
        base: Vec::new(),
        version: 0,
        timer: 0.0,
    });
}

pub fn spawn(mut commands: Commands, map: Res<MiniMap>) {
    let abs = |node: Node| Node {
        position_type: PositionType::Absolute,
        ..node
    };
    let root = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            HudRoot,
            DespawnOnExit(Screen::Playing),
            GlobalZIndex(1),
        ))
        .id();
    commands.entity(root).with_children(|p| {
        // World-space bars and tags go under everything else.
        for i in 0..BARS {
            p.spawn((
                abs(Node {
                    width: px(40),
                    height: px(5),
                    border: UiRect::all(px(1)),
                    ..default()
                }),
                BackgroundColor(hexa(0x000000, 0.6)),
                BorderColor::all(hexa(0x000000, 0.8)),
                Visibility::Hidden,
                HudNode::Bar(i),
            ))
            .with_child((
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(hex(GREEN)),
                HudNode::BarFill(i),
            ));
        }
        for i in 0..TAGS {
            p.spawn((
                abs(Node {
                    padding: UiRect::axes(vm(0.5), vm(0.15)),
                    border_radius: BorderRadius::all(vm(0.4)),
                    ..default()
                }),
                BackgroundColor(hexa(PANEL, 0.6)),
                Visibility::Hidden,
                HudNode::Tag(i),
            ))
            .with_child((text("", 1.35, INK), HudText::Tag(i)));
        }

        // Status, bottom left.
        p.spawn((
            abs(Node {
                left: vm(2.0),
                bottom: vm(2.0),
                width: vm(40.0),
                padding: UiRect::all(vm(1.2)),
                row_gap: vm(0.7),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(vm(0.6)),
                ..default()
            }),
            panel(),
        ))
        .with_children(|s| {
            s.spawn((text("agent@orchestrator:~$", 1.6, Team::Blue.color()), HudText::AgentName));
            for (label, fill, num, color) in [
                ("CONTEXT ", HudNode::ContextFill, HudText::ContextNum, GREEN),
                ("TOKENS  ", HudNode::TokensFill, HudText::TokensNum, Team::Blue.color()),
            ] {
                s.spawn(Node {
                    align_items: AlignItems::Center,
                    column_gap: vm(1.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(text(label, 1.5, DIM));
                    row.spawn((
                        Node {
                            width: vm(18.0),
                            height: vm(1.5),
                            border: UiRect::all(px(1)),
                            ..default()
                        },
                        BackgroundColor(hexa(0x000000, 0.5)),
                        BorderColor::all(hex(EDGE)),
                    ))
                    .with_child((
                        Node {
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(hex(color)),
                        fill,
                    ));
                    row.spawn((text("", 1.5, INK), num));
                });
            }
            s.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: vm(1.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn(text("SOFTWARE", 1.5, DIM));
                row.spawn(Node {
                    column_gap: vm(0.35),
                    width: vm(18.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|disks| {
                    for i in 0..DISKS {
                        disks.spawn((
                            Node {
                                width: vm(0.95),
                                height: vm(1.1),
                                border: UiRect::top(vm(0.3)),
                                ..default()
                            },
                            BackgroundColor(hex(0x2f6fd6)),
                            BorderColor::all(hex(0xc9ced6)),
                            Visibility::Hidden,
                            HudNode::Disk(i),
                        ));
                    }
                });
                row.spawn((text("", 1.5, INK), HudText::SoftwareNum));
            });
            s.spawn((text("", 1.25, DIM), HudText::SoftwareNext));
        });

        // Spell bar, bottom center.
        p.spawn(abs(Node {
            left: percent(0),
            right: percent(0),
            bottom: vm(2.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: vm(0.8),
            ..default()
        }))
        .with_children(|col| {
            // Tooltip for the selected slot.
            col.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(vm(1.4), vm(0.6)),
                    border_radius: BorderRadius::all(vm(0.6)),
                    max_width: vm(62.0),
                    ..default()
                },
                BackgroundColor(hexa(PANEL, 0.7)),
            ))
            .with_children(|tip| {
                tip.spawn(Node {
                    column_gap: vm(1.5),
                    ..default()
                })
                .with_children(|r| {
                    r.spawn((text("", 1.8, INK), HudText::TipTitle));
                    r.spawn((text("", 1.5, YELLOW), HudText::TipCost));
                });
                tip.spawn((text("", 1.35, DIM), TextLayout::justify(Justify::Center), HudText::TipBlurb));
            });
            col.spawn(Node {
                column_gap: vm(0.5),
                align_items: AlignItems::FlexEnd,
                ..default()
            })
            .with_children(|bar| {
                for (i, slot) in SLOTS.iter().enumerate() {
                    if i == 5 {
                        bar.spawn(Node {
                            width: vm(1.6),
                            ..default()
                        });
                    }
                    let key = if i == 9 { "0".to_string() } else { format!("{}", i + 1) };
                    let group = matches!(slot, Slot::Summon(_));
                    bar.spawn((
                        Node {
                            width: vm(8.4),
                            height: vm(7.4),
                            border: UiRect::all(px(2)),
                            border_radius: BorderRadius::all(vm(0.6)),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(vm(0.5)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(hexa(if group { 0x161b22 } else { 0x11161d }, 0.85)),
                        BorderColor::all(hex(EDGE)),
                        HudNode::Slot(i),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            abs(Node {
                                left: px(0),
                                right: px(0),
                                bottom: px(0),
                                height: percent(0),
                                ..default()
                            }),
                            BackgroundColor(hexa(0x000000, 0.6)),
                            HudNode::SlotCd(i),
                        ));
                        b.spawn(text(format!("[{key}]"), 1.25, DIM));
                        b.spawn((
                            text(slot.name(), 1.2, INK),
                            TextLayout::justify(Justify::Center),
                            HudText::SlotName(i),
                        ));
                        b.spawn((text("", 1.2, YELLOW), HudText::SlotCost(i)));
                    });
                }
            });
            col.spawn(text(
                "WASD move  SPACE jump  LMB cast  RMB send subagents  R regroup  F interact  1-0/WHEEL select  ESC pause",
                1.2,
                DIM,
            ));
        });

        // Army, bottom right.
        p.spawn((
            abs(Node {
                right: vm(2.0),
                bottom: vm(2.0),
                width: vm(30.0),
                padding: UiRect::all(vm(1.2)),
                flex_direction: FlexDirection::Column,
                row_gap: vm(0.4),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(vm(0.6)),
                ..default()
            }),
            panel(),
        ))
        .with_children(|s| {
            s.spawn(text("$ ps aux | grep subagent", 1.4, DIM));
            s.spawn((text("", 1.45, INK), HudText::Army));
        });

        // Minimap, top left.
        p.spawn((
            abs(Node {
                left: vm(2.0),
                top: vm(2.0),
                padding: UiRect::all(vm(0.8)),
                flex_direction: FlexDirection::Column,
                row_gap: vm(0.5),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(vm(0.6)),
                ..default()
            }),
            panel(),
        ))
        .with_children(|m| {
            m.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|r| {
                r.spawn(text("~/map", 1.4, DIM));
                r.spawn((text("00:00", 1.4, DIM), HudText::Clock));
            });
            m.spawn((
                Node {
                    width: vm(27.0),
                    height: vm(27.0),
                    ..default()
                },
                ImageNode::new(map.image.clone()),
            ));
        });

        // Versus, top center.
        p.spawn(abs(Node {
            left: percent(0),
            right: percent(0),
            top: vm(2.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: vm(0.8),
            ..default()
        }))
        .with_children(|col| {
            col.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: vm(0.2),
                    padding: UiRect::axes(vm(1.2), vm(0.6)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(vm(0.6)),
                    ..default()
                },
                panel(),
            ))
            .with_children(|r| {
                r.spawn(text("AGENT          PROCS  DC  SW", 1.4, DIM));
                r.spawn((text("", 1.4, Team::Blue.color()), HudText::VsBlue));
                r.spawn((text("", 1.4, Team::Red.color()), HudText::VsRed));
            });
            col.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: vm(0.5),
                    padding: UiRect::all(vm(0.8)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(vm(0.6)),
                    ..default()
                },
                panel(),
                Visibility::Hidden,
                HudNode::RitualBox,
            ))
            .with_children(|b| {
                b.spawn((text("", 1.6, YELLOW), HudText::RitualLabel));
                b.spawn((
                    Node {
                        width: vm(40.0),
                        height: vm(1.2),
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(hexa(0x000000, 0.5)),
                    BorderColor::all(hex(EDGE)),
                ))
                .with_child((
                    Node {
                        width: percent(0),
                        height: percent(100),
                        ..default()
                    },
                    BackgroundColor(hex(YELLOW)),
                    HudNode::RitualFill,
                ));
            });
            col.spawn((
                text("", 3.2, INK),
                TextShadow {
                    offset: Vec2::splat(2.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.9),
                },
                HudText::Banner,
            ));
        });

        // Session log, top right.
        p.spawn((
            abs(Node {
                right: vm(2.0),
                top: vm(2.0),
                width: vm(52.0),
                padding: UiRect::all(vm(1.0)),
                flex_direction: FlexDirection::Column,
                row_gap: vm(0.25),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(vm(0.6)),
                ..default()
            }),
            BackgroundColor(hexa(PANEL, 0.6)),
            BorderColor::all(hexa(EDGE, 0.7)),
        ))
        .with_children(|l| {
            l.spawn(text("session.log", 1.3, DIM));
            for i in 0..LOG_LINES {
                l.spawn((text("", 1.35, INK), HudText::LogLine(i)));
            }
        });

        // Crosshair and the messages under it.
        p.spawn(abs(Node {
            left: percent(50),
            top: percent(50),
            width: vm(1.6),
            height: vm(1.6),
            margin: UiRect {
                left: vm(-0.8),
                top: vm(-0.8),
                ..default()
            },
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::MAX,
            ..default()
        }))
        .insert((BorderColor::all(hexa(0xffffff, 0.8)), HudNode::Cross));
        p.spawn(abs(Node {
            left: percent(0),
            right: percent(0),
            top: percent(56),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: vm(0.6),
            ..default()
        }))
        .with_children(|c| {
            c.spawn((
                text("", 1.7, INK),
                TextShadow {
                    offset: Vec2::splat(1.5),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.9),
                },
                HudText::Prompt,
            ));
            c.spawn((
                text("", 1.7, RED),
                TextShadow {
                    offset: Vec2::splat(1.5),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.9),
                },
                HudText::Error,
            ));
        });
        p.spawn(abs(Node {
            left: percent(0),
            right: percent(0),
            top: percent(34),
            justify_content: JustifyContent::Center,
            ..default()
        }))
        .with_child((
            text("", 2.6, RED),
            TextLayout::justify(Justify::Center),
            TextShadow {
                offset: Vec2::splat(2.0),
                color: Color::srgba(0.0, 0.0, 0.0, 0.9),
            },
            HudText::Dead,
        ));
    });
}

fn set(t: &mut Mut<Text>, s: impl AsRef<str>) {
    if t.0 != s.as_ref() {
        t.0 = s.as_ref().to_string();
    }
}

fn tint(c: &mut Mut<TextColor>, color: Color) {
    if c.0 != color {
        c.0 = color;
    }
}

fn bar(width: usize, frac: f32) -> String {
    let n = (frac.clamp(0.0, 1.0) * width as f32).round() as usize;
    format!("{}{}", "|".repeat(n), ".".repeat(width - n))
}

pub fn update(
    time: Res<Time<Real>>,
    game: Res<Match>,
    log: Res<Log>,
    look: Res<Look>,
    agents: Query<(&Agent, &Health, &Transform)>,
    units: Query<(Entity, &Unit, &Transform)>,
    wells: Query<(Entity, &Well, &Transform)>,
    dcs: Query<(&Datacenter, &Health)>,
    mut texts: Query<(&HudText, &mut Text, &mut TextColor)>,
    mut nodes: Query<
        (
            &HudNode,
            &mut Node,
            Option<&mut BackgroundColor>,
            Option<&mut BorderColor>,
            &mut Visibility,
        ),
        Without<HudText>,
    >,
) {
    // The player's agent, or Blue when the CPU is playing it.
    let viewer = agents
        .iter()
        .find(|(a, _, _)| a.player)
        .or_else(|| agents.iter().find(|(a, _, _)| a.team == Team::Blue));
    let Some((me, hp, tf)) = viewer else {
        return;
    };
    let enemy = agents.iter().find(|(a, _, _)| a.team != me.team);
    let t = time.elapsed_secs();
    let team = me.team;
    let census = |tm: Team| {
        let mut c = [0usize; 5];
        for (_, u, _) in &units {
            if u.team == tm {
                c[u.kind.index()] += 1;
            }
        }
        c
    };
    let mine = census(team);
    let dc_count = |tm: Team| dcs.iter().filter(|(d, h)| d.team == tm && h.alive()).count();
    // The interaction prompt.
    let well_list: Vec<(Entity, bool, Vec3)> = wells
        .iter()
        .map(|(e, w, t)| (e, w.datacenter.is_none(), t.translation))
        .collect();
    let gcs: Vec<(Entity, Team, Vec3)> = units
        .iter()
        .filter(|(_, u, _)| u.kind == UnitKind::Collector && u.summoning <= 0.0)
        .map(|(e, u, t)| (e, u.team, t.translation))
        .collect();
    let enemy_home = enemy.map_or(Vec3::ZERO, |(a, _, _)| a.home);
    let prompt = match interaction(me, tf.translation, &game, &well_list, enemy_home, &gcs) {
        Interaction::Nothing => (String::new(), INK),
        Interaction::Build(_) => ("[F] PROVISION DATACENTER  (80 TOKENS)".to_string(), GREEN),
        Interaction::Deprecate(_) => ("[F] DEPRECATE THIS ALTAR  (100 TOKENS)".to_string(), YELLOW),
        Interaction::Blocked(why) => (format!("[F] {why}"), DIM),
    };
    let sel = SLOTS[me.selected];
    for (kind, mut txt, mut color) in &mut texts {
        match *kind {
            HudText::AgentName => set(&mut txt, format!("agent@{}:~$", team.agent_name().to_lowercase())),
            HudText::ContextNum => {
                set(&mut txt, format!("{:>3.0}/{:.0}", hp.hp.max(0.0), hp.max));
                tint(&mut color, hex(if hp.frac() < 0.3 { RED } else { INK }));
            }
            HudText::TokensNum => set(&mut txt, format!("{:>3.0} +{:.1}/s", me.tokens, me.regen)),
            HudText::SoftwareNum => set(&mut txt, format!("{}", me.software.len())),
            HudText::SoftwareNext => set(
                &mut txt,
                match me.software.first() {
                    Some(n) => format!("next offering: {n}"),
                    None => "no software. find floppies, or bring your GC enemy disks".to_string(),
                },
            ),
            HudText::SlotName(_) => {}
            HudText::SlotCost(i) => {
                let s = SLOTS[i];
                let cost = match s {
                    Slot::Spell(_) => format!("{:.0}t", s.tokens()),
                    Slot::Summon(_) => format!("{:.0}t {}sw", s.tokens(), s.software()),
                };
                set(&mut txt, cost);
                tint(&mut color, hex(if can_afford(me, i) { YELLOW } else { RED }));
            }
            HudText::TipTitle => set(&mut txt, sel.name()),
            HudText::TipCost => set(
                &mut txt,
                match sel {
                    Slot::Spell(_) => format!("{:.0} tokens", sel.tokens()),
                    Slot::Summon(_) => format!("{:.0} tokens + sacrifice {} software", sel.tokens(), sel.software()),
                },
            ),
            HudText::TipBlurb => set(
                &mut txt,
                match sel {
                    Slot::Spell(s) => s.def().blurb,
                    Slot::Summon(k) => k.def().blurb,
                },
            ),
            HudText::Army => {
                let mut s = String::new();
                for k in UnitKind::ALL {
                    let n = mine[k.index()];
                    s.push_str(&format!("{:<18}{}\n", k.def().name.to_lowercase(), n));
                }
                s.push_str(&format!("{:<18}{}", "datacenters", dc_count(team)));
                set(&mut txt, s);
            }
            HudText::Clock => set(&mut txt, format_time(game.time)),
            HudText::VsBlue | HudText::VsRed => {
                let tm = if *kind == HudText::VsBlue {
                    Team::Blue
                } else {
                    Team::Red
                };
                let c = census(tm);
                let army: usize = c.iter().sum();
                let (sw, dead) = agents
                    .iter()
                    .find(|(a, _, _)| a.team == tm)
                    .map_or((0, false), |(a, _, _)| (a.software.len(), a.dead.is_some()));
                let status = if dead { " [z]" } else { "" };
                set(
                    &mut txt,
                    format!(
                        "{:<15}{:>5}{:>4}{:>4}{status}",
                        tm.agent_name().to_lowercase(),
                        army,
                        dc_count(tm),
                        sw
                    ),
                );
            }
            HudText::RitualLabel => {
                if let Some(r) = game.ritual {
                    let (msg, c) = if r.by == team {
                        (
                            format!("DEPRECATING {}'S ALTAR  {:.1}s", r.by.other().agent_name(), r.timer),
                            GREEN,
                        )
                    } else {
                        (format!("YOUR ALTAR IS BEING DEPRECATED  {:.1}s", r.timer), RED)
                    };
                    set(&mut txt, msg);
                    let blink = (t * 4.0).fract() < 0.5 || r.by == team;
                    tint(&mut color, hexa(c, if blink { 1.0 } else { 0.5 }));
                }
            }
            HudText::LogLine(i) => {
                let n = log.lines.len();
                let first = n.saturating_sub(LOG_LINES);
                match log.lines.get(first + i) {
                    Some(line) => {
                        set(&mut txt, &line.text);
                        let fade = (1.0 - (line.age - 25.0) / 10.0).clamp(0.35, 1.0);
                        tint(&mut color, hexa(line.color, fade));
                    }
                    None => set(&mut txt, ""),
                }
            }
            HudText::Prompt => {
                set(&mut txt, &prompt.0);
                tint(&mut color, hex(prompt.1));
            }
            HudText::Error => match (&me.error, &look.notice) {
                (Some(e), _) => {
                    set(&mut txt, &e.0);
                    tint(&mut color, hex(RED));
                }
                (None, Some(n)) => {
                    set(&mut txt, &n.0);
                    tint(&mut color, hex(INK));
                }
                (None, None) => set(&mut txt, ""),
            },
            HudText::Banner => match &game.banner {
                Some((msg, left, c)) => {
                    set(&mut txt, msg);
                    tint(&mut color, hexa(*c, left.min(1.0)));
                }
                None => set(&mut txt, ""),
            },
            HudText::Dead => set(
                &mut txt,
                match me.dead {
                    Some(left) => format!(
                        "CONTEXT WINDOW EXHAUSTED\ncompacting conversation... [{}]\nrespawning at your altar in {:.0}",
                        bar(20, 1.0 - left / crate::agent::RESPAWN),
                        left.ceil()
                    ),
                    None => String::new(),
                },
            ),
            HudText::Tag(_) => {}
        }
    }
    for (kind, mut node, bg, border, mut vis) in &mut nodes {
        match *kind {
            HudNode::ContextFill => node.width = percent(hp.frac() * 100.0),
            HudNode::TokensFill => node.width = percent(me.tokens / MAX_TOKENS * 100.0),
            HudNode::Slot(i) => {
                let selected = i == me.selected;
                let ok = can_afford(me, i);
                if let Some(mut b) = border {
                    let c = if selected {
                        hex(team.color())
                    } else if ok {
                        hex(EDGE)
                    } else {
                        hexa(RED, 0.4)
                    };
                    if b.top != c {
                        *b = BorderColor::all(c);
                    }
                }
                let _ = bg;
            }
            HudNode::SlotCd(i) => {
                let s = SLOTS[i];
                let total = match s {
                    Slot::Spell(sp) => sp.def().cooldown,
                    Slot::Summon(_) => 0.8,
                };
                let k = (me.cooldowns[i] / total).clamp(0.0, 1.0);
                node.height = percent(k * 100.0);
            }
            HudNode::Disk(i) => {
                let want = if i < me.software.len() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                vis.set_if_neq(want);
            }
            HudNode::RitualBox => {
                vis.set_if_neq(if game.ritual.is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
            }
            HudNode::RitualFill => {
                if let Some(r) = game.ritual {
                    node.width = percent((1.0 - r.timer / RITUAL_TIME) * 100.0);
                    if let Some(mut b) = bg {
                        b.0 = hex(if r.by == team { GREEN } else { RED });
                    }
                }
            }
            HudNode::Cross => {
                if let Some(mut b) = border {
                    let c = if look.aim_enemy { hex(RED) } else { hexa(0xffffff, 0.8) };
                    if b.top != c {
                        *b = BorderColor::all(c);
                    }
                }
                vis.set_if_neq(if me.dead.is_some() {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                });
            }
            _ => {}
        }
    }
}

/// Health bars over hurt things and name tags over the enemy agent and
/// nearby software.
pub fn world_labels(
    window: Single<&Window, With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    things: Query<(
        &Health,
        &GlobalTransform,
        Option<&Unit>,
        Option<&Agent>,
        Option<&Datacenter>,
    )>,
    disks: Query<(&Software, &GlobalTransform)>,
    mut nodes: Query<(&HudNode, &mut Node, &mut BackgroundColor, &mut Visibility), Without<HudText>>,
    mut texts: Query<(&HudText, &mut Text, &mut TextColor)>,
) {
    let Ok((cam, cgt)) = cams.single() else { return };
    let eye = cgt.translation();
    let k = (window.height() / 900.0).max(0.5);
    // (screen position, width, fill fraction, color)
    let mut bars: Vec<(Vec2, f32, f32, u32)> = Vec::new();
    let mut tags: Vec<(Vec2, String, u32)> = Vec::new();
    for (h, gt, unit, agent, dc) in &things {
        let pos = gt.translation();
        let dist = pos.distance(eye);
        if dist > 90.0 {
            continue;
        }
        let (height, width, show) = if let Some(u) = unit {
            (u.kind.def().height + 0.4, 34.0, h.frac() < 0.999 && u.summoning <= 0.0)
        } else if let Some(a) = agent {
            if a.dead.is_some() || a.player {
                continue;
            }
            (3.3, 70.0, true)
        } else if dc.is_some() {
            (8.2, 70.0, h.frac() < 0.999)
        } else {
            continue;
        };
        if !show {
            continue;
        }
        let Ok(sp) = cam.world_to_viewport(cgt, pos + Vec3::Y * height) else {
            continue;
        };
        let color = match (unit.map(|u| u.team), agent.map(|a| a.team), dc.map(|d| d.team)) {
            (Some(t), _, _) | (_, Some(t), _) | (_, _, Some(t)) => t.color(),
            _ => GREEN,
        };
        bars.push((sp, width * k * (1.0 - dist / 200.0), h.frac(), color));
        if let Some(a) = agent {
            tags.push((sp - Vec2::Y * 16.0 * k, a.team.agent_name().to_string(), a.team.color()));
        }
    }
    for (s, gt) in &disks {
        if matches!(s.state, SoftState::Carried(_)) {
            continue;
        }
        let pos = gt.translation();
        if pos.distance(eye) > 26.0 {
            continue;
        }
        if let Ok(sp) = cam.world_to_viewport(cgt, pos + Vec3::Y * 1.0) {
            let c = match s.owner {
                None => 0xffe9a8,
                Some(t) => t.color(),
            };
            tags.push((sp, s.name.clone(), c));
        }
    }
    bars.truncate(BARS);
    tags.truncate(TAGS);
    for (kind, mut node, mut bg, mut vis) in &mut nodes {
        match *kind {
            HudNode::Bar(i) => match bars.get(i) {
                Some((sp, w, _, _)) => {
                    vis.set_if_neq(Visibility::Inherited);
                    node.left = px(sp.x - w * 0.5);
                    node.top = px(sp.y);
                    node.width = px(*w);
                    node.height = px(6.0 * k);
                }
                None => {
                    vis.set_if_neq(Visibility::Hidden);
                }
            },
            HudNode::BarFill(i) => {
                if let Some((_, _, f, c)) = bars.get(i) {
                    node.width = percent(f * 100.0);
                    bg.0 = hex(*c);
                }
            }
            HudNode::Tag(i) => match tags.get(i) {
                Some((sp, text, _)) => {
                    vis.set_if_neq(Visibility::Inherited);
                    let w = text.len() as f32 * 7.6 * k;
                    node.left = px(sp.x - w * 0.5);
                    node.top = px(sp.y - 22.0 * k);
                }
                None => {
                    vis.set_if_neq(Visibility::Hidden);
                }
            },
            _ => {}
        }
    }
    for (kind, mut txt, mut color) in &mut texts {
        if let HudText::Tag(i) = *kind
            && let Some((_, s, c)) = tags.get(i)
        {
            set(&mut txt, s);
            tint(&mut color, hex(*c));
        }
    }
}

/// Redraws the minimap: the ground (when it changes) and everything on it.
pub fn minimap(
    time: Res<Time<Real>>,
    terrain: Option<Res<Terrain>>,
    look: Res<Look>,
    mut map: ResMut<MiniMap>,
    mut images: ResMut<Assets<Image>>,
    agents: Query<(&Agent, &Transform)>,
    units: Query<(&Unit, &Transform)>,
    altars: Query<(&Altar, &Transform)>,
    wells: Query<(&Well, &Transform)>,
    dcs: Query<(&Datacenter, &Transform)>,
    disks: Query<(&Software, &Transform)>,
) {
    let Some(terrain) = terrain else { return };
    map.timer -= time.delta_secs();
    if map.timer > 0.0 {
        return;
    }
    map.timer = 1.0 / 12.0;
    if map.version != terrain.version || map.base.is_empty() {
        // Throttle full redraws of the ground while it is changing.
        map.base = terrain.map_pixels();
        map.version = terrain.version;
    }
    let n = CELLS as i32;
    let mut px = map.base.clone();
    let to_px = |p: Vec3| -> (i32, i32) {
        let x = ((p.x + HALF) / (2.0 * HALF) * n as f32) as i32;
        let y = ((p.z + HALF) / (2.0 * HALF) * n as f32) as i32;
        (x, y)
    };
    let mut dot = |p: Vec3, r: i32, c: u32, outline: Option<u32>| {
        let (cx, cy) = to_px(p);
        let rgb = |c: u32| [(c >> 16) as u8, (c >> 8) as u8, c as u8];
        let edge = r + outline.is_some() as i32;
        for dy in -edge..=edge {
            for dx in -edge..=edge {
                let (x, y) = (cx + dx, cy + dy);
                if x < 0 || y < 0 || x >= n || y >= n {
                    continue;
                }
                let inner = dx.abs() <= r && dy.abs() <= r;
                let col = if inner { c } else { outline.unwrap_or(c) };
                let i = ((y * n + x) * 4) as usize;
                px[i..i + 3].copy_from_slice(&rgb(col));
            }
        }
    };
    for (s, tf) in &disks {
        let c = match s.owner {
            None => 0xffe9a8,
            Some(t) => t.color(),
        };
        dot(tf.translation, 0, c, None);
    }
    for (w, tf) in &wells {
        if w.datacenter.is_none() {
            dot(tf.translation, 1, 0x9ff3ff, Some(0x0d1117));
        }
    }
    for (d, tf) in &dcs {
        dot(tf.translation, 2, d.team.color(), Some(0xffffff));
    }
    for (a, tf) in &altars {
        dot(tf.translation, 4, a.team.color(), Some(0x0d1117));
    }
    for (u, tf) in &units {
        dot(
            tf.translation,
            if u.kind == UnitKind::Monolith { 1 } else { 0 },
            u.team.color(),
            None,
        );
    }
    for (a, tf) in &agents {
        if a.dead.is_some() {
            continue;
        }
        if a.player {
            // View direction.
            let d = crate::util::dir_of(look.yaw);
            for k in 3..9 {
                let p = tf.translation + Vec3::new(d.x, 0.0, d.y) * k as f32 * 2.0;
                dot(p, 0, 0xffffff, None);
            }
        }
        dot(
            tf.translation,
            2,
            a.team.color(),
            Some(if a.player { 0xffffff } else { 0x0d1117 }),
        );
    }
    if let Some(mut image) = images.get_mut(&map.image) {
        image.data = Some(px);
    }
    let _ = xz(Vec3::ZERO);
}

pub fn age_log(time: Res<Time<Real>>, mut log: ResMut<Log>, mut game: ResMut<Match>) {
    let dt = time.delta_secs();
    for l in &mut log.lines {
        l.age += dt;
    }
    let done = game.banner.as_mut().is_some_and(|b| {
        b.1 -= dt;
        b.1 <= 0.0
    });
    if done {
        game.banner = None;
    }
}
