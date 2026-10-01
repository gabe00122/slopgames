//! Physical odds and ends: barrels, crates, logs and lily pads that float
//! and tumble; lanterns that swing on ropes and go out in the water; rope
//! bridges that can be cut; docks; springy mushrooms; campfires.

use crate::{
    audio::{Sfx, Sound},
    fx::{Fx, Look},
    game::*,
    level::{Bridge, Dock, PropKind, PropSpawn, Terrain},
    models::{Mats, Models, rot_to},
    water::{Floater, Water},
};
use avian2d::prelude::*;
use bevy::{
    ecs::system::SystemParam,
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};

/// A platform things pass up through from below and land on from above.
/// The bogwight can also drop through by pressing down.
#[derive(Component)]
#[require(ActiveCollisionHooks::MODIFY_CONTACTS)]
pub struct OneWay {
    pub top: f32,
}

#[derive(SystemParam)]
pub struct Hooks<'w, 's> {
    one_way: Query<'w, 's, &'static OneWay>,
    bodies: Query<
        'w,
        's,
        (
            &'static Position,
            Option<&'static crate::player::Bogwight>,
            Has<crate::hunters::Hunter>,
        ),
    >,
}

impl CollisionHooks for Hooks<'_, '_> {
    fn modify_contacts(&self, contacts: &mut ContactPair, _commands: &mut Commands) -> bool {
        let (plat, other) = if let Ok(p) = self.one_way.get(contacts.collider1) {
            (p, contacts.collider2)
        } else if let Ok(p) = self.one_way.get(contacts.collider2) {
            (p, contacts.collider1)
        } else {
            return true;
        };
        let Ok((pos, bw, hunter)) = self.bodies.get(other) else {
            return true;
        };
        if bw.is_some_and(|b| b.dropping > 0.0) {
            return false;
        }
        // Solid only to things whose feet are above the top.
        let half = if bw.is_some() {
            crate::player::R
        } else if hunter {
            crate::hunters::HALF_H
        } else {
            0.25
        };
        pos.0.y - half > plat.top - 0.15
    }
}

/// The bogwight can pick this up and throw it.
#[derive(Component)]
pub struct Grabbable;

/// A light the hunters' eyes benefit from: the bogwight is easier to see near it.
#[derive(Component)]
pub struct LightSource {
    pub strength: f32,
    pub radius: f32,
}

/// Point lights that waver like flames.
#[derive(Component)]
pub struct Flicker {
    pub base: f32,
    pub phase: f32,
}

/// A lantern body (hanging or dropped). Carried lanterns are part of a
/// hunter's rig and use [`CarriedLantern`].
#[derive(Component)]
pub struct Lantern {
    pub lit: bool,
    pub wet: f32,
    pub light: Entity,
    pub glass: Entity,
}

/// A lantern in a hunter's hand.
#[derive(Component)]
pub struct CarriedLantern {
    pub lit: bool,
    pub light: Entity,
    pub glass: Entity,
}

/// A rope drawn between two points on two bodies. If `joint` is set and that
/// joint is gone (cut), the rope goes too.
#[derive(Component)]
pub struct Rope {
    pub a: Entity,
    pub a_off: Vec2,
    pub b: Entity,
    pub b_off: Vec2,
    pub z: f32,
    pub joint: Option<Entity>,
}

#[derive(Component)]
pub struct BridgePlank {
    pub joints: [Option<Entity>; 2],
}

/// Springy: the bogwight bounces off the top.
#[derive(Component)]
pub struct Bouncy {
    pub squash: f32,
    pub cap: Entity,
    pub scale: f32,
}

#[derive(Component)]
pub struct Campfire {
    pub flame: Entity,
    pub embers: f32,
}

/// Thrown by the bogwight: hurts hunters it slams into.
#[derive(Component)]
pub struct Thrown {
    pub t: f32,
    /// Speed last frame, to notice a sudden stop.
    pub speed: f32,
}

