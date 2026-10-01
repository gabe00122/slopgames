//! Loading a beast into the world and posing it every frame.

use crate::{
    beast::{AnimOut, Animator, BeastCtl, COURSES, EnvDef, Mat, ShowView},
    env, fauna,
    meshkit::Skin,
    track::Track,
    util::{Rng, damp},
};
use bevy::{
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    math::Affine3A,
    mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    prelude::*,
};

/// The beast currently in the world.
#[derive(Resource)]
pub struct Course {
    pub index: usize,
    pub track: Track,
    pub inv_rest: Vec<Affine3A>,
    pub joints: Vec<Entity>,
    /// World transform of each bone this frame.
    pub global: Vec<Affine3A>,
    /// Skinning matrix of each bone this frame (`global * inverse rest`).
    pub skin: Vec<Affine3A>,
    pub animator: Box<dyn Animator>,
    /// Beast time. Reset at the start of a race so events land at the same
    /// point of every race.
    pub clock: f32,
    pub ctl: BeastCtl,
    pub out: AnimOut,
    pub env: EnvDef,
    pub view: ShowView,
}

/// Request to replace the current beast.
#[derive(Message)]
pub struct LoadCourse(pub usize);

/// Everything that belongs to the loaded course and goes away with it.
#[derive(Component)]
pub struct CourseEntity;

/// Rides a point on the beast: its transform is re-derived from the skeleton
/// every frame.
#[derive(Component)]
pub struct Anchored {
    pub skin: Skin,
    pub rest: Affine3A,
}

/// Shared materials. Nearly everything is vertex-colored, so a handful covers
/// the whole game.
#[derive(Resource)]
pub struct Mats {
    pub matte: Handle<StandardMaterial>,
    pub gloss: Handle<StandardMaterial>,
    pub glow: Handle<StandardMaterial>,
    pub water: Handle<StandardMaterial>,
    pub sea: Handle<StandardMaterial>,
    pub unlit: Handle<StandardMaterial>,
    pub sky: Handle<StandardMaterial>,
    pub foam: Handle<StandardMaterial>,
    pub ghost: Handle<StandardMaterial>,
}

pub fn setup_mats(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mats = Mats {
        matte: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.92,
            reflectance: 0.25,
            ..default()
        }),
        gloss: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.42,
            reflectance: 0.5,
            ..default()
        }),
        glow: materials.add(StandardMaterial {
            base_color: Color::linear_rgb(3.2, 3.2, 3.2),
            unlit: true,
            ..default()
        }),
        water: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.8),
            perceptual_roughness: 0.08,
            reflectance: 0.7,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        sea: materials.add(StandardMaterial {
            base_color: Color::srgba(0.05, 0.43, 0.63, 0.88),
            perceptual_roughness: 0.22,
            reflectance: 0.55,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        unlit: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            ..default()
        }),
        sky: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        }),
        foam: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.7),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        ghost: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.55),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    };
    commands.insert_resource(mats);
}

impl Mats {
    pub fn get(&self, mat: Mat) -> Handle<StandardMaterial> {
        match mat {
            Mat::Matte => self.matte.clone(),
            Mat::Gloss => self.gloss.clone(),
            Mat::Glow => self.glow.clone(),
            Mat::Water => self.water.clone(),
        }
    }
}

#[derive(Component)]
pub struct Joint;

pub fn load_course(
    mut commands: Commands,
    mut requests: MessageReader<LoadCourse>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mats: Res<Mats>,
    args: Res<crate::Args>,
    current: Option<Res<Course>>,
    old: Query<Entity, With<CourseEntity>>,
) {
    let Some(&LoadCourse(index)) = requests.read().last() else {
        return;
    };
    if current.is_some_and(|c| c.index == index) {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    let started = std::time::Instant::now();
    let built = (COURSES[index].build)();

    let joints: Vec<Entity> = built
        .rest
        .iter()
        .map(|rest| {
            commands
                .spawn((Transform::from_matrix(Mat4::from(*rest)), Joint, CourseEntity))
                .id()
        })
        .collect();
    let inv_rest: Vec<Affine3A> = built.rest.iter().map(|r| r.inverse()).collect();
    let inverse_bindposes = bindposes.add(inv_rest.iter().map(|m| Mat4::from(*m)).collect::<Vec<_>>());

    let mut triangles = 0;
    for layer in &built.layers {
        if layer.mesh.is_empty() {
            continue;
        }
        triangles += layer.mesh.triangles();
        let mut e = commands.spawn((
            Mesh3d(meshes.add(layer.mesh.build(layer.flat, true))),
            MeshMaterial3d(mats.get(layer.mat)),
            SkinnedMesh {
                inverse_bindposes: inverse_bindposes.clone(),
                joints: joints.clone(),
            },
            // The beast is always in view, and its bounds move with the skeleton.
            NoFrustumCulling,
            CourseEntity,
        ));
        if matches!(layer.mat, Mat::Glow | Mat::Water) {
            e.insert(NotShadowCaster);
        }
    }

    let mut rng = Rng::new(index as u64 * 77 + 5);
    env::spawn(&mut commands, &mut meshes, &mats, &built.env, &mut rng, args.low);
    fauna::spawn(&mut commands, &mut meshes, &mats, &built);

    let n = built.rest.len();
    let mut course = Course {
        index,
        track: built.track,
        inv_rest,
        joints,
        global: built.rest.clone(),
        skin: vec![Affine3A::IDENTITY; n],
        animator: built.animator,
        clock: 0.0,
        ctl: BeastCtl {
            calm: true,
            ..default()
        },
        out: AnimOut::default(),
        env: built.env,
        view: built.view,
    };
    pose(&mut course, 0.0);
    info!(
        "loaded {} in {:.0} ms: {} bones, {} track samples ({:.0} m), {} triangles",
        COURSES[index].name,
        started.elapsed().as_secs_f32() * 1000.0,
        n,
        course.track.samples.len(),
        course.track.length,
        triangles
    );
    commands.insert_resource(course);
}

fn pose(c: &mut Course, dt: f32) {
    c.out.banner = None;
    c.out.splashes.clear();
    c.animator.pose(c.clock, dt, &c.ctl, &mut c.global, &mut c.out);
    for i in 0..c.global.len() {
        c.skin[i] = c.global[i] * c.inv_rest[i];
    }
    c.track.update(&c.skin);
}

/// Advances the beast, moves its joints and re-poses the track.
pub fn animate(time: Res<Time>, mut course: ResMut<Course>, mut joints: Query<&mut Transform, With<Joint>>) {
    let dt = time.delta_secs();
    let c = &mut *course;
    c.clock += dt;
    c.ctl.shudder = damp(c.ctl.shudder, 0.0, 1.6, dt);
    pose(c, dt);
    for (i, &e) in c.joints.iter().enumerate() {
        if let Ok(mut t) = joints.get_mut(e) {
            *t = Transform::from_matrix(Mat4::from(c.global[i]));
        }
    }
}

pub fn follow_anchors(course: Res<Course>, mut q: Query<(&Anchored, &mut Transform)>) {
    for (a, mut t) in &mut q {
        *t = Transform::from_matrix(Mat4::from(a.skin.apply(&course.skin) * a.rest));
    }
}
