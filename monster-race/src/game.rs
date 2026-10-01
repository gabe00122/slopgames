//! Shared game state: which screen we're on, who is playing, and the options
//! chosen for the next race.

use crate::input::Device;
use bevy::prelude::*;

#[derive(States, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum Screen {
    #[default]
    Title,
    /// Players join and pick colors.
    Lobby,
    /// Pick the beast and race options.
    Select,
    Race,
    Results,
}

pub const MAX_PLAYERS: usize = 4;
pub const MAX_RACERS: usize = 8;

/// Kart liveries: name and body color.
pub const COLORS: [(&str, u32); MAX_RACERS] = [
    ("RED", 0xe63946),
    ("BLUE", 0x3a86ff),
    ("YELLOW", 0xffc20e),
    ("GREEN", 0x2dc653),
    ("PURPLE", 0x9d4edd),
    ("ORANGE", 0xfb7a00),
    ("CYAN", 0x12c8d8),
    ("PINK", 0xff5d8f),
];

pub const CPU_NAMES: [&str; MAX_RACERS] = [
    "BARNACLE", "THISTLE", "PEBBLE", "KELP", "BRAMBLE", "MOSS", "DRIFT", "FERN",
];

/// Speed classes: name and top-speed multiplier.
pub const CLASSES: [(&str, f32); 3] = [("STROLL", 0.84), ("GALLOP", 1.0), ("STAMPEDE", 1.17)];

pub const LAP_CHOICES: [u32; 4] = [1, 2, 3, 5];

#[derive(Resource, Clone)]
pub struct Settings {
    /// A Grand Prix runs every beast in turn and keeps score.
    pub grand_prix: bool,
    pub course: usize,
    pub laps: u32,
    /// Total karts on the grid; CPUs fill the places players don't take.
    pub racers: usize,
    pub class: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            grand_prix: false,
            course: 0,
            laps: 3,
            racers: MAX_RACERS,
            class: 1,
        }
    }
}

#[derive(Clone, Copy)]
pub struct PlayerSlot {
    pub device: Device,
    pub color: usize,
    pub ready: bool,
}

/// The humans who have joined, in join order. Slot `i` is "P{i+1}".
#[derive(Resource, Default)]
pub struct Roster {
    pub players: Vec<PlayerSlot>,
}

impl Roster {
    pub fn slot_of(&self, device: Device) -> Option<usize> {
        self.players.iter().position(|p| p.device == device)
    }

    pub fn color_taken(&self, color: usize, except: usize) -> bool {
        self.players
            .iter()
            .enumerate()
            .any(|(i, p)| i != except && p.color == color)
    }

    /// First livery nobody has picked.
    pub fn free_color(&self) -> usize {
        (0..MAX_RACERS).find(|&c| !self.color_taken(c, usize::MAX)).unwrap_or(0)
    }
}

/// One row of the results table.
#[derive(Clone)]
pub struct Standing {
    pub name: String,
    pub color: usize,
    pub human: bool,
    /// Finish time, or `None` for racers still on course when the race ended.
    pub time: Option<f32>,
}

#[derive(Resource, Default)]
pub struct Results {
    pub standings: Vec<Standing>,
}

/// Points for each finishing place in a Grand Prix.
pub const POINTS: [u32; MAX_RACERS] = [10, 8, 6, 5, 4, 3, 2, 1];

#[derive(Clone)]
pub struct CupEntry {
    pub name: String,
    pub color: usize,
    pub human: bool,
    pub points: u32,
    /// Points from the latest race.
    pub last: u32,
}

/// A Grand Prix in progress.
#[derive(Resource, Default)]
pub struct Cup {
    pub active: bool,
    /// Races run so far.
    pub round: usize,
    pub table: Vec<CupEntry>,
}

impl Cup {
    pub fn start() -> Self {
        Cup {
            active: true,
            round: 0,
            table: Vec::new(),
        }
    }

    /// Adds a finished race's standings to the table.
    pub fn score(&mut self, standings: &[Standing]) {
        for entry in &mut self.table {
            entry.last = 0;
        }
        for (place, s) in standings.iter().enumerate() {
            let points = POINTS.get(place).copied().unwrap_or(0);
            match self.table.iter_mut().find(|e| e.name == s.name) {
                Some(e) => {
                    e.points += points;
                    e.last = points;
                }
                None => self.table.push(CupEntry {
                    name: s.name.clone(),
                    color: s.color,
                    human: s.human,
                    points,
                    last: points,
                }),
            }
        }
        self.table.sort_by_key(|e| std::cmp::Reverse(e.points));
        self.round += 1;
    }
}
