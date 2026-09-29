# Voxel Raid

A Minecraft / Escape-from-Tarkov hybrid written in Rust: blocky, fully destructible
voxel maps combined with a hardcore extraction-shooter loop. Gear up from your stash,
raid a procedurally generated town full of hostile scavs, loot crates and bodies,
and reach an extraction point before you die. Everything you bring in is lost if you
don't make it out. Back home, upgrade workbenches in your buildable underground
hideout and craft ammo and attachments from the junk you looted.

Built with **wgpu 30.0.1**, **winit 0.30.13**, **glam 0.33.11** and
**egui / egui-wgpu / egui-winit 0.36.2** (all pinned exactly in `Cargo.toml`).

## Running

```sh
cargo run --release
```

Requires a GPU with Vulkan, Metal or DX12 (falls back to OpenGL if none is found).
The profile is saved to `voxel_raid_save.json` in the working directory; override
with `--save <file>` or the `VOXEL_RAID_SAVE` environment variable.

Other useful flags:

| Flag | Effect |
|------|--------|
| `--seed N` | Use a fixed seed for the next raid map |
| `--save FILE` | Use a different save file |
| `--screen stash\|raid\|loot\|summary\|hideout\|station` | Jump straight to a screen (debug) |
| `--smoke-frames N` | Exit after N frames (used for automated smoke tests) |
| `--screenshot out.png` | With `--smoke-frames`, save the last frame as PNG |
| `--cam x,y,z,yaw,pitch` | Debug camera pose (y ≤ 0 means "stand on the ground") |
| `--mod-demo`, `--force-ads` | Debug helpers for inspecting attachments / sights |

`VOXEL_RAID_NO_VSYNC=1` disables vsync. **F12** saves a screenshot at any time,
**F3** toggles the debug overlay (FPS, position, chunk/triangle counts).

Tests: `cargo test` (52 unit/integration tests, including headless raid simulations).

## Controls

**In raid**

| Input | Action |
|-------|--------|
| WASD / Space | Move / jump (1-block ledges are stepped up automatically) |
| Shift | Sprint (uses stamina) |
| C / Left Ctrl | Crouch |
| Mouse | Look |
| Left mouse | Fire |
| Right mouse (hold) | Aim down sights |
| R | Reload (from rig, pockets, pouch, backpack) |
| T | Cycle the ammo type used for the next reload |
| B | Toggle semi / full auto |
| 1 / 2 / mouse wheel | Primary / holster weapon |
| F | Search the container or body you are looking at |
| Tab | Inventory |
| H | Use a med (surgical kit if a limb is destroyed, else a medkit) |
| O | Show extraction points (distance + direction) |
| Esc | Pause (settings, leave raid) |

**Inventory (stash, raid inventory, looting)**

| Input | Action |
|-------|--------|
| Drag & drop | Move items between grids and equipment slots |
| R while dragging | Rotate item |
| Drop on a matching stack | Merge stacks |
| Ctrl + click | Quick-move (loot → your bags, bags → loot/stash) |
| Double-click | Equip into the matching empty slot |
| Right-click | Context menu: Modify, Load/Unload ammo, Use, Split, Empty, Equip, Discard |

**Hideout**

| Input | Action |
|-------|--------|
| Left / right mouse | Remove / place block |
| 1–0, -, = / wheel | Select block from the hotbar |
| E or F | Use a station / open the stash box |
| Tab | Stash |
| Esc | Close window / pause menu (back to main menu) |

## How the game plays

* **Main menu**: Start Raid, Stash, Hideout, Quit. Shows profile stats (raids,
  survival rate, kills, extracted loot value) and your current loadout. If you end
  up with no weapon at all you can request a free emergency kit.
* **Stash**: a 10×30 Tarkov-style grid. Items occupy W×H cells (an AK is 5×2, a
  helmet 2×2, a Salewa 1×2...). Equip gear into slots: primary, holster, headwear,
  body armor, tactical rig, backpack, plus pockets and a 2×2 pouch. Right-click a
  weapon to **Modify** it or load it with a specific ammo type.
* **Raid**: a new 192×64×192 map every raid (terrain, two roads, houses,
  two-storey houses, an office, warehouses, shipping containers, checkpoints, car
  wrecks, sandbags, trees). You spawn at the edge with your equipped loadout; 2–3
  extraction points far from your spawn are open (green flare beacons). Stand in one
  for 6 seconds to extract. The raid lasts 25 minutes, after which you are Missing
  In Action. Dying or MIA loses **everything** you brought in; extracting keeps
  everything you carry out.
