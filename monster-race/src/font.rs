//! The game's lettering: a chunky stroke font drawn into a texture atlas at
//! startup, so the game ships no font files and looks the same everywhere.
//!
//! [`Label`] is the text widget. It lays glyph images out in a row and
//! rebuilds them when its text changes.

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageSampler, TextureAtlas, TextureAtlasLayout},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

const UNIT: f32 = 12.0;
/// Atlas cell size in pixels. Glyphs are 4 × 6 units plus padding.
const CELL_W: u32 = 104;
const CELL_H: u32 = 112;
const PAD_X: f32 = 16.0;
const PAD_Y: f32 = 24.0;
const STROKE: f32 = 0.58;
const OUTLINE: f32 = 0.42;
const SLANT: f32 = 0.16;
const COLS: u32 = 10;

type Stroke = &'static [(f32, f32)];

/// `(character, advance in units, strokes)`.
const GLYPHS: &[(char, f32, &[Stroke])] = &[
    (
        'A',
        4.0,
        &[
            &[(0., 0.), (0., 4.), (2., 6.), (4., 4.), (4., 0.)],
            &[(0., 2.2), (4., 2.2)],
        ],
    ),
    (
        'B',
        4.0,
        &[
            &[(0., 0.), (0., 6.), (3., 6.), (4., 5.), (4., 4.), (3., 3.), (0., 3.)],
            &[(3., 3.), (4., 2.), (4., 1.), (3., 0.), (0., 0.)],
        ],
    ),
    (
        'C',
        4.0,
        &[&[
            (4., 5.),
            (3., 6.),
            (1., 6.),
            (0., 5.),
            (0., 1.),
            (1., 0.),
            (3., 0.),
            (4., 1.),
        ]],
    ),
    (
        'D',
        4.0,
        &[&[(0., 0.), (0., 6.), (2.5, 6.), (4., 4.5), (4., 1.5), (2.5, 0.), (0., 0.)]],
    ),
    (
        'E',
        3.6,
        &[&[(3.6, 6.), (0., 6.), (0., 0.), (3.6, 0.)], &[(0., 3.), (2.8, 3.)]],
    ),
    ('F', 3.6, &[&[(3.6, 6.), (0., 6.), (0., 0.)], &[(0., 3.), (2.8, 3.)]]),
    (
        'G',
        4.0,
        &[&[
            (4., 5.),
            (3., 6.),
            (1., 6.),
            (0., 5.),
            (0., 1.),
            (1., 0.),
            (3., 0.),
            (4., 1.),
            (4., 3.),
            (2.2, 3.),
        ]],
    ),
    (
        'H',
        4.0,
        &[&[(0., 0.), (0., 6.)], &[(4., 0.), (4., 6.)], &[(0., 3.), (4., 3.)]],
    ),
    ('I', 1.6, &[&[(0.8, 0.), (0.8, 6.)]]),
    ('J', 3.6, &[&[(3.6, 6.), (3.6, 1.), (2.6, 0.), (1., 0.), (0., 1.)]]),
    (
        'K',
        4.0,
        &[&[(0., 0.), (0., 6.)], &[(4., 6.), (0., 2.6)], &[(1.5, 3.7), (4., 0.)]],
    ),
    ('L', 3.4, &[&[(0., 6.), (0., 0.), (3.4, 0.)]]),
    ('M', 4.6, &[&[(0., 0.), (0., 6.), (2.3, 2.8), (4.6, 6.), (4.6, 0.)]]),
    ('N', 4.0, &[&[(0., 0.), (0., 6.), (4., 0.), (4., 6.)]]),
    (
        'O',
        4.0,
        &[&[
            (1., 0.),
            (3., 0.),
            (4., 1.),
            (4., 5.),
            (3., 6.),
            (1., 6.),
            (0., 5.),
            (0., 1.),
            (1., 0.),
        ]],
    ),
    (
        'P',
        4.0,
        &[&[(0., 0.), (0., 6.), (3., 6.), (4., 5.), (4., 4.), (3., 3.), (0., 3.)]],
    ),
    (
        'Q',
        4.0,
        &[
            &[
                (1., 0.),
                (3., 0.),
                (4., 1.),
                (4., 5.),
                (3., 6.),
                (1., 6.),
                (0., 5.),
                (0., 1.),
                (1., 0.),
            ],
            &[(2.4, 1.6), (4.2, -0.4)],
        ],
    ),
    (
        'R',
        4.0,
        &[
            &[(0., 0.), (0., 6.), (3., 6.), (4., 5.), (4., 4.), (3., 3.), (0., 3.)],
            &[(2., 3.), (4., 0.)],
        ],
    ),
    (
        'S',
        4.0,
        &[&[
            (4., 5.),
            (3., 6.),
            (1., 6.),
            (0., 5.),
            (0., 4.),
            (1., 3.),
            (3., 3.),
            (4., 2.),
            (4., 1.),
            (3., 0.),
            (1., 0.),
            (0., 1.),
        ]],
    ),
    ('T', 4.0, &[&[(0., 6.), (4., 6.)], &[(2., 6.), (2., 0.)]]),
    (
        'U',
        4.0,
        &[&[(0., 6.), (0., 1.), (1., 0.), (3., 0.), (4., 1.), (4., 6.)]],
    ),
    ('V', 4.0, &[&[(0., 6.), (2., 0.), (4., 6.)]]),
    ('W', 5.0, &[&[(0., 6.), (1.2, 0.), (2.5, 3.6), (3.8, 0.), (5., 6.)]]),
    ('X', 4.0, &[&[(0., 0.), (4., 6.)], &[(0., 6.), (4., 0.)]]),
    ('Y', 4.0, &[&[(0., 6.), (2., 3.), (4., 6.)], &[(2., 3.), (2., 0.)]]),
    ('Z', 4.0, &[&[(0., 6.), (4., 6.), (0., 0.), (4., 0.)]]),
    (
        '0',
        4.0,
        &[
            &[
                (1., 0.),
                (3., 0.),
                (4., 1.),
                (4., 5.),
                (3., 6.),
                (1., 6.),
                (0., 5.),
                (0., 1.),
                (1., 0.),
            ],
            &[(1.5, 2.2), (2.5, 3.8)],
        ],
    ),
    ('1', 2.6, &[&[(0., 4.8), (1.4, 6.), (1.4, 0.)], &[(0.2, 0.), (2.6, 0.)]]),
    (
        '2',
        4.0,
        &[&[(0., 5.), (1., 6.), (3., 6.), (4., 5.), (4., 3.8), (0., 0.), (4., 0.)]],
    ),
    (
        '3',
        4.0,
        &[
            &[(0., 5.), (1., 6.), (3., 6.), (4., 5.), (4., 4.), (3., 3.), (1.6, 3.)],
            &[(3., 3.), (4., 2.), (4., 1.), (3., 0.), (1., 0.), (0., 1.)],
        ],
    ),
    ('4', 4.0, &[&[(3., 0.), (3., 6.), (0., 2.), (4., 2.)]]),
    (
        '5',
        4.0,
        &[&[
            (4., 6.),
            (0., 6.),
            (0., 3.5),
            (3., 3.5),
            (4., 2.5),
            (4., 1.),
            (3., 0.),
            (1., 0.),
            (0., 1.),
        ]],
    ),
    (
        '6',
        4.0,
        &[&[
            (4., 5.),
            (3., 6.),
            (1., 6.),
            (0., 5.),
            (0., 1.),
            (1., 0.),
            (3., 0.),
            (4., 1.),
            (4., 2.5),
            (3., 3.5),
            (0., 3.5),
        ]],
    ),
    ('7', 4.0, &[&[(0., 6.), (4., 6.), (1.4, 0.)]]),
    (
        '8',
        4.0,
        &[&[
            (1., 3.),
            (0., 4.),
            (0., 5.),
            (1., 6.),
            (3., 6.),
            (4., 5.),
            (4., 4.),
            (3., 3.),
            (1., 3.),
            (0., 2.),
            (0., 1.),
            (1., 0.),
            (3., 0.),
            (4., 1.),
            (4., 2.),
            (3., 3.),
        ]],
    ),
    (
        '9',
        4.0,
        &[&[
            (0., 1.),
            (1., 0.),
            (3., 0.),
            (4., 1.),
            (4., 5.),
            (3., 6.),
            (1., 6.),
            (0., 5.),
            (0., 3.5),
            (1., 2.5),
            (4., 2.5),
        ]],
    ),
    ('.', 1.0, &[&[(0.5, 0.), (0.5, 0.05)]]),
    (',', 1.0, &[&[(0.6, 0.2), (0.2, -0.9)]]),
    ('!', 1.0, &[&[(0.5, 6.), (0.5, 2.)], &[(0.5, 0.), (0.5, 0.05)]]),
    (
        '?',
        4.0,
        &[
            &[(0., 5.), (1., 6.), (3., 6.), (4., 5.), (4., 4.), (2., 2.6), (2., 2.)],
            &[(2., 0.), (2., 0.05)],
        ],
    ),
    (':', 1.0, &[&[(0.5, 1.2), (0.5, 1.25)], &[(0.5, 4.2), (0.5, 4.25)]]),
    ('-', 3.0, &[&[(0.3, 3.), (2.7, 3.)]]),
    ('/', 3.4, &[&[(0., 0.), (3.4, 6.)]]),
    ('\'', 1.0, &[&[(0.5, 6.), (0.5, 4.6)]]),
    ('+', 3.4, &[&[(0.2, 3.), (3.2, 3.)], &[(1.7, 1.5), (1.7, 4.5)]]),
    ('<', 3.0, &[&[(2.8, 5.6), (0.2, 3.), (2.8, 0.4)]]),
    ('>', 3.0, &[&[(0.2, 5.6), (2.8, 3.), (0.2, 0.4)]]),
    (
        '%',
        4.0,
        &[
            &[(0., 0.), (4., 6.)],
            &[(0.6, 5.2), (0.6, 5.25)],
            &[(3.4, 0.8), (3.4, 0.85)],
        ],
    ),
    (
        '*',
        3.0,
        &[
            &[(1.5, 1.4), (1.5, 4.6)],
            &[(0.1, 2.2), (2.9, 3.8)],
            &[(0.1, 3.8), (2.9, 2.2)],
        ],
    ),
];

