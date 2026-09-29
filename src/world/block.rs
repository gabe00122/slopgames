//! Block types and their material properties.

/// Texture tiles in the procedural atlas. The discriminant is the tile index
/// (row-major in a 16x16 grid of 16px tiles).
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    White = 0,
    GrassTop,
    GrassSide,
    Dirt,
    Stone,
    Sand,
    Gravel,
    Asphalt,
    Concrete,
    Brick,
    Planks,
    LogSide,
    LogTop,
    Leaves,
    Metal,
    Glass,
    Sandbag,
    Bedrock,
    FloorTile,
    RustMetal,
    CrateSide,
    CrateTop,
    WeaponBoxSide,
    WeaponBoxTop,
    MedCaseSide,
    MedCaseTop,
    CabinetSide,
    WorkbenchTop,
    WorkbenchSide,
    AmmoPressTop,
    AmmoPressSide,
    MedstationTop,
    MedstationSide,
    Lamp,
    Plaster,
    StashSide,
    StashTop,
    BarrelSide,
    BarrelTop,
    AsphaltLine,
    GeneratorSide,
    GeneratorTop,
}

impl Tile {
    pub const COUNT: usize = Tile::GeneratorTop as usize + 1;
}

/// Kinds of lootable containers that exist as blocks in the raid map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ContainerKind {
    WoodenCrate,
    WeaponBox,
    MedCase,
    FileCabinet,
}

