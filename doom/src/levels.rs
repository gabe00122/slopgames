//! Level layouts. One character per map cell; rows may differ in length
//! (missing cells are solid rock). Edit freely — `Map::parse` does the rest.
//!
//! Walls:   `#` brick  `S` stone  `T` tech  `W` wood  `M` marble  `F` flesh
//!          `C` computer  `I` steel support  `H` hell rock  `X` exit switch
//!          (space) solid rock
//! Doors:   `D` door  `R`/`B`/`Y` red/blue/yellow key door  `?` secret door
//! Floors:  `.` tiles  `:` tiles under lights  `'` flickering lights
//!          `,` outdoor dirt (sky)  `~` nukage  `;` nukage under sky
//!          `=` lava (sky)  `_` dark wood room  `+` grating  `-` dark corridor
//!          `r` hell rock (sky)  `h` hell rock indoors  `c` carpet
//! Things:  `P` player start  `z` zombieman  `i` imp  `d` demon  `o` cacodemon
//!          `1` stimpack  `2` medikit  `3` health bonus  `4` soulsphere
//!          `5` green armor  `6` blue armor  `a` clip  `s` shells  `b` box of shells
//!          `q` rocket  `Q` box of rockets  `G` shotgun  `N` chaingun  `L` rocket launcher
//!          `!` `&` `$` red/blue/yellow keycard  `%` barrel  `l` lamp
//!          `t`/`u` red/blue torch  `p` skull pole
//! Things stand on the floor type most common among their neighbours.

use crate::map::LevelDef;