const SPACE: f32 = 2.4;
const GAP: f32 = 1.55;

#[derive(Resource)]
pub struct GameFont {
    image: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
}

fn glyph_index(c: char) -> Option<usize> {
    GLYPHS.iter().position(|g| g.0 == c.to_ascii_uppercase())
}

fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

pub fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let rows = (GLYPHS.len() as u32).div_ceil(COLS);
    let (w, h) = (CELL_W * COLS, CELL_H * rows);
    let mut data = vec![0u8; (w * h * 4) as usize];
    for (i, glyph) in GLYPHS.iter().enumerate() {
        let (cx, cy) = ((i as u32 % COLS) * CELL_W, (i as u32 / COLS) * CELL_H);
        let segs: Vec<(Vec2, Vec2)> = glyph
            .2
            .iter()
            .flat_map(|s| {
                s.windows(2)
                    .map(|w| (Vec2::new(w[0].0, w[0].1), Vec2::new(w[1].0, w[1].1)))
            })
            .collect();
        for py in 0..CELL_H {
            for px in 0..CELL_W {
                // Pixel to glyph units; y grows upward, and the glyph slants right.
                let gy = (CELL_H as f32 - PAD_Y - py as f32 - 0.5) / UNIT;
                let gx = (px as f32 + 0.5 - PAD_X) / UNIT - gy * SLANT;
                let p = Vec2::new(gx, gy);
                let d = segs.iter().map(|&(a, b)| seg_dist(p, a, b)).fold(f32::MAX, f32::min);
                let aa = 0.75 / UNIT;
                let fill = 1.0 - ((d - STROKE) / aa + 0.5).clamp(0.0, 1.0);
                let alpha = 1.0 - ((d - STROKE - OUTLINE) / aa + 0.5).clamp(0.0, 1.0);
                if alpha <= 0.0 {
                    continue;
                }
                let o = (((cy + py) * w + cx + px) * 4) as usize;
                let ink = [26.0, 20.0, 46.0];
                for c in 0..3 {
                    data[o + c] = (ink[c] + (255.0 - ink[c]) * fill) as u8;
                }
                data[o + 3] = (alpha * 255.0) as u8;
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    commands.insert_resource(GameFont {
        image: images.add(image),
        layout: layouts.add(TextureAtlasLayout::from_grid(
            UVec2::new(CELL_W, CELL_H),
            COLS,
            rows,
            None,
            None,
        )),
    });
}

/// A line (or several, split on `\n`) of game lettering. `size` is the cap
/// height as a percentage of the smaller side of the view it is drawn in.
#[derive(Component, Clone)]
#[require(Node)]
pub struct Label {
    pub text: String,
    pub size: f32,
    pub color: Color,
}

impl Label {
    pub fn new(text: impl Into<String>, size: f32, color: Color) -> Self {
        Label {
            text: text.into(),
            size,
            color,
        }
    }

    /// Updates the text, touching the component only if it changed.
    pub fn set(this: &mut Mut<Label>, text: impl AsRef<str>) {
        if this.text != text.as_ref() {
            this.text = text.as_ref().to_string();
        }
    }

    pub fn tint(this: &mut Mut<Label>, color: Color) {
        if this.color != color {
            this.color = color;
        }
    }
}

/// Node style for a label container: a centered column of rows.
pub fn label_node() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        ..default()
    }
}

