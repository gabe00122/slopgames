//! Each player's heads-up display: item slot, lap, place, minimap and the big
//! announcements. One HUD is built per split-screen view and targets that
//! view's camera.

use crate::{
    beast::COURSES,
    camera::{ViewCam, ViewMode, Views},
    course::Course,
    font::{Label, label_node},
    game::COLORS,
    items::Item,
    kart::Kart,
    race::{Phase, Race, RaceEntity},
    util::{format_time, hex, ordinal},
};
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageSampler, TextureAtlas, TextureAtlasLayout},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

// ---------------------------------------------------------------------------
// A tiny software canvas for icons and the minimap.
// ---------------------------------------------------------------------------

pub struct Canvas {
    pub w: u32,
    pub h: u32,
    data: Vec<[f32; 4]>,
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Canvas {
            w,
            h,
            data: vec![[0.0; 4]; (w * h) as usize],
        }
    }

    /// Paints `color` wherever the signed distance `sdf` (in pixels) is
    /// negative, within the rectangle `(x, y, w, h)`.
    pub fn fill_in(&mut self, rect: (u32, u32, u32, u32), color: u32, sdf: impl Fn(Vec2) -> f32) {
        let c = hex(color).to_srgba();
        for py in rect.1..(rect.1 + rect.3).min(self.h) {
            for px in rect.0..(rect.0 + rect.2).min(self.w) {
                let p = Vec2::new(px as f32 + 0.5 - rect.0 as f32, py as f32 + 0.5 - rect.1 as f32);
                let a = (0.5 - sdf(p)).clamp(0.0, 1.0);
                if a <= 0.0 {
                    continue;
                }
                let dst = &mut self.data[(py * self.w + px) as usize];
                let out_a = a + dst[3] * (1.0 - a);
                for (i, src) in [c.red, c.green, c.blue].into_iter().enumerate() {
                    dst[i] = (src * a + dst[i] * dst[3] * (1.0 - a)) / out_a.max(1e-6);
                }
                dst[3] = out_a;
            }
        }
    }

    /// A shape with a dark outline.
    pub fn shape(&mut self, rect: (u32, u32, u32, u32), color: u32, sdf: impl Fn(Vec2) -> f32) {
        self.fill_in(rect, 0x1a142e, |p| sdf(p) - 3.0);
        self.fill_in(rect, color, sdf);
    }

    pub fn into_image(self) -> Image {
        let bytes: Vec<u8> = self
            .data
            .iter()
            .flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8))
            .collect();
        let mut image = Image::new(
            Extent3d {
                width: self.w,
                height: self.h,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            bytes,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::linear();
        image
    }
}

fn circle(p: Vec2, c: Vec2, r: f32) -> f32 {
    p.distance(c) - r
}

fn capsule(p: Vec2, a: Vec2, b: Vec2, r: f32) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t) - r
}

fn ellipse(p: Vec2, c: Vec2, r: Vec2) -> f32 {
    let q = (p - c) / r;
    (q.length() - 1.0) * r.min_element()
}

/// A star with `points` tips, outer radius `r` and inner radius `r * inner`.
fn star(p: Vec2, c: Vec2, r: f32, inner: f32, points: f32, spin: f32) -> f32 {
    let q = p - c;
    let a = q.y.atan2(q.x) + spin;
    let seg = std::f32::consts::TAU / points;
    let local = ((a / seg).fract() + 1.0).fract();
    let tri = (local - 0.5).abs() * 2.0;
    q.length() - r * (inner + (1.0 - inner) * (1.0 - tri).powf(1.6))
}

const ICON: u32 = 96;

#[derive(Resource)]
pub struct HudAssets {
    icons: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
}