pub const LEVELS: &[LevelDef] = &[
    LevelDef {
        name: "HANGAR",
        start_angle: 0.0,
        rows: &[
            "TTTTTTTTTT      CCCCCCCCCCCCCSSSSSS",
            "T::::::::T      Cl.........1C.....S",
            "T:::::a::T#######..z..:.....C..b..S",
            "T::::::::T------....I...I...C...5.S",
            "T::P:::::D------............C.33..S",
            "T::::::::T------..:...z...:.?.....S",
            "T::::::1:T#######...........CSSSSSS",
            "TTTTTTTTTT      C...I...I...C",
            "                C.....:...z.C",
            "                Ca.........lC",
            "          SSSSSSCCCCCCDCCCCCCSSSSSS",
            "          St,,,,,,,,,,,,,,,,,,,,,tS",
            "WWWWWWWWWWS,,,,,,,,,,,,,,,,,,,,i,,S",
            "W_________S,,,%,,,,,,,,,,,,,,z,,,,STTTTT",
            "W_2____z__S,,,,,,,,,%,,,,,,,,,,,,,S::::T",
            "W_________S,,,,;;;;;,,,,,,,,,,,,,,S::i:T",
            "W_!_______D,,,,;;;;;,,,,,,,,,,,,%,S::::T",
            "W_________S,,,,;;;;;,,,,,,,,,,,,,,R::::X",
            "W__a___z__S,,,,;;;;;,,,,,,SSS,,,,,S::::T",
            "W_________S,,,,;;;;;,,,,,,SSS,,,,,S::::T",
            "WWWWWWWWWWS,,,,,,,,,,%,,,,SSS,,,,,S::::T",
            "          S,,,,,,,,,,,,,,,,,,,,,,,STTTTT",
            "          S,,,,,,,,,z,,,,,,Gs,,,,,S",
            "          S,,,,,,,,,,,,,,,,,,,,,,,S",
            "          S,,i,,,,,,,,,,,,,,,,,i,,S",
            "          Sts,,,,,,,,,,,,,,,,,,,,tS",
            "          SSSSSSSSSSSSSSSSSSSSSSSSS",
            "",
        ],
    },
    LevelDef {
        name: "NUKAGE PROCESSING",
        start_angle: 0.0,
        rows: &[
            "            MMMMMMMMMMMMM          SSSSSSSSSSS",
            "            M2::::::::::M          S...333...S",
            "            M:::::&:::::M          S.6.....4.S",
            "            M:i:::::::i:M          S.........S",
            "            M:::::d:::::M         SSSSSS?SSSSSSS",
            "            M:::::::::::M         St,,,,,,,,,NaS",
            "            M:::::::::::M         S,z,,,,,,,,,,S",
            "            M:::::::::::M         S,,,,,,,,,,,,S",
            "        TTTTTTTTTTTTDTTTTTT       S,,,,,,,,,i,,S",
            "        Tl++++++~~~++++++lT       S,,,,,,,,,,,,S",
            "        T+%+++++~~~+i+++%+T       S,,,;;;;;,,,,S",
            "        T++I++++~~~++++I++T       S,,,;;;;;,,,,S",
            "        T+++++++++++++++++T       S,,,;;;;;,,,,S",
            "        T+++++++~~~+++++++T       S,,,,,,,,,,,,S",
            "TTTTTTTTT+++z+++~~~+++++++T       S,%,,,,,,,,z,S",
            "T:::::::T+++++++~~~+++z+++TWWWWWWWW,,,,,,,,,,,,S",
            "T:a:::5:T+++++s+~~~+++++++T-------W,,,,,,,,,,,,S",
            "T:::::::T+++++++~~~%++++++T-''a''-W,,,,,,o,,,,,S",
            "T::P::::D+++++++~~~+++++++D-''z''-D,,,,,,,,,,,,S",
            "T:::::::T++++++%~~~+++++++T-'''''-W,,,,,,,,,,,,S",
            "T:1:::s:T+++++++~~~+1+++++T-------W,,,,,,,,%,,,S",
            "T:::::::T+++++++~~~++z++++TWWWWWWWW,,,,,,,,,,,,S",
            "TTTTTTTTT++++z++~~~+++++++T       S,,,,;;;;;;,,S",
            "        T+++++++~~~+++++++T       S,,,,;;;;;;,,S",
            "        T+++++++++++++++++T       S,,,,;;;;;;,,S",
            "        T++I++++~~~++++I++T       S,,,,;;;;;;,,S",
            "        T+%+++++~~~+++b+z+T       S,,,,,,,,,,,,S",
            "        Tl++++++~~~++++++lT       S,,i,,,,,,,i,S",
            "        TTTTTTTTTTTTTYTTTTT       S,1,,,,,,,,,,S",
            "          FuhhhhhhhhhhhuF         S,,,,,,,,,,,tS",
            "          FhhhhhhhhhhhhhF         SSSSSSBSSSSSSS",
            "          FhhhhhhhhhhhhhF         W____________W",
            "          FhhhhhhhhhhhhhF         W____________W",
            "          FhhhhhhhhhhhhhF         W__d_________W",
            "          FhhhhhhhhhhhhhF         W_________d__W",
            "          FhhhhhhhhhhhhhF         W_s________$_W",
            "          FuhhhhhhhhhhhuF         W____________W",
            "          FFFFFFFXFFFFFFF         WWWWWWWWWWWWWW",
        ],
    },
    LevelDef {
        name: "HELL GATE",
        start_angle: -1.5707964,
        rows: &[
            "          FFFFFFFFFFFFFFXFFFFFFFFFFFFFF",
            "          FthhhhhhhhhhhhhhhhhhhhhhhhhtF",
            "          FhhhhhhhhhhhhhhhhhhhhhhhhhhhF",
            "          FhhhHHhhhhhhhhohhhhhhhhHHhhhF",
            "          FhhhHHhhhhhhhhhhhhhhhhhHHhhhF",
            "          Fhhhhhhhhh===rrr===hhhhhhhhhF",
            "          Fhhhhhhhhh===rrr===hhhhhhhhhF",
            "          Fhhhhhhhhh===r4r===hhhhhhhhhF",
            "          Fhhhhhhhhh===rrr===hhhhhhhhhF",
            "          FhhhHHhhhhhhhhhhhhhhhhhHHhhhF",
            "          FhhhHHhhhhhhhhhhhhhhhhhHHhhhF",
            "          Fhhdhhhhh%hhhhhhhhh%hhhhhdhhF",
            "          FthhhhhhhhhhhhhhhhhhhhhhhhhtF",
            "      HHHHHHHHHHHHHHHHHHRHHHHHHHHHHHHHHHHHH",
            "      HrrprrrrrrrrrrrrrrrrrrrrrrrrrrrriprrH",
            "      HrrrirrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrH",
            "FFFFFFHrrrrrrrrrrrrrrrrrirrrrr=========rrrH",
            "F2phhhHrrr=======rrrrrrrrrrrrr=========rrrH",
            "FhihhhHrrr=======rrrrrrrrrrrrr====o====rrrHHHHHH",
            "FhhhhhHrrr====o==rrrHHrrrrrrrr=========rrrHhbhQH",
            "FhhdhhHrrr=======rrrHHrrrrrrrr=========rrrHhhihH",
            "FhhhhhHrrr=======rrrrrrrrrrrrrrrrrrrrrrrrrHhhhhH",
            "FhhhhhHrrr=======rrrrrrrrrrrrrrrrrrrrrrrrrHhhhhH",
            "F!hhhhDrrrrrrrrrrrrrrrrr2rrrHHrrrrrrr%rrrrHhhhhH",
            "FhhhhhHrrrrrrrrrrrrrrqrrrrrrHHrrrrrrrrrrrrDhhLhH",
            "FhhhhhHrrrrrrrrrirrrrrrrrrrrrrrrr=======rrHhhhhH",
            "FhhdhhHrrrr%rrrrrr=========rrrrrr=======rrHhhhhH",
            "FhhhhhHrrrrr===rrr=========rrrrrr=======rrHhhhhH",
            "FhhhhhHrrrrr===rrr======o==rrrrrr=======rrHhdhhH",
            "F2phhhHrrrrr===rrr=========rrrrrr=======rrHh6hQH",
            "FFFFFFHrrrrr===rrr=========rrrrrrrrrrrrrrrHHHHHH",
            "      Hrirrr===rrrrrrrrrrrrrrrrrirrrrrrrrrH",
            "      HrrrrrrrrrrrrrrrrrrsrrrrrrrrrrrrrrirH",
            "      HrrprrrrrrrrrrrrrrrrrrrrrrrrrrrrrprrH",
            "      HHHHHHHHHHHHHMMMMMDMMMMMHHHHHHHHHHHHH",
            "                   MtccccccctM",
            "                   McczccczccM",
            "                   McccccccccM",
            "                   MacccccccaM",
            "                   MccccPccccM",
            "                   M1ccccccc1M",
            "                   MMMMMMMMMMM",
        ],
    },
];
