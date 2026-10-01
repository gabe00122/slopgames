# Monster Race

Split-screen kart racing on the backs of colossal beasts, built with
**Bevy 0.19**. One to four players race on gamepads or the keyboard, and the
CPU fills the rest of an eight-kart grid.

The course is a living animal. The road is laid over its body and moves with
it: it tilts when the beast rolls, climbs when it raises a limb, and floods
when it wades deeper. Everything on its back is the scenery.

The build ships no asset files. Every mesh, the lettering, the HUD icons, the
sound effects and the music are generated at startup.

## Run

```sh
cargo run --release
```

`F11` toggles fullscreen and `M` mutes the music. `--low` turns off
anti-aliasing and bloom and simplifies shadows for slower GPUs.

To skip the menus: `cargo run --release -- --race 2 --course 1` starts a
two-player race on the second beast. Seats go to connected gamepads first,
then to the keyboard.

## Controls

Press `A` (or `Space` / `Enter`) on the title screen, then again to join.
Each gamepad is a player, and the keyboard can seat two more.

| Action | Gamepad | Keyboard 1 | Keyboard 2 |
| --- | --- | --- | --- |
| Steer | Left stick or D-pad | `A` `D` | `←` `→` |
| Accelerate | `RT` or `A` | `W` | `↑` |
| Brake / reverse | `LT` or `B` | `S` | `↓` |
| Hop and drift | `RB` or `X` | `Space` | `Right Shift` |
| Use item | `LB` or `Y` | `E` | `Enter` |
| Pause | `Start` | `Esc` | `P` |

* **Drift**: hold the drift button while steering into a bend. The kart hops,
  then slides. Steering tightens or widens the slide. Sparks turn blue, then
  orange, then pink; let go for a mini-turbo that grows with the color.
* **Rocket start**: come on the gas just before the countdown ends. Holding
  it for the whole countdown bogs the engine instead.
* **Ramps and boost pads**: striped ramps give a boost on landing, and
  glowing chevrons give one on contact.
* The crab's beach has no barrier on the seaward side. Drive off it and you
  drop into the sea; a bird carries you back, at the cost of a few seconds.

## The beasts

| Course | Beast | What the beast does to the road |
| --- | --- | --- |
| **Crabback Cay** | Old Brine, the island crab | The road circles a palm island on his shell, then runs out along one arm, leaps the gap between his claws and returns along the other. He raises his claws, which turns the arms into a climb and widens the leap. He also sinks lower into the sea and floods the beach on one side. |
| **Mossback Ridge** | Thundermoss, the great grazer | The road runs up one side of his spine, around the crown of his head, back down the other side and out around the club of his tail. He stoops to graze, which turns the neck into a descent, and rears his head, which makes it a steep climb. His tail swings the road from side to side. |
| **Cloudbreak Reef** | Galemother, the sky whale | A coral garden above a sea of cloud. The road loops out along each pectoral fin, crosses her blowhole and swings around her flukes. She beats her fins, which throws the fin loops up and down, and banks into the wind, which tips the whole course sideways. When she blows, the blowhole launches whoever is on it. |

Each beast announces what it is about to do, and does the same things at the
same points in every race.

On the course screen, **Grand Prix** races all three beasts in turn and keeps
score (10 points for a win down to 1 for eighth).

## Items

Item boxes give better items the further back you are.

| Item | Effect |
| --- | --- |
| Dash Berry / Berry Bunch | One burst of speed, or three |
| Spit Seed | Fires straight ahead and bounces off walls |
| Hornet | Chases the racer in front of you |
| Burr | Dropped behind you; spins out whoever hits it |
| Golden Pollen | A few seconds of invincible speed |
| Beast Horn | The beast shudders and everyone ahead of you is thrown |

The small creatures crossing the road (crabs on the cay, beetles on the
ridge) spin you out as well.

## How it's built

* **Beasts** (`beast/`): each is a skeleton, an animator that poses it every
  frame, and a set of meshes skinned to it on the GPU: the body, the road and
  every plant. Legs use two-bone IK against a ground that scrolls past, so
  the beast walks in place.
* **Track** (`track.rs`): a closed spline over the beast's rest pose, sampled
  every 2.5 m. Each sample carries skin weights, so the centerline is re-posed
  from the skeleton each frame.
* **Karts** (`kart.rs`): a kart's position is `(u, d)`: how far along the
  road and how far across it, plus a heading relative to the road. The beast
  can bend the road any way it likes and karts stay on it. Banked ground makes
  them slide, slopes slow them, and crests and ramps launch them. Heights are
  measured against the road, so a jump is the same whether or not the beast
  is moving under it.
* **Split screen** (`camera.rs`): each view renders to its own texture with
  HDR, bloom and tone mapping, and the window lays the textures out as panes.
  Each pane has its own HUD. With three players the fourth pane shows the
  whole beast. A marker in their color floats over each player's kart; render
  layers keep each player's own marker out of their own view.
* **Meshes** (`meshkit.rs`, `beast/flora.rs`): flat-shaded, vertex-colored
  geometry from lofts, lathes and icospheres.
* **Lettering and HUD** (`font.rs`, `hud.rs`): a stroke font and the item
  icons are drawn into textures from distance fields. The minimap is drawn
  from the track when a course loads.
* **Sound** (`audio.rs`): effects, the engine drone and the music loop are
  synthesized into sample buffers.

### Debug flags

For screenshots and unattended testing:

```
--race N [--course N] [--laps N] [--racers N]   start a race with N players
--autopilot                 the CPU drives the players' karts
--autopilot-drift           the same, using the drift button as a player would
--screen lobby|select       start on that screen
--shot out.png [--frames N] save a screenshot and exit
--time T[,T...]             take it at T seconds of race time (one file each)
--wait S[,S...]             take it S real seconds after launch
--pose x,y,z,yaw,pitch      fixed camera on the title screen (degrees)
--novsync  --mute  --dump-audio DIR
```

`MR_LOG=monster_race=debug` logs race events: falls, mini-turbos, items and
finish times. With `--autopilot`, the results screen moves on by itself, so a
whole Grand Prix can run unattended.

`tools/vpad.py` creates virtual Xbox 360 gamepads through `/dev/uinput` on
Linux and plays a script of button presses and stick moves. It is how the
controller support was tested without hardware.