pub fn prop_layers() -> CollisionLayers {
    CollisionLayers::new(
        Layer::Prop,
        [
            Layer::Ground,
            Layer::Player,
            Layer::Hunter,
            Layer::Prop,
            Layer::Boat,
            Layer::Bolt,
        ],
    )
}

pub fn spawn_prop(commands: &mut Commands, models: &Models, mats: &Mats, p: &PropSpawn) {
    let tf = Transform::from_xyz(p.pos.x, p.pos.y, 0.0).with_rotation(Quat::from_rotation_z(p.angle));
    let mut e = commands.spawn((
        tf,
        Visibility::default(),
        RigidBody::Dynamic,
        prop_layers(),
        LevelEntity,
    ));
    match p.kind {
        PropKind::Barrel => {
            e.insert((
                Collider::round_rectangle(0.5, 0.64, 0.08),
                Mass(18.0),
                Restitution::new(0.35),
                Friction::new(0.5),
                Floater::boxed(0.66, 0.8, 3, 3, 2.4, Vec2::splat(1.4)),
                Grabbable,
            ))
            .with_child((
                Mesh3d(models.barrel.clone()),
                MeshMaterial3d(mats.matte.clone()),
                Transform::IDENTITY,
            ));
        }
        PropKind::Crate => {
            e.insert((
                Collider::rectangle(0.8, 0.8),
                Mass(22.0),
                Restitution::new(0.15),
                Friction::new(0.6),
                Floater::boxed(0.8, 0.8, 3, 3, 1.8, Vec2::splat(1.8)),
                Grabbable,
            ))
            .with_child((
                Mesh3d(models.crate_.clone()),
                MeshMaterial3d(mats.matte.clone()),
                Transform::IDENTITY,
            ));
        }
        PropKind::Log => {
            e.insert((
                Collider::capsule_endpoints(0.28, Vec2::new(-1.5, 0.0), Vec2::new(1.5, 0.0)),
                Mass(70.0),
                Restitution::new(0.1),
                Friction::new(0.8),
                Floater {
                    angular_drag: 3.0,
                    ..Floater::boxed(3.5, 0.52, 7, 2, 2.2, Vec2::new(0.6, 2.5))
                },
            ))
            .with_child((
                Mesh3d(models.log.clone()),
                MeshMaterial3d(mats.wet.clone()),
                Transform::IDENTITY,
            ));
        }
        PropKind::LilyPad => {
            let flower = (p.pos.x * 7.3).sin() > 0.5;
            e.insert((
                Collider::rectangle(1.1, 0.06),
                Mass(3.0),
                Restitution::new(0.5),
                Friction::new(0.4),
                Floater {
                    splashy: false,
                    ..Floater::boxed(1.1, 0.12, 4, 1, 6.0, Vec2::new(3.0, 8.0))
                },
            ))
            .with_child((
                Mesh3d(models.lily[flower as usize].clone()),
                MeshMaterial3d(mats.wet.clone()),
                Transform::from_xyz(0.0, 0.02, (p.pos.x * 3.1).sin() * 0.3),
                NotShadowCaster,
            ));
        }
    }
}

fn lantern_light(low: bool, strength: f32) -> PointLight {
    PointLight {
        color: Color::srgb(1.0, 0.68, 0.34),
        intensity: 70_000.0 * strength,
        range: 11.0,
        radius: 0.05,
        shadow_maps_enabled: !low,
        ..default()
    }
}

