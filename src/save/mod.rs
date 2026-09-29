//! Persistent player profile (stash, equipment, hideout, stats) as JSON.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::hideout::HideoutState;
use crate::inventory::{starter_profile_items, Equipment, Grid};
use crate::raid::{OutcomeKind, RaidOutcome};

pub const SAVE_VERSION: u32 = 1;

fn default_version() -> u32 {
    SAVE_VERSION
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Stats {
    pub raids: u32,
    pub survived: u32,
    pub killed: u32,
    pub mia: u32,
    pub kills: u32,
    /// Total value of loot extracted (roubles).
    pub loot_value: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SavedSettings {
    pub sensitivity: f32,
    pub fov: f32,
}

impl Default for SavedSettings {
    fn default() -> Self {
        Self {
            sensitivity: 0.0022,
            fov: 72.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    #[serde(default = "default_version")]
    pub version: u32,
    pub stash: Grid,
    pub equipment: Equipment,
    #[serde(default)]
    pub hideout: HideoutState,
    #[serde(default)]
    pub stats: Stats,
    #[serde(default)]
    pub settings: SavedSettings,
}

impl Profile {
    pub fn new_player() -> Self {
        let (stash, equipment) = starter_profile_items();
        Self {
            version: SAVE_VERSION,
            stash,
            equipment,
            hideout: HideoutState::default(),
            stats: Stats::default(),
            settings: SavedSettings::default(),
        }
    }
}

/// Fold the result of a raid into the profile: survivors keep everything they
/// carried out; the dead and the missing lose everything they brought in.
pub fn apply_raid_result(profile: &mut Profile, mut equipment: Equipment, outcome: &RaidOutcome) {
    let st = &mut profile.stats;
    st.raids += 1;
    st.kills += outcome.kills;
    match outcome.kind {
        OutcomeKind::Survived(_) => {
            st.survived += 1;
            st.loot_value += outcome.value_out.saturating_sub(outcome.value_in);
        }
        OutcomeKind::Killed(_) => {
            st.killed += 1;
            equipment.strip_on_death();
        }
        OutcomeKind::MissingInAction => {
            st.mia += 1;
            equipment.strip_on_death();
        }
    }
    profile.equipment = equipment;
}

/// Save file location: `$VOXEL_RAID_SAVE` or `voxel_raid_save.json` in the working directory.
pub fn save_path() -> PathBuf {
    std::env::var_os("VOXEL_RAID_SAVE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("voxel_raid_save.json"))
}

/// Load the profile, creating a fresh one if missing. A corrupt save is
/// backed up next to the original rather than overwritten.
pub fn load_or_new(path: &Path) -> (Profile, Option<String>) {
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Profile>(&text) {
            Ok(p) => (p, None),
            Err(e) => {
                let backup = path.with_extension(format!(
                    "corrupt-{}.json",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0)
                ));
                let _ = std::fs::copy(path, &backup);
                (
                    Profile::new_player(),
                    Some(format!(
                        "Save file could not be read ({e}); started a new profile. Old file backed up to {}",
                        backup.display()
                    )),
                )
            }
        },
        Err(_) => (Profile::new_player(), None),
    }
}

/// Atomically write the profile (write to a temp file, then rename).
pub fn save(profile: &Profile, path: &Path) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(profile).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("voxel_raid_test_{}", crate::inventory::new_uid()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("save.json");
        let mut p = Profile::new_player();
        p.stats.raids = 3;
        p.hideout.set_level(crate::hideout::StationKind::Workbench, 2);
        save(&p, &path).unwrap();
        let (loaded, warn) = load_or_new(&path);
        assert!(warn.is_none());
        assert_eq!(loaded, p);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn raid_results_update_profile() {
        use crate::inventory::{BarterKind, GridRef, Item, ItemKind};
        let mut p = Profile::new_player();
        let mut eq = std::mem::take(&mut p.equipment);
        eq.grid_mut(GridRef::Backpack)
            .unwrap()
            .insert(Item::new(ItemKind::Barter(BarterKind::Bitcoin)))
            .unwrap();
        let survived = RaidOutcome {
            kind: OutcomeKind::Survived("Crossroads".into()),
            time: 300.0,
            kills: 2,
            value_in: 100,
            value_out: 400,
        };
        apply_raid_result(&mut p, eq, &survived);
        assert_eq!(p.stats.survived, 1);
        assert_eq!(p.stats.kills, 2);
        assert_eq!(p.stats.loot_value, 300);
        assert_eq!(
            p.equipment
                .grid(GridRef::Backpack)
                .unwrap()
                .count_kind(ItemKind::Barter(BarterKind::Bitcoin)),
            1
        );

        let eq = std::mem::take(&mut p.equipment);
        let died = RaidOutcome {
            kind: OutcomeKind::Killed("Scav".into()),
            ..survived
        };
        apply_raid_result(&mut p, eq, &died);
        assert_eq!(p.stats.killed, 1);
        assert!(p.equipment.primary.is_none());
        assert!(p.equipment.backpack.is_none());
        assert_eq!(p.equipment.total_value(), 0);
        assert_eq!(p.stats.raids, 2);
    }

    #[test]
    fn corrupt_save_is_backed_up() {
        let dir = std::env::temp_dir().join(format!("voxel_raid_test_{}", crate::inventory::new_uid()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("save.json");
        std::fs::write(&path, "{ not json").unwrap();
        let (_, warn) = load_or_new(&path);
        assert!(warn.is_some());
        let backups = std::fs::read_dir(&dir).unwrap().count();
        assert!(backups >= 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