impl ContainerKind {
    pub fn name(self) -> &'static str {
        match self {
            ContainerKind::WoodenCrate => "Wooden Crate",
            ContainerKind::WeaponBox => "Weapon Box",
            ContainerKind::MedCase => "Medcase",
            ContainerKind::FileCabinet => "Filing Cabinet",
        }
    }

    /// Grid size (w, h) of the container's inventory.
    pub fn grid_size(self) -> (u8, u8) {
        match self {
            ContainerKind::WoodenCrate => (5, 4),
            ContainerKind::WeaponBox => (6, 4),
            ContainerKind::MedCase => (4, 3),
            ContainerKind::FileCabinet => (4, 4),
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Block {
    #[default]
    Air = 0,
    Grass,
    Dirt,
    Stone,
    Sand,
    Gravel,
    Asphalt,
    Concrete,
    Brick,
    Planks,
    Log,
    Leaves,
    Metal,
    Glass,
    Sandbag,
    Bedrock,
    FloorTile,
    RustMetal,
    Crate,
    WeaponBox,
    MedCase,
    Cabinet,
    Workbench,
    AmmoPress,
    Medstation,
    Lamp,
    Plaster,
    StashBox,
    Barrel,
    AsphaltLine,
    Generator,
}

pub struct BlockInfo {
    pub name: &'static str,
    /// Collides with entities.
    pub solid: bool,
    /// Fully hides the faces of neighbours and blocks line of sight.
    pub opaque: bool,
    /// Damage required to destroy the block. `f32::INFINITY` = indestructible.
    pub hp: f32,
    /// Penetration power a bullet must have to pass through one block of this material.
    pub pen_resist: f32,
    /// Texture tiles: [top, side, bottom].
    pub tiles: [Tile; 3],
    /// Rendered at full brightness regardless of lighting.
    pub emissive: bool,
}

const INF: f32 = f32::INFINITY;

const fn info(
    name: &'static str,
    solid: bool,
    opaque: bool,
    hp: f32,
    pen_resist: f32,
    tiles: [Tile; 3],
) -> BlockInfo {
    BlockInfo {
        name,
        solid,
        opaque,
        hp,
        pen_resist,
        tiles,
        emissive: false,
    }
}

const fn all(t: Tile) -> [Tile; 3] {
    [t, t, t]
}

pub const BLOCK_COUNT: usize = Block::Generator as usize + 1;

static BLOCK_INFO: [BlockInfo; BLOCK_COUNT] = [
    info("Air", false, false, 0.0, 0.0, all(Tile::White)),
    info("Grass", true, true, 70.0, 30.0, [Tile::GrassTop, Tile::GrassSide, Tile::Dirt]),
    info("Dirt", true, true, 70.0, 30.0, all(Tile::Dirt)),
    info("Stone", true, true, 400.0, 90.0, all(Tile::Stone)),
    info("Sand", true, true, 50.0, 35.0, all(Tile::Sand)),
    info("Gravel", true, true, 80.0, 40.0, all(Tile::Gravel)),
    info("Asphalt", true, true, 300.0, 80.0, all(Tile::Asphalt)),
    info("Concrete", true, true, 450.0, 90.0, all(Tile::Concrete)),
    info("Brick", true, true, 180.0, 45.0, all(Tile::Brick)),
    info("Wood Planks", true, true, 60.0, 10.0, all(Tile::Planks)),
    info("Log", true, true, 100.0, 25.0, [Tile::LogTop, Tile::LogSide, Tile::LogTop]),
    info("Leaves", true, false, 8.0, 2.0, all(Tile::Leaves)),
    info("Sheet Metal", true, true, 220.0, 30.0, all(Tile::Metal)),
    info("Glass", true, false, 5.0, 1.0, all(Tile::Glass)),
    info("Sandbag", true, true, 200.0, 60.0, all(Tile::Sandbag)),
    info("Bedrock", true, true, INF, 999.0, all(Tile::Bedrock)),
    info("Floor Tile", true, true, 150.0, 50.0, all(Tile::FloorTile)),
    info("Rusted Metal", true, true, 150.0, 28.0, all(Tile::RustMetal)),
    info("Wooden Crate", true, true, INF, 999.0, [Tile::CrateTop, Tile::CrateSide, Tile::CrateTop]),
    info("Weapon Box", true, true, INF, 999.0, [Tile::WeaponBoxTop, Tile::WeaponBoxSide, Tile::WeaponBoxSide]),
    info("Medcase", true, true, INF, 999.0, [Tile::MedCaseTop, Tile::MedCaseSide, Tile::MedCaseSide]),
    info("Filing Cabinet", true, true, INF, 999.0, [Tile::CabinetSide, Tile::CabinetSide, Tile::CabinetSide]),
    info("Workbench", true, true, INF, 999.0, [Tile::WorkbenchTop, Tile::WorkbenchSide, Tile::WorkbenchSide]),
    info("Ammo Press", true, true, INF, 999.0, [Tile::AmmoPressTop, Tile::AmmoPressSide, Tile::AmmoPressSide]),
    info("Medstation", true, true, INF, 999.0, [Tile::MedstationTop, Tile::MedstationSide, Tile::MedstationSide]),
    BlockInfo {
        name: "Lamp",
        solid: true,
        opaque: true,
        hp: 10.0,
        pen_resist: 5.0,
        tiles: all(Tile::Lamp),
        emissive: true,
    },
    info("Plaster Wall", true, true, 100.0, 20.0, all(Tile::Plaster)),
    info("Stash", true, true, INF, 999.0, [Tile::StashTop, Tile::StashSide, Tile::StashSide]),
    info("Oil Barrel", true, true, 120.0, 35.0, [Tile::BarrelTop, Tile::BarrelSide, Tile::BarrelTop]),
    info("Road Marking", true, true, 300.0, 80.0, all(Tile::AsphaltLine)),
    info("Generator", true, true, INF, 999.0, [Tile::GeneratorTop, Tile::GeneratorSide, Tile::GeneratorSide]),
];

static ALL_BLOCKS: [Block; BLOCK_COUNT] = [
    Block::Air,
    Block::Grass,
    Block::Dirt,
    Block::Stone,
    Block::Sand,
    Block::Gravel,
    Block::Asphalt,
    Block::Concrete,
    Block::Brick,
    Block::Planks,
    Block::Log,
    Block::Leaves,
    Block::Metal,
    Block::Glass,
    Block::Sandbag,
    Block::Bedrock,
    Block::FloorTile,
    Block::RustMetal,
    Block::Crate,
    Block::WeaponBox,
    Block::MedCase,
    Block::Cabinet,
    Block::Workbench,
    Block::AmmoPress,
    Block::Medstation,
    Block::Lamp,
    Block::Plaster,
    Block::StashBox,
    Block::Barrel,
    Block::AsphaltLine,
    Block::Generator,
];

impl Block {
    #[inline]
    pub fn from_u8(v: u8) -> Block {
        ALL_BLOCKS.get(v as usize).copied().unwrap_or(Block::Air)
    }

    #[inline]
    pub fn info(self) -> &'static BlockInfo {
        &BLOCK_INFO[self as usize]
    }

    #[inline]
    pub fn is_air(self) -> bool {
        self == Block::Air
    }

    #[inline]
    pub fn is_solid(self) -> bool {
        self.info().solid
    }

    #[inline]
    pub fn is_opaque(self) -> bool {
        self.info().opaque
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }

    /// Does this block stop AI line of sight? Glass is see-through; foliage hides you.
    pub fn blocks_vision(self) -> bool {
        match self {
            Block::Air | Block::Glass => false,
            Block::Leaves => true,
            b => b.is_opaque(),
        }
    }

    pub fn container(self) -> Option<ContainerKind> {
        match self {
            Block::Crate => Some(ContainerKind::WoodenCrate),
            Block::WeaponBox => Some(ContainerKind::WeaponBox),
            Block::MedCase => Some(ContainerKind::MedCase),
            Block::Cabinet => Some(ContainerKind::FileCabinet),
            _ => None,
        }
    }

    pub fn indestructible(self) -> bool {
        !self.info().hp.is_finite()
    }

    /// Blocks the player may place in the hideout build mode.
    pub fn buildable() -> &'static [Block] {
        &[
            Block::Planks,
            Block::Brick,
            Block::Concrete,
            Block::Metal,
            Block::Glass,
            Block::Sandbag,
            Block::FloorTile,
            Block::Lamp,
            Block::Plaster,
            Block::Log,
            Block::Stone,
            Block::Barrel,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_table_matches_enum() {
        for (i, b) in ALL_BLOCKS.iter().enumerate() {
            assert_eq!(*b as usize, i);
            assert_eq!(Block::from_u8(i as u8), *b);
        }
        assert_eq!(Block::Air.info().name, "Air");
        assert_eq!(Block::Generator.info().name, "Generator");
    }

    #[test]
    fn weaker_blocks_resist_less() {
        assert!(Block::Glass.info().pen_resist < Block::Planks.info().pen_resist);
        assert!(Block::Planks.info().pen_resist < Block::Brick.info().pen_resist);
        assert!(Block::Brick.info().pen_resist < Block::Concrete.info().pen_resist);
    }
}
