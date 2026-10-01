//! In-world text. Every placard, banner and wall panel is laid out with Bevy
//! UI once, rendered into a single atlas texture, and then mapped onto quads
//! in the museum.

use crate::content::{self, MUSEUM_NAME, WELCOME};
use bevy::{
    asset::RenderAssetUsages,
    camera::RenderTarget,
    math::Affine2,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    text::{FontStyle, LetterSpacing, LineHeight},
};

pub const ATLAS: u32 = 4096;
pub const ATLAS_H: u32 = 5632;
const PLACARD: (u32, u32) = (512, 320);
const BANNER: (u32, u32) = (2048, 256);
const INTRO: (u32, u32) = (1024, 640);
const BANNER_Y: u32 = 1600;
const INTRO_Y: u32 = 2624;
const ELEMENT: u32 = 64;
const ELEMENT_Y: u32 = 3904;
const QUOTE: (u32, u32) = (1024, 384);
const QUOTE_Y: u32 = 4096;

#[derive(Clone, Copy)]
pub enum Cell {
    /// Placard for exhibit `i`.
    Exhibit(usize),
    /// Gilded lettering for gallery `i` (7 = the museum's name).
    Banner(usize),
    /// Introductory wall panel for gallery `i` (7 = the welcome panel).
    Intro(usize),
    /// Periodic table tile for the element with atomic number `i + 1`.
    Element(usize),
    /// Gilded quotation on the wall of gallery `i`.
    Quote(usize),
}

impl Cell {
    fn rect(self) -> URect {
        let (x, y, (w, h)) = match self {
            Cell::Exhibit(i) => {
                let i = i as u32;
                ((i % 8) * PLACARD.0, (i / 8) * PLACARD.1, PLACARD)
            }
            Cell::Banner(i) => {
                let i = i as u32;
                ((i % 2) * BANNER.0, BANNER_Y + (i / 2) * BANNER.1, BANNER)
            }
            Cell::Intro(i) => {
                let i = i as u32;
                ((i % 4) * INTRO.0, INTRO_Y + (i / 4) * INTRO.1, INTRO)
            }
            Cell::Quote(i) => {
                let i = i as u32;
                ((i % 4) * QUOTE.0, QUOTE_Y + (i / 4) * QUOTE.1, QUOTE)
            }
            Cell::Element(i) => {
                let i = i as u32;
                ((i % 64) * ELEMENT, ELEMENT_Y + (i / 64) * ELEMENT, (ELEMENT, ELEMENT))
            }
        };
        URect::new(x, y, x + w, y + h)
    }
}

#[derive(Resource)]
pub struct PlacardAtlas {
    pub image: Handle<Image>,
}

impl PlacardAtlas {
    /// A material showing one atlas cell across a quad's full UV range.
    /// Gilded cells are cut-out metallic lettering.
    pub fn material(&self, mats: &mut Assets<StandardMaterial>, cell: Cell, gilded: bool) -> Handle<StandardMaterial> {
        let r = cell.rect().as_rect();
        let s = Vec2::new(ATLAS as f32, ATLAS_H as f32);
        let uv_transform = Affine2::from_scale_angle_translation(r.size() / s, 0.0, r.min / s);
        let mut m = StandardMaterial {
            base_color_texture: Some(self.image.clone()),
            uv_transform,
            perceptual_roughness: 0.55,
            reflectance: 0.3,
            ..default()
        };
        if gilded {
            // Cut-out gold leaf that catches a little light even in shadow.
            m.alpha_mode = AlphaMode::Mask(0.5);
            m.metallic = 0.85;
            m.perceptual_roughness = 0.3;
            m.emissive = LinearRgba::rgb(0.9, 0.62, 0.25) * 3.0;
            m.emissive_texture = Some(self.image.clone());
        }
        mats.add(m)
    }
}

#[derive(Component)]
pub struct AtlasCamera {
    frames: u32,
}

