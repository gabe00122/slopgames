//! The race itself: the grid, the countdown, laps, standings and the finish.

use crate::{
    Args,
    audio::{Sfx, Sound},
    camera::{ViewMode, Views},
    course::{Course, LoadCourse, Mats},
    game::{COLORS, CPU_NAMES, MAX_RACERS, Results, Roster, Screen, Settings, Standing},
    input::Inputs,
    items::{self, ItemAssets},
    kart::{Driver, Kart, KartAssets, spawn_kart},
    util::Rng,
};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    /// Waiting for the beast to be built.
    Loading,
    /// A sweep around the beast before the start.
    Intro,
    Countdown,
    Racing,
    /// Every player is home; a short victory lap before the results.
    Finished,
}

#[derive(Resource)]
pub struct Race {
    pub phase: Phase,
    /// Seconds spent in the current phase.
    pub timer: f32,
    /// The race clock.
    pub time: f32,
    pub laps: u32,
    /// A message shown to every player, and how long it has left.
    pub banner: Option<(String, f32)>,
    pub rng: Rng,
    /// Drive the players' karts with the CPU (for unattended testing); see `Args`.
    pub autopilot: u8,
    pub paused: bool,
    pub pause_cursor: usize,
    pub restart: bool,
    count_beeps: i32,
}

impl Race {
    fn new(laps: u32, autopilot: u8, seed: u64) -> Self {
        Race {
            phase: Phase::Loading,
            timer: 0.0,
            time: 0.0,
            laps,
            banner: None,
            rng: Rng::new(seed),
            autopilot,
            paused: false,
            pause_cursor: 0,
            restart: false,
            count_beeps: 0,
        }
    }

    /// The number shown during the countdown: 3, 2, 1, then 0 for "GO".
    pub fn count(&self) -> i32 {
        (COUNTDOWN - self.timer).ceil() as i32
    }
}

pub const INTRO: f32 = 5.0;
pub const COUNTDOWN: f32 = 3.0;

/// Everything spawned for one race.
#[derive(Component)]
pub struct RaceEntity;

pub fn begin(
    mut commands: Commands,
    settings: Res<Settings>,
    args: Res<Args>,
    time: Res<Time<Real>>,
    mut load: MessageWriter<LoadCourse>,
) {
    load.write(LoadCourse(settings.course));
    let seed = (time.elapsed_secs() * 1000.0) as u64 + 17;
    commands.insert_resource(Race::new(settings.laps, args.autopilot, seed));
}

/// Builds the grid once the course is ready.
pub fn setup(
    mut commands: Commands,
    mut race: ResMut<Race>,
    mut course: ResMut<Course>,
    mut views: ResMut<Views>,
    settings: Res<Settings>,
    roster: Res<Roster>,
    kart_assets: Res<KartAssets>,
    item_assets: Res<ItemAssets>,
    mats: Res<Mats>,
    old: Query<Entity, With<RaceEntity>>,
) {
    if race.restart {
        for e in &old {
            commands.entity(e).despawn();
        }
        let (laps, autopilot) = (race.laps, race.autopilot);
        let seed = race.rng.u32() as u64;
        *race = Race::new(laps, autopilot, seed);
    }
    if race.phase != Phase::Loading || course.index != settings.course {
        return;
    }
    let humans = roster.players.len();
    let total = settings.racers.clamp(humans.max(1), MAX_RACERS);
    let cpus = total - humans;
    let n = course.track.n();

    // CPUs take the liveries the players left.
    let mut free: Vec<usize> = (0..COLORS.len())
        .filter(|&c| !roster.color_taken(c, usize::MAX))
        .collect();
    let mut modes = Vec::new();
    for i in 0..total {
        // CPUs fill the front of the grid and players start at the back, so
        // everyone has traffic to fight through.
        let (driver, color, name) = if i < cpus {
            // Liveries (and so names) are handed out in order, so a CPU keeps its
            // name from race to race in a Grand Prix.
            let color = free.remove(0);
            (Driver::Cpu, color, CPU_NAMES[color].to_string())
        } else {
            let slot = i - cpus;
            (
                Driver::Human(slot),
                roster.players[slot].color,
                format!("P{}", slot + 1),
            )
        };
        let row = (i / 2) as f32;
        let u = n - 3.0 - row * 3.4 - (i % 2) as f32 * 1.3;
        let d = if i % 2 == 0 { -3.0 } else { 3.0 };
        let mut kart = Kart::new(i, driver, color, name, u, d);
        // A spread of ability so the field strings out.
        kart.ai.skill = if cpus > 1 {
            0.9 + 0.1 * (i as f32 / (cpus - 1) as f32).min(1.0)
        } else {
            0.97
        };
        kart.ai.skill += race.rng.sym(0.012);
        kart.ground_y = course.track.at(u).at(d).y;
        let entity = spawn_kart(&mut commands, &kart_assets, &mats, kart, RaceEntity);
        if let Driver::Human(_) = driver {
            modes.push(ViewMode::Chase(entity));
        }
    }
    if modes.is_empty() {
        modes.push(ViewMode::Overview);
    }
    if modes.len() == 3 {
        modes.push(ViewMode::Overview);
    }
    views.modes = modes;
    items::spawn_boxes(&mut commands, &item_assets, &mats, &course);

    course.clock = 0.0;
    course.ctl.calm = true;
    race.phase = Phase::Intro;
    race.timer = 0.0;
}