/// The lit parts of a lantern: the glowing glass, a halo, and the light.
/// Returns (light, glass).
pub fn lantern_parts(parent: &mut EntityCommands, models: &Models, mats: &Mats, low: bool) -> (Entity, Entity) {
    let mut light = Entity::PLACEHOLDER;
    let mut glass = Entity::PLACEHOLDER;
    parent.with_children(|c| {
        c.spawn((
            Mesh3d(models.lantern.clone()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::IDENTITY,
        ));
        glass = c
            .spawn((
                Mesh3d(models.lantern_glass.clone()),
                MeshMaterial3d(mats.glow.clone()),
                Transform::IDENTITY,
                NotShadowCaster,
            ))
            .with_child((
                Mesh3d(models.quad.clone()),
                MeshMaterial3d(mats.halo_warm.clone()),
                Transform::from_xyz(0.0, -0.27, 0.12).with_scale(Vec3::splat(0.9)),
                LanternHalo,
                NotShadowCaster,
                NotShadowReceiver,
            ))
            .id();
        light = c
            .spawn((
                lantern_light(low, 1.0),
                Transform::from_xyz(0.0, -0.27, 0.25),
                Flicker {
                    base: 70_000.0,
                    phase: (glass.index_u32() as f32) * 1.7,
                },
            ))
            .id();
    });
    (light, glass)
}

/// Halos face the camera; they're tinted by the lantern's glow.
#[derive(Component)]
pub struct LanternHalo;

pub fn spawn_lantern_post(commands: &mut Commands, models: &Models, mats: &Mats, ground: Vec2, dir: f32, low: bool) {
    let yaw = if dir < 0.0 { std::f32::consts::PI } else { 0.0 };
    commands.spawn((
        Mesh3d(models.lamp_post.clone()),
        MeshMaterial3d(mats.matte.clone()),
        Transform::from_xyz(ground.x, ground.y, -0.35).with_rotation(Quat::from_rotation_y(yaw)),
        LevelEntity,
    ));
    let tip = Vec2::new(ground.x + 0.9 * dir, ground.y + 2.7);
    let anchor = commands
        .spawn((RigidBody::Static, Transform::from_xyz(tip.x, tip.y, 0.0), LevelEntity))
        .id();
    let lantern = spawn_lantern_body(commands, models, mats, tip - Vec2::Y * 0.6, true, low);
    let joint = commands
        .spawn((
            DistanceJoint::new(anchor, lantern)
                .with_local_anchor1(Vec2::ZERO)
                .with_local_anchor2(Vec2::new(0.0, -0.02))
                .with_limits(0.0, 0.6),
            LevelEntity,
        ))
        .id();
    spawn_rope(
        commands,
        models,
        mats,
        anchor,
        Vec2::ZERO,
        lantern,
        Vec2::new(0.0, -0.02),
        -0.0,
        Some(joint),
    );
}

/// A free lantern body, hanging or dropped.
pub fn spawn_lantern_body(
    commands: &mut Commands,
    models: &Models,
    mats: &Mats,
    at: Vec2,
    lit: bool,
    low: bool,
) -> Entity {
    let mut e = commands.spawn((
        Transform::from_xyz(at.x, at.y, 0.0),
        Visibility::default(),
        RigidBody::Dynamic,
        Collider::compound(vec![(
            Vec2::new(0.0, -0.27),
            Rotation::IDENTITY,
            Collider::rectangle(0.2, 0.3),
        )]),
        Mass(3.0),
        Restitution::new(0.3),
        prop_layers(),
        Floater {
            splashy: true,
            ..Floater {
                points: vec![Vec2::new(0.0, -0.36), Vec2::new(0.0, -0.18)],
                cell: 0.09,
                buoyancy: 1.3,
                drag: Vec2::splat(2.0),
                angular_drag: 2.0,
                submerged: 0.0,
                splashy: true,
            }
        },
        Grabbable,
        LightSource {
            strength: 1.0,
            radius: 8.0,
        },
        LevelEntity,
    ));
    let (light, glass) = lantern_parts(&mut e, models, mats, low);
    let id = e.id();
    commands.entity(id).insert(Lantern {
        lit: true,
        wet: 0.0,
        light,
        glass,
    });
    if !lit {
        commands.entity(id).insert(Douse);
    }
    id
}

/// Put out this lantern on the next update.
#[derive(Component)]
pub struct Douse;

pub fn spawn_rope(
    commands: &mut Commands,
    models: &Models,
    mats: &Mats,
    a: Entity,
    a_off: Vec2,
    b: Entity,
    b_off: Vec2,
    z: f32,
    joint: Option<Entity>,
) {
    commands.spawn((
        Mesh3d(models.rope.clone()),
        MeshMaterial3d(mats.matte.clone()),
        Transform::IDENTITY,
        Rope {
            a,
            a_off,
            b,
            b_off,
            z,
            joint,
        },
        NotShadowCaster,
        LevelEntity,
    ));
}

const PLANK_PITCH: f32 = 0.92;

pub fn spawn_bridge(commands: &mut Commands, models: &Models, mats: &Mats, bridge: &Bridge) {
    let (a, b) = (bridge.a, bridge.b);
    let span = a.distance(b);
    // Fit the plank pitch to the span so the bridge is barely slack.
    let n = (span / PLANK_PITCH).round().max(2.0) as usize;
    let pitch = span * 1.006 / n as f32;
    let length = n as f32 * pitch;
    // Sag so the chain's length along a parabola matches.
    let sag = (3.0 * span * (length - span).max(0.0) / 8.0).sqrt();
    let curve = |t: f32| a + (b - a) * t - Vec2::Y * sag * 4.0 * t * (1.0 - t);
    // Walk the curve at equal arc lengths.
    let samples: Vec<Vec2> = (0..=400).map(|i| curve(i as f32 / 400.0)).collect();
    let mut cum = vec![0.0];
    for w in samples.windows(2) {
        let l = cum.last().unwrap() + w[0].distance(w[1]);
        cum.push(l);
    }
    let total = *cum.last().unwrap();
    let at = |s: f32| -> Vec2 {
        let s = s * total / length;
        let i = cum.partition_point(|&c| c < s).clamp(1, samples.len() - 1);
        let (c0, c1) = (cum[i - 1], cum[i]);
        let f = if c1 > c0 { (s - c0) / (c1 - c0) } else { 0.0 };
        samples[i - 1].lerp(samples[i], f)
    };
    let anchor_a = commands
        .spawn((RigidBody::Static, Transform::from_xyz(a.x, a.y, 0.0), LevelEntity))
        .id();
    let anchor_b = commands
        .spawn((RigidBody::Static, Transform::from_xyz(b.x, b.y, 0.0), LevelEntity))
        .id();
    let half = pitch * 0.5;
    let mut prev = anchor_a;
    let mut prev_off = Vec2::ZERO;
    let mut planks = vec![];
    for i in 0..n {
        let p0 = at(i as f32 * pitch);
        let p1 = at((i + 1) as f32 * pitch);
        let mid = (p0 + p1) * 0.5;
        let ang = (p1 - p0).to_angle();
        let plank = commands
            .spawn((
                Transform::from_xyz(mid.x, mid.y, 0.0).with_rotation(Quat::from_rotation_z(ang)),
                Visibility::default(),
                RigidBody::Dynamic,
                Collider::rectangle(0.86, 0.09),
                Mass(6.0),
                Friction::new(0.9),
                prop_layers(),
                Floater {
                    splashy: false,
                    ..Floater::boxed(0.86, 0.1, 2, 1, 1.6, Vec2::splat(2.0))
                },
                LinearDamping(0.3),
                AngularDamping(0.6),
                LevelEntity,
            ))
            .with_child((
                Mesh3d(models.plank.clone()),
                MeshMaterial3d(mats.matte.clone()),
                Transform::IDENTITY,
            ))
            .id();
        let joint = commands
            .spawn((
                RevoluteJoint::new(prev, plank)
                    .with_local_anchor1(prev_off)
                    .with_local_anchor2(Vec2::new(-half, 0.0))
                    .with_point_compliance(0.0),
                JointCollisionDisabled,
                LevelEntity,
            ))
            .id();
        for z in [0.52, -0.52] {
            spawn_rope(
                commands,
                models,
                mats,
                prev,
                prev_off,
                plank,
                Vec2::new(-half + 0.04, 0.0),
                z,
                Some(joint),
            );
        }
        planks.push((plank, joint));
        prev = plank;
        prev_off = Vec2::new(half, 0.0);
    }
    let last = commands
        .spawn((
            RevoluteJoint::new(prev, anchor_b)
                .with_local_anchor1(prev_off)
                .with_local_anchor2(Vec2::ZERO)
                .with_point_compliance(0.0),
            JointCollisionDisabled,
            LevelEntity,
        ))
        .id();
    for z in [0.52, -0.52] {
        spawn_rope(
            commands,
            models,
            mats,
            prev,
            prev_off - Vec2::X * 0.04,
            anchor_b,
            Vec2::ZERO,
            z,
            Some(last),
        );
    }
    for (i, &(plank, joint)) in planks.iter().enumerate() {
        let right = planks.get(i + 1).map_or(last, |p| p.1);
        commands.entity(plank).insert(BridgePlank {
            joints: [Some(joint), Some(right)],
        });
    }
    // Posts at both ends.
    for p in [a, b] {
        commands.spawn((
            Mesh3d(models.post.clone()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::from_xyz(p.x, p.y + 0.9, 0.62).with_scale(Vec3::new(1.0, 1.6, 1.0)),
            LevelEntity,
        ));
        commands.spawn((
            Mesh3d(models.post.clone()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::from_xyz(p.x, p.y + 0.9, -0.62).with_scale(Vec3::new(1.0, 1.6, 1.0)),
            LevelEntity,
        ));
    }
}

pub fn spawn_dock(commands: &mut Commands, models: &Models, mats: &Mats, t: &Terrain, dock: &Dock) {
    let mut x = dock.x0;
    while x < dock.x1 - 0.01 {
        commands.spawn((
            Mesh3d(models.dock_plank.clone()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::from_xyz(x + 0.5, dock.y, 0.0),
            LevelEntity,
        ));
        x += 1.0;
    }
    let mut x = dock.x0 + 0.2;
    while x < dock.x1 {
        let floor = t.height(x);
        let len = dock.y - floor + 0.5;
        for z in [0.72, -1.25] {
            commands.spawn((
                Mesh3d(models.post.clone()),
                MeshMaterial3d(mats.matte.clone()),
                Transform::from_xyz(x, dock.y - 0.1, z).with_scale(Vec3::new(1.0, len, 1.0)),
                LevelEntity,
            ));
        }
        x += 2.2;
    }
    let w = dock.x1 - dock.x0;
    commands.spawn((
        RigidBody::Static,
        Collider::rectangle(w, 0.24),
        Friction::new(0.8),
        OneWay { top: dock.y },
        Transform::from_xyz((dock.x0 + dock.x1) * 0.5, dock.y - 0.12, 0.0),
        CollisionLayers::new(Layer::Ground, LayerMask::ALL),
        LevelEntity,
    ));
}

pub fn spawn_mushroom(commands: &mut Commands, models: &Models, mats: &Mats, ground: Vec2, scale: f32) {
    let s = scale;
    let mut e = commands.spawn((
        Transform::from_xyz(ground.x, ground.y, 0.0),
        Visibility::default(),
        RigidBody::Static,
        Collider::compound(vec![(
            Vec2::new(0.0, 0.95 * s),
            Rotation::IDENTITY,
            Collider::capsule_endpoints(0.14 * s, Vec2::new(-0.5 * s, 0.0), Vec2::new(0.5 * s, 0.0)),
        )]),
        OneWay {
            top: ground.y + 1.09 * s,
        },
        Restitution::new(0.9).with_combine_rule(CoefficientCombine::Max),
        CollisionLayers::new(Layer::Ground, LayerMask::ALL),
        LightSource {
            strength: 0.35,
            radius: 4.0,
        },
        LevelEntity,
    ));
    let mut cap = Entity::PLACEHOLDER;
    e.with_children(|c| {
        cap = c
            .spawn((Transform::from_scale(Vec3::splat(s)), Visibility::default()))
            .with_children(|c| {
                c.spawn((
                    Mesh3d(models.mushroom.clone()),
                    MeshMaterial3d(mats.wet.clone()),
                    Transform::IDENTITY,
                ));
                c.spawn((
                    Mesh3d(models.mushroom_glow.clone()),
                    MeshMaterial3d(mats.glow.clone()),
                    Transform::IDENTITY,
                    NotShadowCaster,
                ));
            })
            .id();
        c.spawn((
            PointLight {
                color: Color::srgb(0.3, 1.0, 0.85),
                intensity: 30_000.0,
                range: 5.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, 0.6 * s, 0.6),
        ));
    });
    e.insert(Bouncy {
        squash: 0.0,
        cap,
        scale: s,
    });
}

pub fn spawn_campfire(commands: &mut Commands, models: &Models, mats: &Mats, ground: Vec2, low: bool) {
    let mut flame = Entity::PLACEHOLDER;
    let mut e = commands.spawn((
        Mesh3d(models.campfire.clone()),
        MeshMaterial3d(mats.matte.clone()),
        Transform::from_xyz(ground.x, ground.y, -1.2),
        LightSource {
            strength: 1.2,
            radius: 10.0,
        },
        LevelEntity,
    ));
    e.with_children(|c| {
        flame = c
            .spawn((
                Mesh3d(models.flame.clone()),
                MeshMaterial3d(mats.glow.clone()),
                Transform::from_xyz(0.0, 0.08, 0.0),
                NotShadowCaster,
            ))
            .id();
        c.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.55, 0.22),
                intensity: 160_000.0,
                range: 14.0,
                radius: 0.3,
                shadow_maps_enabled: !low,
                ..default()
            },
            Transform::from_xyz(0.0, 0.9, 0.3),
            Flicker {
                base: 160_000.0,
                phase: ground.x,
            },
        ));
        c.spawn((
            Mesh3d(models.quad.clone()),
            MeshMaterial3d(mats.halo_warm.clone()),
            Transform::from_xyz(0.0, 0.5, 0.5).with_scale(Vec3::splat(2.6)),
            LanternHalo,
            NotShadowCaster,
            NotShadowReceiver,
        ));
    });
    let id = e.id();
    commands.entity(id).insert(Campfire { flame, embers: 0.0 });
}

