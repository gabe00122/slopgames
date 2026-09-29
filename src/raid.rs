//! An in-progress raid: the map, the player and everything happening in it.

use glam::Vec3;

use crate::player::Player;
use crate::world::gen::{generate_raid_map, MapInfo};
use crate::world::World;

pub struct Raid {
    pub seed: u64,
    pub world: World,
    pub map: MapInfo,
    pub player: Player,
    pub time: f32,
}

impl Raid {
    pub fn new(seed: u64) -> Self {
        let (world, map) = generate_raid_map(seed);
        let spawn = map
            .player_spawns
            .first()
            .copied()
            .unwrap_or(Vec3::new(96.5, 40.0, 96.5));
        // Face the map centre.
        let to_center = Vec3::new(96.0, spawn.y, 96.0) - spawn;
        let yaw = (-to_center.x).atan2(-to_center.z);
        let player = Player::new(spawn, yaw);
        Self {
            seed,
            world,
            map,
            player,
            time: 0.0,
        }
    }
}
