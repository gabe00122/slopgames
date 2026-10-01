# sudo sacrifice

A Sacrifice-like built with **Bevy 0.19**. Two coding agents fight over an
island. Each one sacrifices old software on its altar to summon subagents,
reshapes the ground with spells, and tries to deprecate the other's altar.

You are ORCHESTRATOR (blue). The CPU is HALLUCINATOR (red). The title screen
runs a CPU-versus-CPU match in the background.

The build ships no asset files. Every mesh, the sound effects and the music
are generated at startup. The HUD uses Bevy's built-in monospace font.

## Run

```sh
cargo run --release
```

`F11` toggles fullscreen and `M` mutes the music. `--low` turns off MSAA and
bloom and uses fewer shadow cascades.

To skip the title: `cargo run --release -- --play --difficulty hard`.

## How it plays

* **Software** is what you sacrifice: floppy disks of old code (`left-pad@0.0.3`,
  `payroll.cbl`, `load-bearing.xlsx`...). Some lie around the island; walk
  over one to pick it up. Every subagent costs tokens plus one or more pieces
  of software, burned on your altar.
* **Tokens** regenerate. Provisioning a **datacenter** on a compute well
  (the glowing stone rings, `F`) raises your income. You also earn a bonus
  while you're near your own buildings.
* When a subagent dies it drops the software it was made from. Yours floats
  back to you after a few seconds. The enemy's must be carried to your altar
  by a **Garbage Collector**, which then sacrifices it for you.
* **Context** is your health. When it runs out you are compacted, and you
  come back at your altar eight seconds later.
* **To win, deprecate the enemy altar.** Stand near it with a Garbage
  Collector at its base and press `F` (100 tokens). The collector must
  survive for 12 seconds, and you must stay close. Garbage Collectors that
  are following you walk up to an enemy altar on their own when you get near.

### Spells

| Key | Spell | Cost | Effect |
| --- | --- | --- | --- |
| `1` | SEGFAULT | 18 | A lobbed core dump. Burns what it hits and blasts a small crater. Hold to keep firing. |
| `2` | HOTFIX | 40 | Heals you and every subagent within 14 m. |
| `3` | FIREWALL | 55 | Raises a 16 m wall of earth across the target. It burns for 7 s, and walkers can't climb it. |
| `4` | FORCE PUSH | 110 | A commit falls from orbit: heavy damage, knockback, and a crater that can fill with water. |
| `5` | FORK BOMB | 90 | Spikes erupt at random across the target area for 5 s. The ground ends up shattered. |

The ground keeps every change. Walls and spikes block walkers, craters can be
climbed out of, and deep water can be left but not entered. Trees and rocks
caught in a blast are destroyed. Only the plazas around altars and wells
can't be changed.

### Subagents

| Key | Subagent | Cost | Role |
| --- | --- | --- | --- |
| `6` | LINTER | 30 + 1 sw | A fast, cheap beetle that swarms in melee |
| `7` | TEST RUNNER | 55 + 1 sw | Fires homing assertions from range |
| `8` | FUZZER | 90 + 2 sw | A flying drone that drops bursting random bytes |
| `9` | MONOLITH | 150 + 3 sw | A slow stone golem of legacy code whose slams dent the ground |
| `0` | GARBAGE COLLECTOR | 40 + 1 sw | Collects software and is needed to deprecate an altar |

Subagents follow you in formation and defend you. Right-click sends every
fighter to the point under the crosshair, and they fight on the way there.
`R` calls them back. One agent can run 22 subagents at a time.

## Controls

| Action | Keyboard and mouse | Gamepad |
| --- | --- | --- |
| Move / look | `WASD` / mouse | Left stick / right stick |
| Jump | `Space` | `A` |
| Cast the selected slot | Left mouse | `RT` |
| Select a slot | `1`-`0`, wheel, `Q` `E` | `LB` `RB`, d-pad (up/down jumps between spells and summons) |
| Send subagents to the crosshair | Right mouse | `LT` |
| Regroup | `R` | `Y` |
| Provision datacenter / deprecate altar | `F` | `X` |
| Pause | `Esc` | `Start` |
| Mouse sensitivity / invert look | `[` `]` / `I` | |

On the title screen, `A`/`D` (or left/right) sets the rival's level:
JUNIOR, SENIOR or STAFF. Higher levels think faster, aim better, cast more
often, earn more tokens and attack sooner. JUNIOR won't march on your base
for the first four minutes, and SENIOR waits two.

## How it's built

* **Terrain** (`terrain.rs`): a 193 x 193 heightfield at 1.25 m spacing, cut
  into 144 chunk meshes. Spells queue animated edits (craters, walls, spikes)
  that run over a fraction of a second; each edit rebuilds only the chunks it
  touches. Height queries follow the drawn triangles exactly, and a ray march
  finds the point under the crosshair. The same data draws the minimap.
* **The island** (`world.rs`): noise hills, ridges and lakes, averaged with
  its own mirror image through the center so neither side has the better
  ground. The coast wanders and rises into cliffs in places, and the plazas
  under the altars and wells are flattened.
* **Agents and subagents** (`agent.rs`, `units.rs`): walkers refuse climbs
  steeper than a set grade and slide along whatever blocks them. They steer
  around walls, and deep water stops them. Every agent is driven through an
  `Intent`, which the keyboard, mouse and gamepad fill for you and `ai.rs`
  fills for the CPU.
* **The CPU agent** (`ai.rs`): re-plans a few times a second. It defends a
  threatened altar, retreats when low on context, fights nearby threats and
  claims wells, and it saves up for the next subagent or for a deprecation.
  Once its army is large enough it marches on enemy datacenters, then the
  altar.
* **Models** (`models.rs`, `meshkit.rs`, `glyphs.rs`): flat-shaded,
  vertex-colored meshes built from primitives, each with a separate glowing
  part for bloom. The floating `{ } ; </> $_` landmarks and the `>_` on each
  agent's monitor are a stroke font extruded into tubes.
* **Sound** (`audio.rs`): effects and the music loop are synthesized into
  sample buffers. Effects in the world get quieter with distance from the camera.

### Debug flags

For screenshots and unattended testing:

```
--play                        skip the title
--autopilot                   the CPU plays Blue too
--timescale X                 run the game faster
--verbose                     print the session log and a status line per agent
--exit-on-end                 quit when a session ends, printing the winner
--rich                        the player never runs out of tokens
--gallery                     one of each subagent in front of you, and a passive rival
--shot-on ritual|dead         take the --shot when a deprecation starts or Blue is compacted
--seed N  --difficulty easy|normal|hard
--shot out.png [--time T,T..|--wait S,S..] [--frames N] [--pose x,y,z,yaw,pitch] [--slot N]
--novsync  --mute  --dump-audio DIR  --size WxH
```

For example, `--play --autopilot --timescale 3 --verbose --exit-on-end`
plays out a whole session between two CPU agents in a minute or two.

`tools/vpad.py` creates virtual Xbox 360 gamepads through `/dev/uinput` on
Linux and plays a script of button presses and stick moves. It was used to
test the controls without hardware.