pub fn flicker(time: Res<Time>, mut q: Query<(&Flicker, &mut PointLight)>) {
    let t = time.elapsed_secs();
    for (f, mut l) in &mut q {
        if l.intensity <= 0.0 {
            continue;
        }
        let n = (t * 11.0 + f.phase).sin() * 0.06
            + (t * 23.0 + f.phase * 2.3).sin() * 0.05
            + (t * 3.1 + f.phase).sin() * 0.05;
        l.intensity = f.base * (1.0 + n);
    }
}

pub fn campfires(
    time: Res<Time>,
    mut fires: Query<(&GlobalTransform, &mut Campfire)>,
    mut flames: Query<&mut Transform>,
    mut fx: ResMut<Fx>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (gt, mut fire) in &mut fires {
        if let Ok(mut tf) = flames.get_mut(fire.flame) {
            let s = 1.0 + (t * 13.0 + gt.translation().x).sin() * 0.12 + (t * 7.0).sin() * 0.08;
            tf.scale = Vec3::new(1.0 + (t * 9.0).sin() * 0.06, s, 1.0);
        }
        fire.embers += dt;
        if fire.embers > 0.12 {
            fire.embers = 0.0;
            let p = gt.translation() + Vec3::new(fx.rng.sym(0.25), 0.4, fx.rng.sym(0.2));
            let v = Vec3::new(fx.rng.sym(0.4), fx.rng.range(1.0, 2.2), 0.0);
            let life = fx.rng.range(1.0, 2.0);
            fx.emit(Look::Ember, p, v, 0.05, life);
        }
    }
}