/// Rebuilds the glyph images of labels whose text, size or color changed.
pub fn render_labels(
    mut commands: Commands,
    font: Res<GameFont>,
    mut labels: Query<(Entity, &Label, &mut Node), Changed<Label>>,
) {
    for (entity, label, mut node) in &mut labels {
        node.flex_direction = FlexDirection::Column;
        commands.entity(entity).despawn_related::<Children>();
        // One unit of the glyph grid, in vmin.
        let unit = label.size / 6.0;
        let px = unit / UNIT;
        let cell_h = CELL_H as f32 * px;
        let cell_w = CELL_W as f32 * px;
        let lines: Vec<&str> = label.text.split('\n').collect();
        let line_width = |line: &str| -> f32 {
            line.chars()
                .map(|c| glyph_index(c).map_or(SPACE, |i| GLYPHS[i].1 + GAP) * unit)
                .sum()
        };
        let row_h = cell_h * if lines.len() > 1 { 0.95 } else { 0.8 };
        node.width = Val::VMin(lines.iter().map(|l| line_width(l)).fold(0.0, f32::max));
        node.height = Val::VMin(row_h * lines.len() as f32);
        node.flex_shrink = 0.0;
        commands.entity(entity).with_children(|parent| {
            for line in lines {
                parent
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        width: Val::VMin(line_width(line)),
                        height: Val::VMin(row_h),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|row| {
                        for c in line.chars() {
                            let Some(index) = glyph_index(c) else {
                                row.spawn(Node {
                                    width: Val::VMin(SPACE * unit),
                                    ..default()
                                });
                                continue;
                            };
                            // Cells are wider than the advance; pull neighbours in over the padding.
                            let left = PAD_X * px - GAP * unit * 0.5;
                            let right = cell_w - PAD_X * px - (GLYPHS[index].1 + GAP * 0.5) * unit;
                            row.spawn((
                                Node {
                                    width: Val::VMin(cell_w),
                                    height: Val::VMin(cell_h),
                                    margin: UiRect {
                                        left: Val::VMin(-left),
                                        right: Val::VMin(-right),
                                        ..default()
                                    },
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                ImageNode {
                                    color: label.color,
                                    ..ImageNode::from_atlas_image(
                                        font.image.clone(),
                                        TextureAtlas {
                                            layout: font.layout.clone(),
                                            index,
                                        },
                                    )
                                },
                            ));
                        }
                    });
            }
        });
    }
}