pub fn setup_assets(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let mut c = Canvas::new(ICON * 8, ICON);
    let cell = |i: u32| (i * ICON, 0, ICON, ICON);
    let v = Vec2::new;
    // Dash berry.
    c.shape(cell(0), 0x3fa34d, |p| ellipse(p, v(58.0, 22.0), v(16.0, 8.0)));
    c.shape(cell(0), 0xe63946, |p| circle(p, v(46.0, 54.0), 27.0));
    c.fill_in(cell(0), 0xff9aa2, |p| circle(p, v(36.0, 44.0), 7.0));
    // Berry bunch.
    c.shape(cell(1), 0x3fa34d, |p| ellipse(p, v(50.0, 14.0), v(15.0, 7.0)));
    for (x, y) in [(30.0, 40.0), (64.0, 40.0), (47.0, 68.0)] {
        c.shape(cell(1), 0xe63946, |p| circle(p, v(x, y), 18.0));
        c.fill_in(cell(1), 0xff9aa2, |p| circle(p, v(x - 6.0, y - 6.0), 4.5));
    }
    // Spit seed.
    c.shape(cell(2), 0x7ddc4a, |p| {
        ellipse(p, v(48.0, 52.0), v(22.0, 30.0)).max(-(p.y - 20.0))
    });
    c.shape(cell(2), 0x7ddc4a, |p| capsule(p, v(48.0, 30.0), v(48.0, 14.0), 7.0));
    c.fill_in(cell(2), 0xd7ff8a, |p| capsule(p, v(40.0, 40.0), v(40.0, 62.0), 4.0));
    // Hornet.
    c.shape(cell(3), 0xdff6ff, |p| ellipse(p, v(30.0, 28.0), v(19.0, 12.0)));
    c.shape(cell(3), 0xdff6ff, |p| ellipse(p, v(66.0, 28.0), v(19.0, 12.0)));
    c.shape(cell(3), 0xffc20e, |p| ellipse(p, v(48.0, 56.0), v(30.0, 21.0)));
    for x in [34.0, 50.0, 66.0] {
        c.fill_in(cell(3), 0x1c1c24, |p| {
            capsule(p, v(x, 40.0), v(x, 72.0), 4.0).max(ellipse(p, v(48.0, 56.0), v(30.0, 21.0)))
        });
    }
    c.fill_in(cell(3), 0xff4040, |p| circle(p, v(24.0, 52.0), 4.0));
    // Burr.
    c.shape(cell(4), 0xc98f4a, |p| star(p, v(48.0, 50.0), 38.0, 0.5, 9.0, 0.3));
    c.fill_in(cell(4), 0x8a5a2b, |p| circle(p, v(48.0, 50.0), 17.0));
    // Golden pollen.
    c.shape(cell(5), 0xffc20e, |p| star(p, v(48.0, 50.0), 40.0, 0.42, 5.0, 0.95));
    c.fill_in(cell(5), 0xfff3b0, |p| star(p, v(48.0, 50.0), 22.0, 0.42, 5.0, 0.95));
    // Beast horn: a curling horn with sound rings.
    for (r, w) in [(34.0, 3.0), (25.0, 3.0)] {
        c.fill_in(cell(6), 0xf7f7f2, |p| {
            (circle(p, v(62.0, 44.0), r).abs() - w)
                .max(60.0 - p.x)
                .max((p.y - 44.0).abs() - 22.0)
        });
    }
    c.shape(cell(6), 0xe9d7a2, |p| {
        let t = ((p.x - 12.0) / 50.0).clamp(0.0, 1.0);
        capsule(p, v(12.0, 66.0), v(62.0, 44.0), 3.0 + 13.0 * t)
    });
    c.fill_in(cell(6), 0x7a5230, |p| ellipse(p, v(63.0, 43.0), v(6.0, 14.0)));
    // Roulette placeholder.
    c.shape(cell(7), 0xf7f7f2, |p| {
        let hook = (circle(p, v(48.0, 36.0), 15.0).abs() - 5.5).max(-(p.y - 40.0).min(p.x - 44.0));
        hook.min(capsule(p, v(48.0, 50.0), v(48.0, 58.0), 5.5))
            .min(circle(p, v(48.0, 76.0), 6.5))
    });
    commands.insert_resource(HudAssets {
        icons: images.add(c.into_image()),
        layout: layouts.add(TextureAtlasLayout::from_grid(UVec2::splat(ICON), 8, 1, None, None)),
    });
}

