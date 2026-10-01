# The Museum of Human Achievement

A walkable 3D museum built with **Bevy 0.19**. You start in the entrance hall
and walk into a domed rotunda. A shaft of sunlight falls through the oculus
onto a gilded armillary sphere. Seven themed galleries open off the rotunda,
with 36 exhibits running from the first stone tools to the World Wide Web.

The build ships no asset files. Every mesh, texture, normal map, the sky, and
all in-world text (placards, gilded lettering, wall panels, even the periodic
table's element tiles) is generated at startup.

## Run

```sh
cargo run --release
```

Click to enter. Walk up to any exhibit and look at it to read its story. Keep
looking for a moment and it's added to your discoveries.

| Action | Keys |
| --- | --- |
| Walk / strafe | `W A S D` or arrow keys |
| Hurry | `Shift` |
| Look | Mouse |
| Guided tour (on / off) | `T` |
| Next / previous tour stop | `N` / `→`, `P` / `←` |
| Map of the museum | `M` |
| Pause | `Esc` |
| Mouse sensitivity | `[` / `]` |

`--low` turns off SSAO, TAA and volumetric light for slower GPUs.
`--size 1920x1080` sets the window size.

## The galleries

| Gallery | Exhibits |
| --- | --- |
| **Rotunda** | The armillary sphere |
| **Dawn of Humanity** | Stone tools, control of fire, cave art, agriculture, the wheel |
| **Ancient Wonders** | Writing, the Great Pyramid, Euclid's *Elements*, the Parthenon, the Antikythera mechanism |
| **The Age of Discovery** | Movable type, heliocentrism, the telescope, circumnavigation, Newton's *Principia* |
| **Flight & Space** | Wright Flyer, Sputnik 1, Voyager and the Golden Record, JWST, Apollo 11 (a 1:10 Saturn V) |
| **Power & Industry** | Watt's steam engine, Stephenson's *Rocket*, the periodic table, electric light, radio |
| **Life & Medicine** | The microscope, vaccination, evolution, penicillin, the double helix |
| **The Information Age** | The Analytical Engine, ENIAC, the transistor, the microprocessor, the World Wide Web |

Many exhibits move. The fire flickers and lights the floor, and the steam
engine runs a slider-crank linkage with a spinning governor. The Antikythera
gears mesh, the orrery's planets orbit at Kepler-ish rates, and Newton's
cannonball stays in orbit. The ENIAC panel lights blink, radio waves ripple
out from the mast, and data packets travel around the web globe.

## How it's built

* **Layout** (`layout.rs`): an octagonal rotunda with galleries radiating from
  its sides. Collision is 2D: circles against wall segments and plinths.
* **Architecture** (`architecture.rs`): tiled walls, a coffered Pantheon-style
  dome with an oculus, engaged columns, a reflecting pool and benches. Each
  gallery has its own wall color, light box, spotlight tracks, mouldings and a
  gilded quotation.
* **Models** (`models.rs`, `meshes.rs`): every exhibit is assembled from
  primitives plus custom builders for surfaces of revolution, gears, prisms,
  convex polyhedra, lofted hulls and tubes swept along curves.
* **Textures** (`textures.rs`): tileable value-noise marble, plaster, wood,
  the coffer albedo and normal maps, the cave painting, a petri dish, globes,
  and HDR cubemaps for the sky and reflections. All get CPU-built mip chains.
* **Text in the world** (`placards.rs`): Bevy UI is laid out once into a
  4096×5632 atlas by an offscreen camera, which is then switched off. Placards,
  banners and panels sample their own cell of the atlas.
* **Lighting**: HDR with AgX tonemapping, bloom, SSAO and TAA. Each exhibit has
  its own spotlight, and only the six nearest cast shadows. The sunbeam is a
  distant spotlight whose shadow is clipped by the dome, lit with volumetric
  fog.
* **Map** (`map.rs`): a second orthographic camera just below the ceilings,
  with a marker only it can see.

### Debug flags

For screenshots and testing:

```
--shot out.png [--frames N]   render, save a screenshot, print the frame time and exit
--pose x,y,z,yaw,pitch        start pose (degrees)
--exhibit N                   stand at exhibit N's viewpoint
--placard N                   stand in front of exhibit N's placard
--tour [--exhibit N]          start the guided tour
--map  --title  --novsync  --timescale X
```