/// Lanterns go out when they sink, hiss, and stop lighting the bogwight up.
pub fn lanterns(
    mut commands: Commands,
    time: Res<Time>,
    water: Res<Water>,
    mut q: Query<(Entity, &mut Lantern, &GlobalTransform, Has<Douse>)>,
    mut carried: Query<(&mut CarriedLantern, &GlobalTransform)>,
    mut lights: Query<&mut PointLight>,
    mut vis: Query<&mut Visibility>,
    mut sfx: MessageWriter<Sfx>,
    mut fx: ResMut<Fx>,
    mut session: Option<ResMut<Session>>,
) {
    let dt = time.delta_secs();
    let mut out = |light: Entity, glass: Entity, pos: Vec3, wet: bool, fx: &mut Fx, sfx: &mut MessageWriter<Sfx>| {
        if let Ok(mut l) = lights.get_mut(light) {
            l.intensity = 0.0;
        }
        if let Ok(mut v) = vis.get_mut(glass) {
            *v = Visibility::Hidden;
        }
        if wet {
            sfx.write(Sfx::at(Sound::Sizzle, pos.truncate()));
            for _ in 0..8 {
                let v = Vec3::new(fx.rng.sym(0.6), fx.rng.range(0.6, 1.6), 0.0);
                fx.emit(Look::Steam, pos, v, 0.18, 1.2);
            }
        }
    };
    for (e, mut l, gt, douse) in &mut q {
        if !l.lit {
            continue;
        }
        let p = gt.translation() + Vec3::new(0.0, -0.27, 0.0);
        if douse {
            commands.entity(e).remove::<(Douse, LightSource)>();
            l.lit = false;
            out(l.light, l.glass, p, false, &mut fx, &mut sfx);
            continue;
        }
        if water.depth(p.truncate()) > 0.05 {
            l.wet += dt;
        } else {
            l.wet = 0.0;
        }
        if l.wet > 0.25 {
            l.lit = false;
            commands.entity(e).remove::<LightSource>();
            out(l.light, l.glass, p, true, &mut fx, &mut sfx);
            if let Some(s) = session.as_mut() {
                s.stats.lanterns_doused += 1;
            }
        }
    }
    for (mut c, gt) in &mut carried {
        if !c.lit {
            continue;
        }
        let p = gt.translation() + Vec3::new(0.0, -0.27, 0.0);
        if water.depth(p.truncate()) > 0.05 {
            c.lit = false;
            out(c.light, c.glass, p, true, &mut fx, &mut sfx);
            if let Some(s) = session.as_mut() {
                s.stats.lanterns_doused += 1;
            }
        }
    }
}