// ---------------------------------------------------------------------------
// Minimap
// ---------------------------------------------------------------------------

/// A picture of the track from above, rebuilt whenever a course loads.
#[derive(Resource)]
pub struct MiniMap {
    pub image: Handle<Image>,
    course: usize,
    /// Rest-pose XZ at the image center, and pixels per meter.
    center: Vec2,
    scale: f32,
    size: Vec2,
}

const MAP_LONG: f32 = 300.0;

pub fn build_minimap(
    mut commands: Commands,
    course: Res<Course>,
    existing: Option<Res<MiniMap>>,
    mut images: ResMut<Assets<Image>>,
) {
    if existing.is_some_and(|m| m.course == course.index) {
        return;
    }
    let samples = &course.track.samples;
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for s in samples {
        let p = Vec2::new(-s.c.x, -s.c.z);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let span = hi - lo;
    let margin = 16.0;
    let scale = (MAP_LONG - 2.0 * margin) / span.max_element();
    let size = (span * scale + Vec2::splat(2.0 * margin)).ceil();
    let center = (lo + hi) * 0.5;
    let (w, h) = (size.x as u32, size.y as u32);
    // Distance from each pixel to the centerline, by stamping discs along it.
    let mut dist = vec![f32::MAX; (w * h) as usize];
    let reach = 9i32;
    let n = samples.len();
    for i in 0..n {
        let (a, b) = (samples[i].c, samples[(i + 1) % n].c);
        for step in 0..3 {
            let p3 = a.lerp(b, step as f32 / 3.0);
            let p = (Vec2::new(-p3.x, -p3.z) - center) * scale + size * 0.5;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let (x, y) = (p.x as i32 + dx, p.y as i32 + dy);
                    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                        continue;
                    }
                    let d = Vec2::new(x as f32 + 0.5, y as f32 + 0.5).distance(p);
                    let cell = &mut dist[(y as u32 * w + x as u32) as usize];
                    *cell = cell.min(d);
                }
            }
        }
    }
    let mut c = Canvas::new(w, h);
    c.fill_in((0, 0, w, h), 0x1a142e, |p| {
        dist[(p.y as u32 * w + p.x as u32) as usize] - 6.5
    });
    c.fill_in((0, 0, w, h), 0xf7f3e8, |p| {
        dist[(p.y as u32 * w + p.x as u32) as usize] - 3.6
    });
    // Start line tick.
    let s0 = &samples[0];
    let mark = |v: Vec3| (Vec2::new(-v.x, -v.z) - center) * scale + size * 0.5;
    let (a, b) = (mark(s0.c - s0.r * 14.0), mark(s0.c + s0.r * 14.0));
    c.fill_in((0, 0, w, h), 0xff5d5d, |p| capsule(p, a, b, 2.2));
    commands.insert_resource(MiniMap {
        image: images.add(c.into_image()),
        course: course.index,
        center,
        scale,
        size,
    });
}

impl MiniMap {
    /// Where a track position falls on the map, as fractions of its size.
    fn locate(&self, course: &Course, u: f32, d: f32) -> Vec2 {
        let samples = &course.track.samples;
        let n = samples.len();
        let i = (u.rem_euclid(n as f32)) as usize % n;
        let f = u.rem_euclid(n as f32).fract();
        let (a, b) = (&samples[i], &samples[(i + 1) % n]);
        let p = a.c.lerp(b.c, f) + a.r * d;
        ((Vec2::new(-p.x, -p.z) - self.center) * self.scale + self.size * 0.5) / self.size
    }
}

// ---------------------------------------------------------------------------
// Widgets
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct HudRoot;

