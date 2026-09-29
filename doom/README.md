# HELLCAST

A raycasting Doom-like written in Rust with **wgpu** (WebGPU) and **bytemuck**.
There are no asset files: every wall texture, sprite, animation frame, the sky,
the status bar face, and the font is generated procedurally at startup.

The only dependencies are `wgpu`, `bytemuck`, `winit` (for the window) and
`pollster` (to block on wgpu's async setup).

## Run

```sh
cargo run --release
```

Click the window to grab the mouse. `Esc` pauses and releases the mouse;
press `Esc` again on the pause screen to quit.

| Action | Keys |
| --- | --- |
| Move / strafe | `W A S D` (or arrow keys to move and turn) |
| Turn | Mouse |
| Run | `Shift` |
| Fire | Left mouse button / `Ctrl` |
| Use (doors, switches) | `E` / `Space` |
| Weapons | `1`–`4`, mouse wheel |
| Automap | `Tab` |
| Mouse sensitivity | `[` / `]` |
| Internal resolution (300/400/600/800 lines) | `F2` |
| Fullscreen | `F11` |

Command-line options:

```
hellcast [--level N] [--god] [--res 0-3] [--novsync]
```

* `--level N` skips the title screen and starts on level N (1–3).
* `--god` makes you invulnerable.
* `--res` picks the internal resolution (0 = 300 lines … 3 = 800 lines).
* `--novsync` renders uncapped.

## The game

Three levels: **Hangar**, **Nukage Processing**, and **Hell Gate**. Each one has
doors, locked key doors, a secret, and an exit switch. You'll find:

* **Weapons:** pistol, shotgun, chaingun, rocket launcher (with splash damage).
* **Monsters:** zombiemen (hitscan), imps (fireballs and claws), demons (melee
  charge), and cacodemons (floating, plasma balls). They wake when they see you
  or hear gunfire, follow a flow field around walls, open doors, flinch, and
  die in several frames.
* **Items:** stimpacks, medikits, health bonuses, soulspheres, green and blue
  armor, ammo, keycards, and barrels that explode in chains.
* **Doom trimmings:** sector lighting with distance falloff, flickering lights,
  dynamic lights from muzzle flashes, fireballs, torches and explosions,
  damaging nukage and lava floors, a sky, Wolfenstein-style sliding doors with
  door jambs, a status bar with the face, an automap, and intermission stats.

## How it renders

Each frame runs three GPU passes (see `src/render.rs` and `src/shaders/`):

1. **Raycast (compute):** `raycast.wgsl` runs one invocation per screen column.
   Each invocation walks the map grid with DDA until it hits a wall or a door
   panel. It writes the distance, texture coordinate, texture, side, and sector
   light into a storage buffer.
2. **Scene (low resolution, 400 lines by default):**
   * `world.wgsl` draws a fullscreen triangle. Pixels inside the column's wall
     span sample the wall texture. The rest inverse-project onto the floor or
     ceiling plane (or sample the sky). Walls write `frag_depth` from their
     distance.
   * World sprites (monsters, items, projectiles, effects) are instanced quads
     in `sprite.wgsl`. They are depth-tested against the walls and alpha-tested.
     Texture alpha between 0.5 and 1 marks "fullbright" texels.
   * The HUD, status bar, menus, and first-person weapon are drawn as more
     quads with alpha blending.
3. **Blit:** `blit.wgsl` upscales to the window with sharp-bilinear filtering,
   then applies the damage and pickup tint and a vignette.

The CPU never touches pixels. It uploads the map as packed `u32` cells, updates
the per-cell door state each frame, and fills a buffer of `Quad` instances.
bytemuck casts the uniforms and instances straight into GPU buffers.

## Project layout

| File | Purpose |
| --- | --- |
| `src/main.rs` | winit event loop, input, headless screenshot mode |
| `src/render.rs` | wgpu setup, pipelines, per-frame passes |
| `src/shaders/*.wgsl` | raycast compute, world, sprite/UI, and blit shaders |
| `src/game.rs` | simulation: player, weapons, AI, projectiles, doors, pickups |
| `src/hud.rs` | status bar, automap, menus, weapon overlay |
| `src/map.rs` | level parsing, collision, CPU raycasts (hitscan, line of sight) |
| `src/levels.rs` | the three maps as ASCII, with the legend in the header |
| `src/textures.rs` | procedural wall, flat, and sky textures |
| `src/sprites.rs` | procedural sprite painter and the sprite catalogue |
| `src/font.rs`, `src/png.rs`, `src/math.rs` | 8x8 font, PNG writer, vector math and RNG |

## Development helpers

```sh
# Render a frame headlessly (no window) to a PNG:
cargo run --release -- --shot out.png --level 2 --pos 10.5,18.5,-20 --frames 30

# Simulate combat and print monster AI state every second:
cargo run --release -- --shot out.png --god --arsenal --weapon 2 --fire --frames 600 --trace

# Dump the generated textures and sprites as contact sheets:
cargo run --release -- --dump-assets assets-preview/
```

Also available: `--size WxH`, `--forward`, `--use`, `--automap`, and `--title`.
Run `--help` for the full list.
