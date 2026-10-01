//! Small, reusable animations that bring the exhibits to life.

use bevy::prelude::*;
use std::f32::consts::TAU;

pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spin,
                swing,
                slide,
                flicker,
                flicker_light,
                blink,
                pulse,
                travel,
                slider_crank,
                bob,
            ),
        );
    }
}

/// Constant rotation about a local axis.
#[derive(Component)]
pub struct Spin {
    pub axis: Vec3,
    pub speed: f32,
}

impl Spin {
    pub fn y(speed: f32) -> Self {
        Spin { axis: Vec3::Y, speed }
    }
    pub fn z(speed: f32) -> Self {
        Spin { axis: Vec3::Z, speed }
    }
}

fn spin(time: Res<Time>, mut q: Query<(&Spin, &mut Transform)>) {
    for (s, mut t) in &mut q {
        t.rotate_local(Quat::from_axis_angle(s.axis, s.speed * time.delta_secs()));
    }
}

/// Sinusoidal rotation about an axis, around a base orientation.
#[derive(Component)]
pub struct Swing {
    pub axis: Vec3,
    pub amplitude: f32,
    pub freq: f32,
    pub phase: f32,
    pub base: Quat,
}

fn swing(time: Res<Time>, mut q: Query<(&Swing, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (s, mut tr) in &mut q {
        let a = s.amplitude * (t * s.freq * TAU + s.phase).sin();
        tr.rotation = s.base * Quat::from_axis_angle(s.axis, a);
    }
}

/// Sinusoidal translation along an axis, around a base position.
#[derive(Component)]
pub struct Slide {
    pub axis: Vec3,
    pub amplitude: f32,
    pub freq: f32,
    pub phase: f32,
    pub base: Vec3,
}

fn slide(time: Res<Time>, mut q: Query<(&Slide, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (s, mut tr) in &mut q {
        tr.translation = s.base + s.axis * s.amplitude * (t * s.freq * TAU + s.phase).sin();
    }
}

/// Gentle vertical float.
#[derive(Component)]
pub struct Bob {
    pub amplitude: f32,
    pub freq: f32,
    pub base: Vec3,
}

fn bob(time: Res<Time>, mut q: Query<(&Bob, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (b, mut tr) in &mut q {
        tr.translation = b.base + Vec3::Y * b.amplitude * (t * b.freq * TAU).sin();
    }
}

/// Noisy scale for flames.
#[derive(Component)]
pub struct Flicker {
    pub base: Vec3,
    pub seed: f32,
}

fn wobble(t: f32, seed: f32) -> f32 {
    ((t * 7.3 + seed).sin() * 0.5 + (t * 13.1 + seed * 2.7).sin() * 0.3 + (t * 23.7 + seed * 5.1).sin() * 0.2) * 0.5
        + 0.5
}

fn flicker(time: Res<Time>, mut q: Query<(&Flicker, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (f, mut tr) in &mut q {
        let w = wobble(t, f.seed);
        tr.scale = f.base * Vec3::new(0.85 + 0.3 * w, 0.7 + 0.6 * w, 0.85 + 0.3 * w);
    }
}

#[derive(Component)]
pub struct FlickerLight {
    pub base: f32,
}

fn flicker_light(time: Res<Time>, mut q: Query<(&FlickerLight, &mut PointLight)>) {
    let t = time.elapsed_secs();
    for (f, mut l) in &mut q {
        l.intensity = f.base * (0.7 + 0.5 * wobble(t * 1.3, 3.0));
    }
}

/// Swaps between two materials, like a panel light blinking.
#[derive(Component)]
pub struct Blink {
    pub on: Handle<StandardMaterial>,
    pub off: Handle<StandardMaterial>,
    pub rate: f32,
    pub seed: u32,
    pub state: bool,
}

fn blink(time: Res<Time>, mut q: Query<(&mut Blink, &mut MeshMaterial3d<StandardMaterial>)>) {
    let tick = (time.elapsed_secs() * 6.0) as u32;
    for (mut b, mut mat) in &mut q {
        let h = (tick.wrapping_mul(2_654_435_761) ^ b.seed.wrapping_mul(40_503)).wrapping_mul(2_246_822_519);
        let on = (h >> 16) % 100 < (b.rate * 100.0) as u32;
        if on != b.state {
            b.state = on;
            mat.0 = if on { b.on.clone() } else { b.off.clone() };
        }
    }
}

/// An expanding ring: grows from 0 to `max` scale over `period` seconds.
#[derive(Component)]
pub struct Pulse {
    pub period: f32,
    pub offset: f32,
    pub max: f32,
}

fn pulse(time: Res<Time>, mut q: Query<(&Pulse, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (p, mut tr) in &mut q {
        let k = ((t + p.offset) / p.period).fract();
        let s = 0.05 + k * p.max;
        tr.scale = Vec3::new(s, 1.0 - k * 0.8, s);
    }
}

/// Moves back and forth between two points (data packets on the web).
#[derive(Component)]
pub struct Travel {
    pub from: Vec3,
    pub to: Vec3,
    pub speed: f32,
    pub offset: f32,
}

fn travel(time: Res<Time>, mut q: Query<(&Travel, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (tv, mut tr) in &mut q {
        let k = (t * tv.speed + tv.offset).fract();
        tr.translation = tv.from.lerp(tv.to, k);
    }
}

/// Slider-crank linkage: a rotating crank drives a rod and a piston that
/// slides along local X. Used by the steam engine and locomotive.
#[derive(Component)]
pub struct SliderCrank {
    pub speed: f32,
    pub crank: f32,
    pub rod: f32,
    /// Crank center in the parent's space.
    pub center: Vec3,
    pub wheel: Entity,
    pub rod_entity: Entity,
    pub piston: Entity,
    pub phase: f32,
}

fn slider_crank(time: Res<Time>, q: Query<&SliderCrank>, mut tr: Query<&mut Transform>) {
    let t = time.elapsed_secs();
    for sc in &q {
        let a = t * sc.speed + sc.phase;
        let pin = sc.center + Vec3::new(a.cos() * sc.crank, a.sin() * sc.crank, 0.0);
        // Piston slides on the line y = center.y, to the -X side of the crank.
        let dy = pin.y - sc.center.y;
        let px = pin.x - (sc.rod * sc.rod - dy * dy).max(0.0).sqrt();
        let piston = Vec3::new(px, sc.center.y, sc.center.z);
        if let Ok(mut w) = tr.get_mut(sc.wheel) {
            w.rotation = Quat::from_rotation_z(a);
        }
        if let Ok(mut r) = tr.get_mut(sc.rod_entity) {
            let d = pin - piston;
            r.translation = (pin + piston) / 2.0 + Vec3::Z * 0.12;
            r.rotation = Quat::from_rotation_z(d.y.atan2(d.x));
        }
        if let Ok(mut p) = tr.get_mut(sc.piston) {
            p.translation = piston;
        }
    }
}