#[derive(Component, Clone, Copy)]
pub enum Hud {
    ItemIcon,
    ItemName,
    ItemCount,
    Lap,
    Time,
    Place,
    Ordinal,
    Center,
    Sub,
    Banner,
    Dot(usize),
}

/// Which kart a widget reports on.
#[derive(Component)]
pub struct HudFor(Entity);

const GOLD: Color = Color::srgb(1.0, 0.82, 0.2);

fn place_color(place: usize) -> Color {
    match place {
        1 => GOLD,
        2 => Color::srgb(0.85, 0.9, 0.95),
        3 => Color::srgb(0.95, 0.6, 0.35),
        _ => Color::WHITE,
    }
}

/// Builds the HUDs for the current views if they are missing.
pub fn sync(
    mut commands: Commands,
    race: Res<Race>,
    views: Res<Views>,
    assets: Res<HudAssets>,
    map: Option<Res<MiniMap>>,
    cams: Query<(Entity, &ViewCam)>,
    karts: Query<&Kart>,
    roots: Query<(), With<HudRoot>>,
) {
    let Some(map) = map else { return };
    if race.phase == Phase::Loading || !roots.is_empty() {
        return;
    }
    let abs = |node: Node| Node {
        position_type: PositionType::Absolute,
        ..node
    };
    for (cam, view) in &cams {
        let Some(&ViewMode::Chase(kart)) = views.modes.get(view.slot) else {
            continue;
        };
        let root = commands
            .spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                UiTargetCamera(cam),
                HudRoot,
                RaceEntity,
            ))
            .id();
        commands.entity(root).with_children(|p| {
            // Item slot.
            p.spawn((
                abs(Node {
                    left: Val::VMin(3.0),
                    top: Val::VMin(3.0),
                    width: Val::VMin(17.0),
                    height: Val::VMin(17.0),
                    border: UiRect::all(Val::VMin(0.7)),
                    border_radius: BorderRadius::all(Val::VMin(3.5)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                }),
                BackgroundColor(Color::srgba(0.08, 0.06, 0.16, 0.55)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.85)),
            ))
            .with_children(|slot| {
                slot.spawn((
                    Node {
                        width: Val::VMin(13.5),
                        height: Val::VMin(13.5),
                        ..default()
                    },
                    ImageNode::from_atlas_image(
                        assets.icons.clone(),
                        TextureAtlas {
                            layout: assets.layout.clone(),
                            index: 7,
                        },
                    ),
                    Visibility::Hidden,
                    Hud::ItemIcon,
                    HudFor(kart),
                ));
                slot.spawn((
                    abs(Node {
                        right: Val::VMin(0.4),
                        bottom: Val::VMin(-0.4),
                        ..default()
                    }),
                    Label::new("", 3.4, Color::WHITE),
                    Hud::ItemCount,
                    HudFor(kart),
                ));
            });
            p.spawn((
                abs(Node {
                    left: Val::VMin(3.0),
                    top: Val::VMin(21.0),
                    ..default()
                }),
                Label::new("", 2.3, Color::WHITE),
                Hud::ItemName,
                HudFor(kart),
            ));
            // Lap and clock.
            p.spawn((
                abs(Node {
                    right: Val::VMin(3.0),
                    top: Val::VMin(2.5),
                    align_items: AlignItems::FlexEnd,
                    ..label_node()
                }),
                Label::new("LAP 1/3", 5.0, Color::WHITE),
                Hud::Lap,
                HudFor(kart),
            ));
            p.spawn((
                abs(Node {
                    right: Val::VMin(3.0),
                    top: Val::VMin(10.0),
                    ..default()
                }),
                Label::new("0:00.00", 2.8, Color::srgb(0.9, 0.95, 1.0)),
                Hud::Time,
                HudFor(kart),
            ));
            // Place.
            p.spawn(abs(Node {
                left: Val::VMin(3.0),
                bottom: Val::VMin(2.0),
                align_items: AlignItems::FlexEnd,
                ..default()
            }))
            .with_children(|row| {
                row.spawn((Label::new("1", 15.0, GOLD), Hud::Place, HudFor(kart)));
                row.spawn((
                    Node {
                        margin: UiRect::bottom(Val::VMin(1.8)),
                        ..default()
                    },
                    Label::new("ST", 5.0, GOLD),
                    Hud::Ordinal,
                    HudFor(kart),
                ));
            });
            // Minimap with a dot per racer.
            let long = 30.0;
            let (mw, mh) = if map.size.x > map.size.y {
                (long, long * map.size.y / map.size.x)
            } else {
                (long * map.size.x / map.size.y, long)
            };
            p.spawn((
                abs(Node {
                    right: Val::VMin(2.5),
                    bottom: Val::VMin(2.5),
                    width: Val::VMin(mw),
                    height: Val::VMin(mh),
                    ..default()
                }),
                ImageNode {
                    color: Color::srgba(1.0, 1.0, 1.0, 0.9),
                    ..ImageNode::new(map.image.clone())
                },
            ))
            .with_children(|m| {
                for k in karts.iter() {
                    let mine = karts.get(kart).is_ok_and(|me| me.index == k.index);
                    let size = if mine { 3.4 } else { 2.2 };
                    m.spawn((
                        abs(Node {
                            width: Val::VMin(size),
                            height: Val::VMin(size),
                            margin: UiRect::all(Val::VMin(-size * 0.5)),
                            border: UiRect::all(Val::VMin(0.4)),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        }),
                        BackgroundColor(hex(COLORS[k.color].1)),
                        BorderColor::all(if mine {
                            Color::WHITE
                        } else {
                            Color::srgb(0.1, 0.08, 0.18)
                        }),
                        ZIndex(if mine { 2 } else { 1 }),
                        Hud::Dot(k.index),
                        HudFor(kart),
                    ));
                }
            });
            // Announcements.
            p.spawn(abs(Node {
                width: percent(100),
                top: percent(24),
                row_gap: Val::VMin(1.0),
                ..label_node()
            }))
            .with_children(|col| {
                col.spawn((
                    label_node(),
                    Label::new("", 17.0, Color::WHITE),
                    Hud::Center,
                    HudFor(kart),
                ));
                col.spawn((label_node(), Label::new("", 5.0, Color::WHITE), Hud::Sub, HudFor(kart)));
            });
            p.spawn(abs(Node {
                width: percent(100),
                top: Val::VMin(15.0),
                ..label_node()
            }))
            .with_children(|col| {
                col.spawn((
                    label_node(),
                    Label::new("", 3.0, Color::srgb(1.0, 0.95, 0.7)),
                    Hud::Banner,
                    HudFor(kart),
                ));
            });
        });
    }
}

