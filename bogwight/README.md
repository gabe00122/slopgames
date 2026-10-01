# BOGWIGHT

A physics-driven 2D side-scroller built with **Bevy 0.19** and **Avian 2D**.
You are the thing in the swamp. The monster hunters who followed you in at
night, in a thunderstorm, are about to regret it.

Everything is generated at startup: the swamp, every mesh, every sound and
the music. There are no asset files. The physics runs on a flat 2D plane,
and the scene is drawn in 3D with a perspective camera. That way lanterns,
campfires, glowing fungus and lightning can light the swamp for real, and
the background layers drift past with true parallax.

## Run

```sh
cargo run --release
```

`F11` toggles fullscreen and `M` mutes. `--low` turns off MSAA, bloom and
lantern shadows, and halves the rain.

To skip the title: `cargo run --release -- --play [--night N] [--seed N]`.

## How it plays

Each night is a new stretch of swamp: mud banks, deep pools, reed marsh,
stilt docks, rope bridges and hunters' camps, with the warden's camp at the
far end. Take every hunter to end the night. Later nights are longer, with
more hunters and more crossbows.

**In the water you're at home.** Swimming is fast and fluid in every
direction, with momentum and drag. Under water you're nearly invisible,
especially deep down, and you heal. Rest at the surface and only your eyes
show. Burst (`Space`) for a sudden surge, or to leap clean out of the water.

**On land you're slow and clumsy.** You walk heavily and make short hops.
You also dry out: the *damp* meter drains, and when it's empty you start to
suffer. Get back in the water.

**Stealth.** Each hunter's sight depends on how lit you are and how exposed
you are. Darkness, deep water and reeds hide you. Lanterns, campfires and
open ground give you away. Hunters see what's in front of them far better
than what's behind. Footsteps on land, splashes and screams can be heard.
A hunter who notices something shows `?` and comes to look. One who's sure
shows `!`, blows a whistle, and brings everyone nearby.

**Lightning** lights up the whole swamp for everyone who's looking. When the
sky starts to flicker, get under the water.

### Killing

| | |
| --- | --- |
| **Ambush** | A claw strike on a hunter who hasn't seen you, who has lost track of you, who is floundering in the water, or whom you've just burst up out of the water beneath. One blow. |
| **Claw** | Against a hunter who's ready for you: three hits (the warden takes seven). |
| **Drown** | Grab a hunter and hold them under. |
| **Throw** | Grab a hunter, a corpse, a barrel, a crate or a lantern and hurl it. Hard landings hurt, and so does being hit by a flying friend. |
| **Capsize** | Grab a boat's side from the water and pull down, or ram it from below. Its crew goes in the water, where they're helpless. |
| **Cut the bridge** | Claw a rope bridge's plank and whoever's on it goes in. |

Douse lanterns by dunking them, or smash them with a claw, and the dark
spreads. Mushrooms bounce you high. Docks can be burst up through from
below, and pressing down drops you back through.

### The hunters

* **Lantern hunters** carry a light that shows you to everyone near it, and
  club you when they get close.
* **Crossbowmen** hold their distance. A red sight line warns you before
  each bolt. Bolts slow to nothing in water.
* **Boat crews** paddle their skiffs around the pools and chase you once
  they've seen you. The boats float on buoyancy forces and the crew stand in
  them on friction alone, so a boat rocks, lists, ships its crew and
  capsizes on its own physics.
* **The warden** at the end of each night has a big lantern, a fast
  crossbow, and a lot of health.

## Controls

| Action | Keyboard and mouse | Gamepad |
| --- | --- | --- |
| Move / swim | `WASD` or arrows | Left stick or d-pad |
| Burst (water) / jump (land) | `Space` (`Shift` also bursts) | `A` (`RT` / `RB` also burst) |
| Claw | `J` or left mouse | `X` |
| Grab / throw / let go | `K` or right mouse | `B` |
| Drop through a dock | Down on a dock | Down on a dock |
| Pause | `Esc` | `Start` |

A prompt under the bogwight shows what claw and grab would do right now.
`AMBUSH` means the next strike kills.

## Under the hood

* **Water** is a wave-equation simulation over 20 cm columns along the
  whole level. Splashes, wakes, swimming and rain all push the surface. The
  translucent water meshes are rebuilt from it every frame.
* **Buoyancy and drag** are applied as forces at sample points across each
  floating body, weighted by how far each point is under the surface. That's
  what lets boats right themselves, list under weight and turn over.
  Barrels, crates, logs, lily pads, lanterns, bridge planks and hunters all
  float the same way.
* **The bogwight** is a dynamic circle body. In the water it gets
  near-neutral buoyancy and steers toward the stick with momentum. On land it
  walks with ground-relative velocity, so it can ride a moving boat. The rig
  is animated procedurally: tail undulation, swept-back arms when swimming,
  a lurching walk, claw swipes, and squash and stretch on landing.
* **Rope bridges** are chains of planks on revolute joints. **Lanterns**
  hang from distance joints and swing when bumped. **Docks and mushroom
  caps** are one-way platforms, made with a collision hook.
* **Lighting**: lanterns and campfires are flickering point lights with
  shadows. Fungus, mushrooms and fireflies glow. Lightning briefly floods the
  scene through the moonlight and draws a bolt across the sky, with thunder
  after it. The fog turns murky green when the camera is under the water.

## Testing flags

```
--start X        start the bogwight at x = X
--god            the bogwight can't be hurt
--autopilot      a crude bot plays (lurks, ambushes, drowns, capsizes)
--grip-test      hang off the nearest boat and pull it over, printing its angle
--timescale X    run the game X times faster
--verbose        print the level layout and a status line every 5 s
--exit-on-end    quit when the night is decided and print the stats
--shot out.png   screenshot, then exit: --time T,T.. (play time), --wait S,S..
                 (real time), or --shot-on kill|spotted|flip|flash|dead|won|over
--dump-audio DIR write every synthesized sound as a WAV
```

`tools/vpad.py` creates virtual gamepads over `/dev/uinput`, to drive the
game unattended, e.g. `python3 tools/vpad.py wait:4 axis:0:lx:1 wait:3 tap:0:a`.