fn color(hex: u32) -> Color {
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn first_sentence(text: &str) -> String {
    match text.find(". ") {
        Some(i) => text[..=i].to_string(),
        None => text.to_string(),
    }
}

fn font(source: FontSource, size: f32) -> TextFont {
    TextFont {
        font: source,
        font_size: FontSize::Px(size),
        ..default()
    }
}

pub fn setup_atlas(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d {
            width: ATLAS,
            height: ATLAS_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let handle = images.add(image);
    commands.insert_resource(PlacardAtlas { image: handle.clone() });

    let camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: -10,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(handle.into()),
            AtlasCamera { frames: 0 },
        ))
        .id();
    let root = commands
        .spawn((
            Node {
                width: px(ATLAS),
                height: px(ATLAS_H),
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .id();

    let galleries = content::galleries();
    let paper = color(0xf1ebdd);
    let ink = color(0x1e1b18);

    for (i, e) in content::exhibits().iter().enumerate() {
        let r = Cell::Exhibit(i).rect();
        let accent = e.gallery.map(|g| galleries[g].accent).unwrap_or(color(0xc9a55c));
        let accent_ink = accent.darker(0.3);
        let gallery_name = e.gallery.map(|g| galleries[g].name).unwrap_or("The Rotunda");
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(r.min.x),
                top: px(r.min.y),
                width: px(r.width()),
                height: px(r.height()),
                padding: UiRect::axes(px(30), px(26)),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                border: UiRect::all(px(4)),
                ..default()
            },
            BackgroundColor(paper),
            BorderColor::all(accent_ink),
            ChildOf(root),
            children![
                (
                    Text::new(gallery_name.to_uppercase()),
                    font(FontSource::SansSerif, 15.0),
                    LetterSpacing::Px(2.0),
                    TextColor(accent_ink),
                ),
                (
                    Node {
                        width: px(56),
                        height: px(4),
                        ..default()
                    },
                    BackgroundColor(accent)
                ),
                (
                    Text::new(e.title),
                    TextFont {
                        weight: FontWeight::BOLD,
                        ..font(FontSource::Serif, 40.0)
                    },
                    LineHeight::RelativeToFont(1.1),
                    TextColor(ink),
                ),
                (
                    Text::new(e.date),
                    TextFont {
                        style: FontStyle::Italic,
                        ..font(FontSource::Serif, 20.0)
                    },
                    TextColor(color(0x5a4a3a)),
                ),
                (
                    Text::new(e.tagline),
                    font(FontSource::SansSerif, 21.0),
                    TextColor(color(0x3a3530)),
                ),
                (
                    Text::new(first_sentence(e.blurb)),
                    font(FontSource::Serif, 16.0),
                    LineHeight::RelativeToFont(1.3),
                    TextColor(color(0x4a443c)),
                    Node {
                        margin: UiRect::top(px(4)),
                        ..default()
                    },
                ),
            ],
        ));
    }

    let banner = |commands: &mut Commands, i: usize, text: &str| {
        let r = Cell::Banner(i).rect();
        let size = (r.width() as f32 * 0.86 / (text.len() as f32 * 0.78)).min(118.0);
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(r.min.x),
                top: px(r.min.y),
                width: px(r.width()),
                height: px(r.height()),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(root),
            children![(
                Text::new(text.to_uppercase()),
                TextFont {
                    weight: FontWeight::SEMIBOLD,
                    ..font(FontSource::Serif, size)
                },
                LetterSpacing::Px(size * 0.12),
                TextColor(color(0xe6c47c)),
            )],
        ));
    };
    for (i, g) in galleries.iter().enumerate() {
        banner(&mut commands, i, g.name);
    }
    banner(&mut commands, 7, MUSEUM_NAME);

    let intro =
        |commands: &mut Commands, i: usize, eyebrow: &str, title: &str, body: &str, bg: Color, accent: Color| {
            let r = Cell::Intro(i).rect();
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(r.min.x),
                    top: px(r.min.y),
                    width: px(r.width()),
                    height: px(r.height()),
                    padding: UiRect::axes(px(64), px(56)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(18),
                    ..default()
                },
                BackgroundColor(bg),
                ChildOf(root),
                children![
                    (
                        Text::new(eyebrow.to_uppercase()),
                        font(FontSource::SansSerif, 24.0),
                        LetterSpacing::Px(4.0),
                        TextColor(accent),
                    ),
                    (
                        Text::new(title),
                        TextFont {
                            weight: FontWeight::BOLD,
                            ..font(FontSource::Serif, 70.0)
                        },
                        LineHeight::RelativeToFont(1.05),
                        TextColor(color(0xf4efe4)),
                    ),
                    (
                        Node {
                            width: px(90),
                            height: px(5),
                            ..default()
                        },
                        BackgroundColor(accent)
                    ),
                    (
                        Text::new(body),
                        font(FontSource::Serif, 29.0),
                        LineHeight::RelativeToFont(1.4),
                        TextColor(color(0xe4ddd0)),
                    ),
                ],
            ));
        };
    for (i, g) in galleries.iter().enumerate() {
        intro(&mut commands, i, g.era, g.name, g.intro, g.wall.darker(0.12), g.accent);
    }
    intro(
        &mut commands,
        7,
        "Welcome to",
        MUSEUM_NAME,
        WELCOME,
        color(0x1d1a17),
        color(0xd9b464),
    );

    for (i, (quote, who)) in content::QUOTES.iter().enumerate() {
        let r = Cell::Quote(i).rect();
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(r.min.x),
                top: px(r.min.y),
                width: px(r.width()),
                height: px(r.height()),
                padding: UiRect::axes(px(40), px(24)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(18),
                ..default()
            },
            ChildOf(root),
            children![
                (
                    Text::new(format!("\u{201c}{quote}\u{201d}")),
                    TextFont {
                        style: FontStyle::Italic,
                        weight: FontWeight::MEDIUM,
                        ..font(FontSource::Serif, 54.0)
                    },
                    LineHeight::RelativeToFont(1.2),
                    TextLayout::justify(Justify::Center),
                    TextColor(color(0xe6c47c)),
                ),
                (
                    Text::new(who.to_uppercase()),
                    TextFont {
                        weight: FontWeight::SEMIBOLD,
                        ..font(FontSource::SansSerif, 24.0)
                    },
                    LetterSpacing::Px(5.0),
                    TextColor(color(0xe6c47c)),
                ),
            ],
        ));
    }

    for (i, symbol) in crate::models::ELEMENTS.iter().enumerate() {
        let r = Cell::Element(i).rect();
        let z = i as u32 + 1;
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(r.min.x),
                top: px(r.min.y),
                width: px(r.width()),
                height: px(r.height()),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(2)),
                ..default()
            },
            BackgroundColor(crate::models::element_color(z)),
            BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            ChildOf(root),
            children![
                (
                    Text::new(z.to_string()),
                    font(FontSource::SansSerif, 11.0),
                    TextColor(ink)
                ),
                (
                    Text::new(*symbol),
                    TextFont {
                        weight: FontWeight::BOLD,
                        ..font(FontSource::SansSerif, 26.0)
                    },
                    TextColor(ink),
                ),
            ],
        ));
    }
}

/// The atlas only needs to be drawn once; stop its camera after a few frames
/// so it costs nothing afterwards.
pub fn retire_atlas_camera(mut q: Query<(&mut Camera, &mut AtlasCamera)>) {
    for (mut cam, mut atlas) in &mut q {
        if cam.is_active {
            atlas.frames += 1;
            if atlas.frames > 90 {
                cam.is_active = false;
            }
        }
    }
}