/// The largest label size (up to `want`) at which `text` fits across a view.
fn fit(text: &str, want: f32, views: usize) -> f32 {
    // Width of the view in units of its smaller side: two-player panes are tall.
    let across = if views == 2 { 100.0 } else { 177.0 };
    let longest = text.split('\n').map(|l| l.len()).max().unwrap_or(0).max(1) as f32;
    want.min(across * 0.9 * 6.0 / (longest * 5.6))
}

pub fn update(
    time: Res<Time<Real>>,
    views: Res<Views>,
    race: Res<Race>,
    course: Res<Course>,
    map: Option<Res<MiniMap>>,
    karts: Query<&Kart>,
    mut labels: Query<(&Hud, &HudFor, &mut Label)>,
    mut icons: Query<(&Hud, &HudFor, &mut ImageNode, &mut Visibility), Without<Label>>,
    mut dots: Query<(&Hud, &mut Node), (Without<Label>, Without<ImageNode>)>,
) {
    let t = time.elapsed_secs();
    for (hud, owner, mut label) in &mut labels {
        let Ok(k) = karts.get(owner.0) else { continue };
        match hud {
            Hud::Lap => {
                let lap = k.lap.clamp(1, race.laps as i32);
                Label::set(&mut label, format!("LAP {}/{}", lap, race.laps));
            }
            Hud::Time => Label::set(&mut label, format_time(k.finished.unwrap_or(race.time))),
            Hud::Place => {
                Label::set(&mut label, format!("{}", k.place));
                Label::tint(&mut label, place_color(k.place));
            }
            Hud::Ordinal => {
                Label::set(&mut label, ordinal(k.place));
                Label::tint(&mut label, place_color(k.place));
            }
            Hud::ItemName => Label::set(
                &mut label,
                if k.roulette > 0.0 {
                    ""
                } else {
                    k.item.map_or("", Item::name)
                },
            ),
            Hud::ItemCount => Label::set(
                &mut label,
                if k.item_uses > 1 {
                    format!("X{}", k.item_uses)
                } else {
                    String::new()
                },
            ),
            Hud::Banner => {
                let text = race.banner.as_ref().map_or("", |b| b.0.as_str());
                let size = fit(text, 3.0, views.modes.len());
                if label.size != size {
                    label.size = size;
                }
                Label::set(&mut label, text);
            }
            Hud::Center | Hud::Sub => {
                let (mut big, mut small, mut color) = (String::new(), String::new(), Color::WHITE);
                match race.phase {
                    Phase::Intro => {
                        big = COURSES[course.index].name.to_string();
                        small = COURSES[course.index].beast.to_string();
                        color = GOLD;
                    }
                    Phase::Countdown => {
                        big = format!("{}", race.count().max(1));
                        color = Color::srgb(1.0, 0.45, 0.35);
                    }
                    _ => {
                        if let Some(time) = k.finished {
                            big = "FINISH!".into();
                            small = format!("{}{} PLACE  {}", k.place, ordinal(k.place), format_time(time));
                            color = place_color(k.place);
                        } else if race.phase == Phase::Racing && race.timer < 1.0 {
                            big = "GO!".into();
                            color = Color::srgb(0.45, 1.0, 0.5);
                        } else if k.wrong_way > 1.0 {
                            if (t * 3.0).fract() < 0.6 {
                                small = "WRONG WAY".into();
                            }
                            color = Color::srgb(1.0, 0.4, 0.35);
                        } else if k.lap_flash > 0.0 && k.lap > 1 {
                            small = if k.lap == race.laps as i32 {
                                "FINAL LAP!".into()
                            } else {
                                format!("LAP {}", k.lap)
                            };
                            color = GOLD;
                        }
                    }
                }
                let text = if matches!(hud, Hud::Center) { big } else { small };
                // Size the announcement to fit narrow split-screen views.
                let want = match hud {
                    Hud::Center if text.len() > 8 => 9.0,
                    Hud::Center => 17.0,
                    _ => 5.0,
                };
                let size = fit(&text, want, views.modes.len());
                if label.size != size {
                    label.size = size;
                }
                Label::set(&mut label, text);
                Label::tint(&mut label, color);
            }
            _ => {}
        }
    }
    for (hud, owner, mut image, mut vis) in &mut icons {
        if !matches!(hud, Hud::ItemIcon) {
            continue;
        }
        let Ok(k) = karts.get(owner.0) else { continue };
        let index = if k.roulette > 0.0 {
            Some((t * 14.0) as usize % Item::ALL.len())
        } else {
            k.item.map(Item::index)
        };
        *vis = if index.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if let (Some(index), Some(atlas)) = (index, image.texture_atlas.as_mut())
            && atlas.index != index
        {
            atlas.index = index;
        }
    }
    let Some(map) = map else { return };
    for (hud, mut node) in &mut dots {
        let Hud::Dot(index) = hud else { continue };
        let Some(k) = karts.iter().find(|k| k.index == *index) else {
            continue;
        };
        let p = map.locate(&course, k.u, k.d);
        node.left = percent(p.x * 100.0);
        node.top = percent(p.y * 100.0);
    }
}
