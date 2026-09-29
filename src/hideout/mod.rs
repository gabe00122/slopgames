//! Hideout: persistent state (station levels, player-built blocks).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StationKind {
    Workbench,
    AmmoPress,
    Medstation,
}

impl StationKind {
    pub const ALL: [StationKind; 3] = [StationKind::Workbench, StationKind::AmmoPress, StationKind::Medstation];
}

/// Saved hideout state.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct HideoutState {
    #[serde(default)]
    pub levels: Vec<(StationKind, u32)>,
    /// Player block edits relative to the generated hideout: [x, y, z, block id].
    #[serde(default)]
    pub edits: Vec<[i32; 4]>,
}

impl HideoutState {
    pub fn level(&self, s: StationKind) -> u32 {
        self.levels.iter().find(|(k, _)| *k == s).map(|(_, l)| *l).unwrap_or(0)
    }

    pub fn set_level(&mut self, s: StationKind, level: u32) {
        if let Some(e) = self.levels.iter_mut().find(|(k, _)| *k == s) {
            e.1 = level;
        } else {
            self.levels.push((s, level));
        }
    }
}