/// Runs the phases, the race clock and the standings.
pub fn tick(
    time: Res<Time>,
    inputs: Res<Inputs>,
    mut race: ResMut<Race>,
    mut course: ResMut<Course>,
    mut results: ResMut<Results>,
    mut next: ResMut<NextState<Screen>>,
    mut sfx: MessageWriter<Sfx>,
    mut karts: Query<&mut Kart>,
) {
    let dt = time.delta_secs();
    race.timer += dt;
    if let Some((_, left)) = &mut race.banner {
        *left -= dt;
        if *left <= 0.0 {
            race.banner = None;
        }
    }
    if let Some(text) = course.out.banner {
        race.banner = Some((text.to_string(), 3.6));
        sfx.write(Sfx(Sound::Groan));
    }

    let mut all: Vec<Mut<Kart>> = karts.iter_mut().collect();
    let track = &course.track;

    // Standings: finishers by time, then everyone else by distance covered.
    let mut order: Vec<usize> = (0..all.len()).collect();
    order.sort_by(|&a, &b| match (all[a].finished, all[b].finished) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => all[b].progress(track).total_cmp(&all[a].progress(track)),
    });
    for (place, &i) in order.iter().enumerate() {
        all[i].place = place + 1;
    }
    let leader = order.first().map(|&i| all[i].pos);

    match race.phase {
        Phase::Loading => {}
        Phase::Intro => {
            let skip = race.timer > 0.6 && inputs.any_nav().confirm;
            if race.timer >= INTRO || skip || race.autopilot > 0 {
                race.phase = Phase::Countdown;
                race.timer = 0.0;
                race.count_beeps = 4;
            }
        }
        Phase::Countdown => {
            let count = race.count();
            if count < race.count_beeps && count > 0 {
                race.count_beeps = count;
                sfx.write(Sfx(Sound::Count));
            }
            if race.timer >= COUNTDOWN {
                race.phase = Phase::Racing;
                race.timer = 0.0;
                race.time = 0.0;
                course.clock = 0.0;
                course.ctl.calm = false;
                sfx.write(Sfx(Sound::Go));
                // Rocket start for anyone who came on the gas just before the flag.
                for k in all.iter_mut() {
                    if k.gas_held > 0.12 && k.gas_held < 0.9 {
                        k.give_boost(1.3);
                    } else if k.gas_held >= 1.8 {
                        // Revved too long: the engine bogs.
                        k.spin = 0.55;
                    }
                }
            }
        }
        Phase::Racing | Phase::Finished => {
            race.time += dt;
            let racing_time = race.time;
            let laps = race.laps as i32;
            for k in all.iter_mut() {
                if k.finished.is_none() && k.lap > laps {
                    k.finished = Some(racing_time);
                    debug!("{} finished in {:.2}", k.name, racing_time);
                    k.item = None;
                    k.roulette = 0.0;
                    if k.is_human() {
                        sfx.write(Sfx(Sound::Finish));
                    }
                } else if k.lap_flash == 2.2 && k.is_human() && k.lap > 1 {
                    sfx.write(Sfx(if k.lap == laps { Sound::FinalLap } else { Sound::Lap }));
                }
            }
            // Rubber band: CPUs far ahead of the best player ease off, stragglers hurry.
            let best_human = all
                .iter()
                .filter(|k| k.is_human())
                .map(|k| k.progress(track))
                .fold(f32::MIN, f32::max);
            for k in all.iter_mut() {
                if !k.is_human() && best_human > f32::MIN {
                    let gap_m = (k.progress(track) - best_human) * 2.5;
                    k.ai.band = (1.0 - gap_m / 3000.0).clamp(0.95, 1.045);
                }
            }
            let humans_left = all.iter().filter(|k| k.is_human() && k.finished.is_none()).count();
            let everyone_done = all.iter().all(|k| k.finished.is_some());
            if race.phase == Phase::Racing && (humans_left == 0 || everyone_done) {
                race.phase = Phase::Finished;
                race.timer = 0.0;
            }
            if race.phase == Phase::Finished {
                let skip = race.timer > 1.5 && inputs.any_nav().confirm;
                if race.timer > 6.0 || skip {
                    results.standings = order
                        .iter()
                        .map(|&i| Standing {
                            name: all[i].name.clone(),
                            color: all[i].color,
                            human: all[i].is_human(),
                            time: all[i].finished,
                        })
                        .collect();
                    next.set(Screen::Results);
                }
            }
        }
    }
    course.ctl.look_at = leader;
}