/// Smashes a lantern body: glass, sparks, darkness.
pub fn smash_lantern(commands: &mut Commands, e: Entity, pos: Vec2, fx: &mut Fx, sfx: &mut MessageWriter<Sfx>) {
    commands.entity(e).insert(Douse);
    sfx.write(Sfx::at(Sound::Glass, pos));
    for _ in 0..14 {
        let v = Vec3::new(fx.rng.sym(3.0), fx.rng.range(0.5, 4.0), fx.rng.sym(1.0));
        let life = fx.rng.range(0.3, 0.8);
        fx.emit(Look::Spark, pos.extend(0.2), v, 0.05, life);
    }
}

pub fn ropes(
    mut commands: Commands,
    mut ropes: Query<(Entity, &Rope, &mut Transform)>,
    bodies: Query<&GlobalTransform, Without<Rope>>,
    joints: Query<(), Or<(With<RevoluteJoint>, With<DistanceJoint>)>>,
) {
    for (e, rope, mut tf) in &mut ropes {
        if rope.joint.is_some_and(|j| joints.get(j).is_err()) {
            commands.entity(e).despawn();
            continue;
        }
        let (Ok(ga), Ok(gb)) = (bodies.get(rope.a), bodies.get(rope.b)) else {
            commands.entity(e).despawn();
            continue;
        };
        let pa = ga.transform_point(rope.a_off.extend(0.0)).truncate();
        let pb = gb.transform_point(rope.b_off.extend(0.0)).truncate();
        let d = pb - pa;
        let len = d.length().max(0.001);
        tf.translation = pa.extend(rope.z);
        tf.rotation = rot_to(d / len);
        tf.scale = Vec3::new(1.0, len, 1.0);
    }
}