* **Scavs** (12 per raid) patrol with A* pathfinding over the voxel grid, spot you
  by line of sight (awareness builds faster when you are close, slower when you
  crouch; foliage hides you, glass doesn't), investigate gunshots and sprinting
  footsteps, turn on you when shot, strafe, crouch, burst-fire with accuracy that
  improves the longer the fight lasts, and reload. They carry random weapons, ammo,
  armor and helmets, all of which can be looted from their bodies.
* **Loot**: wooden crates (barter goods, ammo), weapon boxes (guns, attachments,
  gunpowder), medcases and filing cabinets (money, electronics, valuables).

### Weapons, ammo and destruction

Four receivers (MP-443 Grach pistol, PP-19 Vityaz SMG, AK-74N, AKM) with
per-weapon deterministic recoil patterns (learnable sprays that partly recover
after you stop firing; pulling down compensates), spread for hip-fire, ADS, movement
and sustained fire, fire modes, ADS time from ergonomics and weight, and reloads.

Nine ammo types across three calibers, each with distinct flesh damage, armor
penetration and block penetration power:

| Ammo | Damage | Armor pen | Block pen | Passes through |
|------|-------:|----------:|----------:|----------------|
| 9x19 RIP (HP) | 82 | 3 | 3 | glass, leaves |
| 9x19 PST (FMJ) | 50 | 20 | 12 | + wood planks |
| 9x19 AP 6.3 | 44 | 32 | 32 | + logs, sheet metal, dirt |
| 5.45 HP | 76 | 9 | 4 | glass, leaves |
| 5.45 PS (FMJ) | 53 | 23 | 18 | + wood planks |
| 5.45 BS (AP) | 43 | 52 | 55 | + brick |
| 7.62 HP | 84 | 14 | 6 | glass, leaves |
| 7.62 PS (FMJ) | 58 | 33 | 26 | + planks, logs |
| 7.62 BP (AP) | 58 | 47 | 66 | + brick, sandbags |

Every block has hit points and a penetration resistance (glass 1, planks 10,
sheet metal 30, brick 45, sandbags 60, concrete/stone 90, bedrock/containers
indestructible). Bullets damage every block they touch; blocks show progressive
cracks and are destroyed when their HP runs out, which remeshes the chunk (and its
neighbours / skylight below a broken roof). A bullet keeps going through blocks
whose resistance is below its remaining power, losing damage and armor penetration
on the way — so AP rounds can kill someone behind a brick wall while hollow points
can't get through a plank door. AI bullets obey the same rules.

Health is per body part (head 35, thorax 85, stomach 70, arms 60, legs 65).
Head or thorax at 0 kills; damage to a destroyed limb spreads to the rest of the
body; destroyed legs slow you and destroyed arms increase sway. Helmets and body
armor (class 2–5) have durability and roll for penetration against the ammo's
armor-pen value; blocked hits deal blunt damage and wear the armor down. Medkits
heal, surgical kits restore destroyed limbs, and fall damage hurts your legs.

### Weapon customization

Each receiver exposes a subset of six slots: barrel, muzzle, stock, sight,
magazine, foregrip. 33 attachments (barrels, brakes, compensators, suppressors,
stocks, red dot / holographic / 4× PSO scope, 17–95 round magazines, grips) with
compatibility rules modify recoil, ergonomics, accuracy, ADS time, reload time,
magazine size, zoom, loudness (how far scavs hear you) and weight. A gun without a
barrel is inoperable; without a stock it kicks much harder.

The **modding screen** (right-click a weapon in the stash → Modify) lists each slot
with the compatible parts you own; hovering a part previews its effect on the stats
panel (bars with the new value marked in green/red). Parts come from and go back to
your stash; swapping a magazine unloads its rounds into the stash. A rotating 3D
model of the weapon reflects every attachment, and the same model is used for the
first-person viewmodel and the guns scavs carry.

### Hideout

An underground bunker you can walk around and build in: place and remove blocks
from a 12-block palette, or dig into the stone around the hall (the outer bedrock
shell can't be broken). Your edits are saved.

Stations are upgraded with items from your stash (barter goods and roubles found in
raids) and each level unlocks recipes and adds decor around the station:

* **Workbench** (3 levels): foregrip, muzzle brakes, red dot → suppressors, compensator,
  45-round mag, Zhukov stock → PSO-1 scope, holo sight, 95-round drum, and a whole
  AK-74N.
* **Ammo Press** (3 levels): FMJ → hollow point → armor-piercing ammo for all calibers.
* **Medstation** (2 levels): AI-2 medkits → Salewa kits and Surv12 surgical kits.

A new profile can immediately afford the level-1 workbench upgrade.

## Architecture

```
src/
  main.rs        winit ApplicationHandler, egui integration, CLI flags, frame loop
  game.rs        screen state machine (menu/stash/raid/summary/hideout), raid lifecycle,
                 saving, per-screen 3D scene assembly
  raid.rs        a raid session: player, scavs, combat resolution, loot, healing,
                 extraction, outcome
  input.rs       keyboard/mouse state with per-frame edges
  effects.rs     particles, tracers, muzzle flashes
  rng.rs         deterministic SplitMix64 RNG + value noise / fbm
  world/         voxel data: Block table (hardness, penetration resistance, tiles),
                 16³ chunks, heightmap skylight, block damage, DDA raycasts, and
                 gen.rs (procedural raid map + spawn/patrol/extract/container info)
  render/        wgpu renderer: pipeline + WGSL shader (sun, skylight, AO, crack
                 overlay, fog, tile-clamped manual mip selection), procedural mipmapped
                 texture atlas, chunk mesher (face culling, AO, soft skylight,
                 parallel meshing), box-built models (humanoids, modular weapons,
                 viewmodel), PNG capture
  player/        first-person controller, AABB-vs-voxel physics with step-up (shared
                 with the AI), body-part health
  weapons/       receivers, attachments, ammo, armor, weapon stats, recoil patterns,
                 attachment swapping, hitscan ballistics with block penetration
  ai/            scav state machine (patrol / investigate / engage / dead) and A*
                 navigation over walkable voxel cells
  inventory/     items, W×H grids (placement, rotation, stacking), equipment,
                 starter kit, loot tables
  ui/            egui screens: style, HUD, main/pause/summary menus, drag & drop
                 inventory, modding screen, hideout HUD + station window
  save/          JSON profile (stash, equipment, hideout, stats, settings), atomic
                 writes, corrupt-save backup, raid-result application
  hideout/       stations, upgrade costs, recipes, crafting, bunker generation,
                 build mode and persistence of block edits
```

**Frame flow**: input events → `Game::update` (simulation for the active screen,
chunk remeshing of dirty chunks) → `Game::ui` (egui immediate mode; UI actions are
applied directly) → `Game::build_scene` (camera, per-frame entity/effect geometry,
viewmodel) → `Renderer::render` (world pass, viewmodel pass with its own depth,
egui pass).

**Rendering**: one pipeline and a 32-byte vertex (position, UV, packed normal,
colour, AO/skylight/crack) are used for chunks, entities, effects and the weapon.
The swapchain is a non-sRGB format with an sRGB view for the 3D passes, so egui and
the world both get correct gamma. Chunks are frustum culled; the whole 576-chunk map
meshes in ~12 ms on 16 threads, and a frame costs ~1.3 ms on an RTX 4070 laptop
GPU.

**Data model**: an `Item` is a kind plus optional state (stack count, a `Weapon`
with its attachments and loaded rounds, armor durability, med resource, or a nested
`Grid` for rigs/backpacks). Raids take ownership of the player's `Equipment` and give
it back (or a stripped copy) through `save::apply_raid_result`.

## Milestones

All five milestones are complete and each was committed separately:

1. **Engine and world**: camera, movement, gravity, AABB collision, 16³ chunked
   procedural raid map with buildings and cover, face-culled meshing, directional
   lighting, procedural texture atlas.
2. **Destruction and gunplay**: hitscan weapons with recoil patterns, spread, ADS,
   reloads and magazines; FMJ/HP/AP ammo with damage, armor and block penetration;
   block hardness and destruction with remeshing; scav AI with patrol, line-of-sight
   detection and return fire; body-part health for player and AI.
3. **Weapon customization**: receivers with six attachment slots, stat-modifying
   attachments, modding screen with a stats panel.
4. **Raid loop and stash**: main menu, grid inventories for stash, backpack and rig,
   loadouts, looting containers and bodies, extraction, loss on death, JSON saves.
5. **Hideout**: walkable, buildable voxel hideout; upgradeable workbench, ammo press
   and medstation that cost looted items and unlock ammo/attachment recipes.

## Known issues and limitations

* No audio (gunshots and footsteps exist only as AI "noise").
* Lamps are emissive but don't light their surroundings; lighting is sun + skylight +
  ambient occlusion, so building interiors and the hideout use flat ambient light.
* Scopes are a 2D overlay (no picture-in-picture), and the viewmodel is lit in view
  space.
* AI pathfinding has a node budget; scavs occasionally give up on unreachable goals
  or stand still until they pick a new patrol point. They only shoot at the player
  (no scav-vs-scav fights; their bullets pass through other scavs).
* The player is never blocked by scav bodies (no entity-vs-entity collision).
* Health resets between raids, and crafting is instant (no timers).
* Nested containers can only be browsed when equipped (use "Empty contents" on a
  backpack in the stash).
* There is no Tarkov-style secure container: the spec says dying loses *everything*
  brought in, so the 2×2 pouch is lost on death too.
* Quitting or closing the window during a raid counts as Missing In Action (the
  profile is saved before each raid, so a crash does not lose your gear).
* Some Wayland compositors throttle rendering to a few FPS while the window is
  hidden or unfocused; this is compositor behaviour, not game load.
* Balance (loot rates, scav accuracy, upgrade costs) has only been tuned lightly.
