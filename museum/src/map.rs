//! A live top-down map: a second camera looking straight down from just
//! below the gallery ceilings, with a marker for the visitor.

use crate::{
    content,
    layout::{self, ROT_APOTHEM},
    player::{Control, Player},
};
use bevy::{
    camera::{Exposure, Hdr, ScalingMode, Viewport, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    window::PrimaryWindow,
};

const MAP_LAYER: usize = 1;
const SPAN: f32 = 122.0;

#[derive(Component)]
pub struct MapCamera;
#[derive(Component)]
pub struct MapMarker;
#[derive(Component)]
pub struct MapLabel(Vec3);
#[derive(Component)]
pub struct MapFrame;

#[derive(Resource, Default)]
pub struct MapOpen(pub bool);

pub fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(MapOpen(false));
    let camera = commands
        .spawn((
            Camera3d::default(),
            Camera {
                order: 5,
                is_active: false,
                clear_color: ClearColorConfig::Custom(Color::srgb(0.05, 0.045, 0.04)),
                ..default()
            },
            Hdr,
            Msaa::Off,
            Tonemapping::AgX,
            Exposure { ev100: 5.0 },
            Projection::Orthographic(OrthographicProjection {
                near: 0.0,
                far: 30.0,
                scaling_mode: ScalingMode::Fixed {
                    width: SPAN,
                    height: SPAN,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_xyz(0.0, 6.8, -13.0).looking_to(Vec3::NEG_Y, Vec3::NEG_Z),
            RenderLayers::from_layers(&[0, MAP_LAYER]),
            MapCamera,
        ))
        .id();
    // The "you are here" arrow, visible only to the map camera.
    let arrow = meshes.add(crate::meshes::prism(
        &[
            Vec2::new(0.0, -1.8),
            Vec2::new(1.1, 1.2),
            Vec2::new(0.0, 0.5),
            Vec2::new(-1.1, 1.2),
        ],
        0.2,
    ));
    let mat = mats.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.3, 0.2),
        emissive: LinearRgba::rgb(40.0, 8.0, 4.0),
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(arrow),
        MeshMaterial3d(mat),
        Transform::default(),
        RenderLayers::layer(MAP_LAYER),
        MapMarker,
    ));
    // Frame + labels
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            border: UiRect::all(px(3)),
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.85, 0.7, 0.4)),
        Visibility::Hidden,
        MapFrame,
    ));
    let galleries = content::galleries();
    let mut label = |text: String, at: Vec3| {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Text::new(text),
            TextFont {
                font: FontSource::Serif,
                font_size: FontSize::Px(17.0),
                weight: FontWeight::BOLD,
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.93, 0.78)),
            TextShadow {
                offset: Vec2::splat(2.0),
                color: Color::BLACK,
            },
            Visibility::Hidden,
            MapLabel(at),
            UiTargetCamera(camera),
        ));
    };
    for g in &galleries {
        let p = layout::side_rot(g.side) * Vec3::new(0.0, 0.0, -(ROT_APOTHEM + g.length * 0.55));
        label(g.name.to_string(), p);
    }
    label("Rotunda".into(), Vec3::new(0.0, 0.0, 0.0));
    label(
        "Entrance".into(),
        layout::side_rot(layout::ENTRANCE_SIDE) * Vec3::new(0.0, 0.0, -(ROT_APOTHEM + layout::VESTIBULE_L + 2.5)),
    );
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, control: Res<Control>, mut open: ResMut<MapOpen>) {
    if keys.just_pressed(KeyCode::KeyM) && control.started {
        open.0 = !open.0;
    }
    if control.paused {
        open.0 = false;
    }
}

pub fn update(
    open: Res<MapOpen>,
    window: Single<&Window, With<PrimaryWindow>>,
    player: Single<(&Transform, &Player), Without<MapMarker>>,
    cam: Single<(&mut Camera, &Camera3d, &GlobalTransform), With<MapCamera>>,
    mut marker: Single<&mut Transform, With<MapMarker>>,
    frame: Single<(&mut Node, &mut Visibility), (With<MapFrame>, Without<MapLabel>)>,
    mut labels: Query<(&mut Node, &mut Visibility, &MapLabel, &ComputedNode), Without<MapFrame>>,
) {
    let (mut camera, _, cam_transform) = cam.into_inner();
    if camera.is_active != open.0 {
        camera.is_active = open.0;
    }
    let (pt, pp) = *player;
    marker.translation = Vec3::new(pt.translation.x, 6.0, pt.translation.z);
    marker.rotation = Quat::from_rotation_y(pp.yaw) * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    marker.scale = Vec3::splat(2.2);

    let (mut fnode, mut fvis) = frame.into_inner();
    let vis = if open.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    fvis.set_if_neq(vis);
    if !open.0 {
        for (_, mut v, _, _) in &mut labels {
            v.set_if_neq(vis);
        }
        return;
    }
    let size = window.physical_size();
    let side = (size.y as f32 * 0.86).min(size.x as f32 * 0.9) as u32;
    let origin = UVec2::new((size.x - side) / 2, (size.y - side) / 2);
    camera.viewport = Some(Viewport {
        physical_position: origin,
        physical_size: UVec2::splat(side),
        ..default()
    });
    let scale = window.scale_factor();
    fnode.left = px(origin.x as f32 / scale - 3.0);
    fnode.top = px(origin.y as f32 / scale - 3.0);
    fnode.width = px(side as f32 / scale + 6.0);
    fnode.height = px(side as f32 / scale + 6.0);

    for (mut node, mut vis, label, computed) in &mut labels {
        vis.set_if_neq(Visibility::Inherited);
        if let Ok(p) = camera.world_to_viewport(cam_transform, label.0) {
            let half = computed.size() * computed.inverse_scale_factor() / 2.0;
            // Labels live in the map camera's UI, which is laid out
            // relative to its viewport.
            let o = origin.as_vec2() / scale;
            node.left = px(p.x - o.x - half.x);
            node.top = px(p.y - o.y - half.y);
        }
    }
}