/// Cuts the bridge joint nearest to `at` on this plank.
pub fn cut_plank(
    commands: &mut Commands,
    plank: &mut BridgePlank,
    plank_pos: Vec2,
    plank_rot: &Rotation,
    at: Vec2,
) -> bool {
    let local = plank_rot.inverse() * (at - plank_pos);
    let side = if local.x < 0.0 { 0 } else { 1 };
    let pick = if plank.joints[side].is_some() { side } else { 1 - side };
    if let Some(j) = plank.joints[pick].take() {
        commands.entity(j).despawn();
        return true;
    }
    false
}

pub fn mushrooms(time: Res<Time>, mut q: Query<&mut Bouncy>, mut caps: Query<&mut Transform>) {
    let dt = time.delta_secs();
    for mut b in &mut q {
        b.squash = (b.squash - dt * 2.5).max(0.0);
        if let Ok(mut tf) = caps.get_mut(b.cap) {
            let w = (b.squash * 28.0).sin() * b.squash;
            let s = b.scale;
            tf.scale = Vec3::new(s * (1.0 + w * 0.25), s * (1.0 - w * 0.5), s * (1.0 + w * 0.25));
        }
    }
}

/// Thrown things slamming into hunters.
pub fn thrown(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Thrown, &CollidingEntities, &LinearVelocity)>,
    hunters: Query<(), With<crate::hunters::Hunter>>,
    mut hits: MessageWriter<HitHunter>,
) {
    let dt = time.delta_secs();
    for (e, mut th, touching, v) in &mut q {
        th.t -= dt;
        let speed = v.length();
        let hit_speed = th.speed.max(speed);
        for &other in touching.iter() {
            if other != e && hunters.get(other).is_ok() && hit_speed > 5.0 {
                hits.write(HitHunter {
                    entity: other,
                    damage: 1.0 + (hit_speed - 5.0) / 4.0,
                    kind: HitKind::Impact,
                    knock: v.0 * 0.5,
                });
                th.t = 0.0;
            }
        }
        // A thrown hunter who slams into anything hard gets hurt too.
        let stop = th.speed - speed;
        if hunters.get(e).is_ok() && !touching.is_empty() && stop > 5.0 {
            hits.write(HitHunter {
                entity: e,
                damage: 1.0 + (stop - 5.0) / 3.0,
                kind: HitKind::Impact,
                knock: Vec2::ZERO,
            });
            th.t = 0.0;
        }
        th.speed = speed;
        if th.t <= 0.0 {
            commands.entity(e).remove::<(Thrown, CollidingEntities)>();
        }
    }
}

/// Keeps halos facing the camera whatever their parent is doing.
pub fn halos(
    cam: Query<&GlobalTransform, With<Camera3d>>,
    mut q: Query<(&mut Transform, &GlobalTransform), With<LanternHalo>>,
) {
    let Ok(c) = cam.single() else { return };
    for (mut tf, gt) in &mut q {
        let (_, world_rot, _) = gt.to_scale_rotation_translation();
        let parent = world_rot * tf.rotation.inverse();
        let to_cam = (c.translation() - gt.translation()).normalize_or(Vec3::Z);
        tf.rotation = parent.inverse() * Quat::from_rotation_arc(Vec3::Z, to_cam);
    }
}