/// Pausing. Runs on real time so it still works while the clock is stopped.
pub fn pause(
    inputs: Res<Inputs>,
    roster: Res<Roster>,
    mut race: ResMut<Race>,
    mut time: ResMut<Time<Virtual>>,
    mut next: ResMut<NextState<Screen>>,
    mut sfx: MessageWriter<Sfx>,
) {
    if race.phase == Phase::Loading {
        return;
    }
    // Any player may pause; any player may work the pause menu.
    let mut nav = crate::input::Nav::default();
    for p in &roster.players {
        let n = inputs.nav(p.device);
        nav.dx |= n.dx;
        if nav.dy == 0 {
            nav.dy = n.dy;
        }
        nav.confirm |= n.confirm;
        nav.back |= n.back;
        nav.pause |= n.pause;
    }
    if roster.players.is_empty() {
        nav = inputs.any_nav();
    }
    if !race.paused {
        if nav.pause {
            debug!("paused");
            race.paused = true;
            race.pause_cursor = 0;
            time.pause();
            sfx.write(Sfx(Sound::Blip));
        }
        return;
    }
    if nav.dy != 0 {
        race.pause_cursor = (race.pause_cursor as i32 + nav.dy).rem_euclid(3) as usize;
        sfx.write(Sfx(Sound::Blip));
    }
    let choice = if nav.pause || nav.back {
        Some(0)
    } else if nav.confirm {
        Some(race.pause_cursor)
    } else {
        None
    };
    if let Some(choice) = choice {
        debug!("pause menu choice {choice}");
        race.paused = false;
        time.unpause();
        sfx.write(Sfx(Sound::Confirm));
        match choice {
            1 => race.restart = true,
            2 => next.set(Screen::Select),
            _ => {}
        }
    }
}

pub fn end(
    mut commands: Commands,
    mut views: ResMut<Views>,
    mut time: ResMut<Time<Virtual>>,
    course: Option<ResMut<Course>>,
    q: Query<Entity, With<RaceEntity>>,
) {
    for e in &q {
        commands.entity(e).despawn();
    }
    views.modes = vec![ViewMode::Showcase];
    time.unpause();
    if let Some(mut course) = course {
        course.ctl.calm = true;
        course.ctl.look_at = None;
    }
    commands.remove_resource::<Race>();
}
