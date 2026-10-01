//! Shared game types: screens, physics layers, the session and its stats,
//! and the messages systems use to talk to each other.

use avian2d::prelude::*;
use bevy::prelude::*;

/// Downward acceleration, m/s². Heavier than Earth so jumps feel snappy.
pub const GRAVITY: f32 = 20.0;
/// Height of the still water surface everywhere in the swamp.
pub const WATER_Y: f32 = 0.0;
/// The front edge of the ground and of the water, toward the camera. The
/// playfield (every physics body) lives on z = 0.
pub const Z_GROUND_FRONT: f32 = 1.0;
pub const Z_WATER_FRONT: f32 = 1.3;
/// Where the ground's top surface and the water's top surface end.
pub const Z_BACK: f32 = -5.0;

#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Screen {
    /// Before the startup systems have built the shared assets.
    #[default]
    Boot,
    Title,
    Playing,
    Over,
}

#[derive(PhysicsLayer, Clone, Copy, Debug, Default)]
pub enum Layer {
    #[default]
    Ground,
    Player,
    Hunter,
    Prop,
    Boat,
    Bolt,
}

impl Layer {
    /// Things feet can stand on.
    pub const FOOTING: [Layer; 4] = [Layer::Ground, Layer::Prop, Layer::Boat, Layer::Hunter];
}

/// Everything that belongs to the current night; despawned when a new one starts.
#[derive(Component)]
pub struct LevelEntity;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Won,
    Lost,
}

#[derive(Default, Clone, Copy)]
pub struct Stats {
    pub ambushes: u32,
    pub drowned: u32,
    pub thrown: u32,
    pub clawed: u32,
    pub spotted: u32,
    pub boats_flipped: u32,
    pub lanterns_doused: u32,
}

#[derive(Resource)]
pub struct Session {
    pub night: u32,
    pub seed: u64,
    /// Seconds of play this night.
    pub time: f32,
    pub outcome: Option<Outcome>,
    pub hunters_total: u32,
    pub hunters_left: u32,
    pub stats: Stats,
    /// The title screen's attract mode: no player, the camera drifts.
    pub demo: bool,
}

impl Session {
    pub fn live(&self) -> bool {
        self.outcome.is_none()
    }
}

/// The player made a sound hunters within `radius` can hear.
#[derive(Message, Clone, Copy)]
pub struct Noise {
    pub pos: Vec2,
    pub radius: f32,
}

/// Floating text in the world.
#[derive(Message, Clone)]
pub struct Popup {
    pub pos: Vec2,
    pub text: String,
    pub color: u32,
    pub big: bool,
}

impl Popup {
    pub fn new(pos: Vec2, text: impl Into<String>, color: u32, big: bool) -> Self {
        Popup {
            pos,
            text: text.into(),
            color,
            big,
        }
    }
}

/// Something struck the player.
#[derive(Message, Clone, Copy)]
pub struct HurtPlayer {
    pub amount: f32,
    pub from: Vec2,
    pub knock: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HitKind {
    Claw,
    /// A strike the hunter never saw coming: always fatal.
    Ambush,
    Drown,
    Impact,
}

/// Something struck a hunter.
#[derive(Message, Clone, Copy)]
pub struct HitHunter {
    pub entity: Entity,
    pub damage: f32,
    pub kind: HitKind,
    pub knock: Vec2,
}

/// A body hit the water surface.
#[derive(Message, Clone, Copy)]
pub struct Splash {
    pub pos: Vec2,
    /// Roughly the impact speed in m/s, scaled by size.
    pub strength: f32,
}
