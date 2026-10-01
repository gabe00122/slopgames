//! Sound. Like everything else, it is generated at startup: each effect, the
//! engine loop and the music are synthesized into sample buffers.

use crate::{
    Args,
    kart::{Kart, VMAX},
    race::{Phase, Race, RaceEntity},
};
use bevy::{
    audio::{AddAudioSource, ChannelCount, PlaybackMode, SampleRate, Source, Volume},
    platform::collections::HashMap,
    prelude::*,
    reflect::TypePath,
};
use std::{f32::consts::TAU, sync::Arc, time::Duration};

const RATE: u32 = 44_100;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sound {
    Blip,
    Confirm,
    Back,
    Join,
    Count,
    Go,
    Boost,
    MiniTurbo,
    Hop,
    Bump,
    Land,
    Hit,
    Splash,
    Fall,
    ItemBox,
    ItemGet,
    Use,
    Star,
    Horn,
    Groan,
    Lap,
    FinalLap,
    Finish,
}

/// Request to play a sound effect.
#[derive(Message, Clone, Copy)]
pub struct Sfx(pub Sound);

/// A mono sample buffer.
#[derive(Asset, TypePath, Clone)]
pub struct Wave {
    samples: Arc<[f32]>,
}

pub struct WaveDecoder {
    samples: Arc<[f32]>,
    pos: usize,
}

impl Iterator for WaveDecoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let s = self.samples.get(self.pos).copied();
        self.pos += 1;
        s
    }
}

impl Source for WaveDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(self.samples.len() as f32 / RATE as f32))
    }
}

impl Decodable for Wave {
    type Decoder = WaveDecoder;
    fn decoder(&self) -> WaveDecoder {
        WaveDecoder {
            samples: self.samples.clone(),
            pos: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Synthesis
// ---------------------------------------------------------------------------

struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 8_388_608.0 - 1.0
    }
}

/// Renders `secs` of sound from `f(t, phase helper)`.
fn render(secs: f32, mut f: impl FnMut(f32) -> f32) -> Vec<f32> {
    let n = (secs * RATE as f32) as usize;
    (0..n).map(|i| f(i as f32 / RATE as f32).clamp(-1.0, 1.0)).collect()
}

/// Attack/decay envelope: a quick rise then an exponential fall.
fn pluck(t: f32, attack: f32, decay: f32) -> f32 {
    if t < 0.0 {
        0.0
    } else {
        (t / attack).min(1.0) * (-t / decay).exp()
    }
}

/// Fades the last `tail` seconds to silence to avoid a click.
fn fade(mut v: Vec<f32>, tail: f32) -> Vec<f32> {
    let n = ((tail * RATE as f32) as usize).min(v.len());
    let len = v.len();
    for i in 0..n {
        v[len - 1 - i] *= i as f32 / n as f32;
    }
    v
}

fn square(phase: f32) -> f32 {
    // A softened square: the first few odd harmonics.
    let x = phase * TAU;
    x.sin() + (3.0 * x).sin() / 3.0 + (5.0 * x).sin() / 5.0
}

fn tri(phase: f32) -> f32 {
    1.0 - 4.0 * (phase.fract() - 0.5).abs()
}

fn note(semitones_from_a4: f32) -> f32 {
    440.0 * 2f32.powf(semitones_from_a4 / 12.0)
}

/// A run of plucked notes: `(start time, semitone, length)`.
fn arp(notes: &[(f32, f32, f32)], secs: f32, vol: f32) -> Vec<f32> {
    fade(
        render(secs, |t| {
            notes
                .iter()
                .map(|&(at, semi, len)| {
                    let lt = t - at;
                    if lt < 0.0 {
                        0.0
                    } else {
                        square(lt * note(semi)) * pluck(lt, 0.004, len)
                    }
                })
                .sum::<f32>()
                * vol
        }),
        0.02,
    )
}

/// A tone sweeping from `f0` to `f1`.
fn sweep(secs: f32, f0: f32, f1: f32, vol: f32, decay: f32) -> Vec<f32> {
    let mut phase = 0.0f32;
    fade(
        render(secs, |t| {
            let f = f0 + (f1 - f0) * (t / secs);
            phase += f / RATE as f32;
            square(phase) * vol * pluck(t, 0.005, decay)
        }),
        0.02,
    )
}

/// Noise through a one-pole low-pass whose cutoff moves from `c0` to `c1` (0..1).
fn whoosh(secs: f32, c0: f32, c1: f32, vol: f32, decay: f32, seed: u32) -> Vec<f32> {
    let mut noise = Noise(seed);
    let mut lp = 0.0f32;
    fade(
        render(secs, |t| {
            let c = c0 + (c1 - c0) * (t / secs);
            lp += (noise.next() - lp) * c;
            lp * vol * pluck(t, 0.01, decay) * 3.0
        }),
        0.03,
    )
}

fn mix(a: Vec<f32>, b: Vec<f32>) -> Vec<f32> {
    let n = a.len().max(b.len());
    (0..n)
        .map(|i| (a.get(i).copied().unwrap_or(0.0) + b.get(i).copied().unwrap_or(0.0)).clamp(-1.0, 1.0))
        .collect()
}

fn effect(sound: Sound) -> Vec<f32> {
    match sound {
        Sound::Blip => arp(&[(0.0, 12.0, 0.05)], 0.12, 0.25),
        Sound::Confirm => arp(&[(0.0, 7.0, 0.07), (0.07, 14.0, 0.12)], 0.35, 0.25),
        Sound::Back => arp(&[(0.0, 7.0, 0.07), (0.07, 0.0, 0.12)], 0.35, 0.25),
        Sound::Join => arp(
            &[
                (0.0, 3.0, 0.08),
                (0.07, 7.0, 0.08),
                (0.14, 10.0, 0.08),
                (0.21, 15.0, 0.2),
            ],
            0.6,
            0.22,
        ),
        Sound::Count => arp(&[(0.0, 0.0, 0.16)], 0.35, 0.3),
        Sound::Go => arp(&[(0.0, 12.0, 0.4), (0.0, 19.0, 0.4)], 0.9, 0.22),
        Sound::Boost => mix(
            sweep(0.7, 180.0, 700.0, 0.16, 0.3),
            whoosh(0.7, 0.05, 0.5, 0.3, 0.35, 7),
        ),
        Sound::MiniTurbo => sweep(0.3, 500.0, 1300.0, 0.16, 0.12),
        Sound::Hop => sweep(0.12, 300.0, 520.0, 0.12, 0.06),
        Sound::Bump => mix(
            sweep(0.22, 130.0, 50.0, 0.4, 0.07),
            whoosh(0.12, 0.4, 0.1, 0.25, 0.04, 3),
        ),
        Sound::Land => mix(
            sweep(0.2, 100.0, 45.0, 0.3, 0.07),
            whoosh(0.15, 0.2, 0.05, 0.2, 0.05, 5),
        ),
        Sound::Hit => mix(sweep(0.5, 700.0, 90.0, 0.2, 0.2), whoosh(0.4, 0.7, 0.1, 0.24, 0.14, 11)),
        Sound::Splash => whoosh(0.9, 0.5, 0.06, 0.3, 0.3, 13),
        Sound::Fall => sweep(0.9, 1000.0, 180.0, 0.14, 0.6),
        Sound::ItemBox => arp(
            &[
                (0.0, 12.0, 0.05),
                (0.05, 16.0, 0.05),
                (0.1, 19.0, 0.05),
                (0.15, 24.0, 0.1),
            ],
            0.4,
            0.18,
        ),
        Sound::ItemGet => arp(&[(0.0, 19.0, 0.09), (0.1, 24.0, 0.2)], 0.5, 0.2),
        Sound::Use => mix(
            sweep(0.16, 520.0, 260.0, 0.18, 0.08),
            whoosh(0.14, 0.5, 0.2, 0.2, 0.05, 17),
        ),
        Sound::Star => arp(
            &[
                (0.0, 12.0, 0.07),
                (0.06, 16.0, 0.07),
                (0.12, 19.0, 0.07),
                (0.18, 24.0, 0.07),
                (0.24, 28.0, 0.3),
            ],
            0.8,
            0.2,
        ),
        Sound::Horn => {
            let mut p = [0.0f32; 2];
            fade(
                render(1.5, |t| {
                    let bend = 1.0 - 0.12 * (-t * 6.0).exp() + 0.012 * (t * 30.0).sin();
                    p[0] += 98.0 * bend / RATE as f32;
                    p[1] += 147.0 * bend / RATE as f32;
                    let saw = |x: f32| 2.0 * x.fract() - 1.0;
                    (saw(p[0]) * 0.22 + saw(p[1]) * 0.14 + square(p[0] * 0.5) * 0.12)
                        * (t / 0.08).min(1.0)
                        * (1.0 - t / 1.5).powf(0.6)
                }),
                0.05,
            )
        }
        Sound::Groan => {
            let mut p = 0.0f32;
            let mut noise = Noise(23);
            let mut lp = 0.0f32;
            fade(
                render(2.4, |t| {
                    let f = 62.0 - 18.0 * (t / 2.4) + 6.0 * (t * 5.0).sin();
                    p += f / RATE as f32;
                    lp += (noise.next() - lp) * 0.02;
                    let env = (t / 0.4).min(1.0) * (1.0 - t / 2.4);
                    ((p * TAU).sin() * 0.5 + (p * TAU * 2.0).sin() * 0.2 + lp * 1.5) * env * 0.7
                }),
                0.1,
            )
        }
        Sound::Lap => arp(&[(0.0, 12.0, 0.1), (0.12, 19.0, 0.25)], 0.6, 0.22),
        Sound::FinalLap => arp(
            &[
                (0.0, 12.0, 0.08),
                (0.1, 12.0, 0.08),
                (0.2, 12.0, 0.08),
                (0.3, 19.0, 0.3),
            ],
            0.9,
            0.22,
        ),
        Sound::Finish => arp(
            &[
                (0.0, 3.0, 0.12),
                (0.14, 7.0, 0.12),
                (0.28, 10.0, 0.12),
                (0.42, 15.0, 0.25),
                (0.7, 10.0, 0.12),
                (0.84, 15.0, 0.6),
                (0.84, 19.0, 0.6),
            ],
            1.8,
            0.2,
        ),
    }
}

const ALL: [Sound; 23] = [
    Sound::Blip,
    Sound::Confirm,
    Sound::Back,
    Sound::Join,
    Sound::Count,
    Sound::Go,
    Sound::Boost,
    Sound::MiniTurbo,
    Sound::Hop,
    Sound::Bump,
    Sound::Land,
    Sound::Hit,
    Sound::Splash,
    Sound::Fall,
    Sound::ItemBox,
    Sound::ItemGet,
    Sound::Use,
    Sound::Star,
    Sound::Horn,
    Sound::Groan,
    Sound::Lap,
    Sound::FinalLap,
    Sound::Finish,
];

/// One second of engine drone that loops seamlessly (whole cycles only).
fn engine() -> Vec<f32> {
    render(1.0, |t| {
        let f = 56.0;
        let x = t * f;
        let saw = 2.0 * x.fract() - 1.0;
        let sub = (x * 0.5 * TAU).sin();
        let buzz = (x * 3.0 * TAU).sin() * 0.3;
        (saw * 0.5 + sub * 0.5 + buzz) * 0.35 * (1.0 + 0.25 * (t * 11.0 * TAU).sin())
    })
}

/// An eight-bar loop: plucked lead, triangle bass, kick, snare and hats.
fn music() -> Vec<f32> {
    const BEAT: f32 = 60.0 / 132.0;
    const R: f32 = -99.0;
    // Eighth notes, in semitones from A4.
    #[rustfmt::skip]
    let lead: [f32; 64] = [
        7.0, R, 10.0, 7.0, 5.0, R, 3.0, 5.0,
        7.0, R, 10.0, 12.0, 10.0, R, 7.0, R,
        12.0, R, 15.0, 12.0, 10.0, R, 7.0, 10.0,
        12.0, 10.0, 7.0, 5.0, 7.0, R, R, R,
        8.0, R, 12.0, 8.0, 7.0, R, 5.0, 7.0,
        8.0, R, 12.0, 15.0, 12.0, R, 8.0, R,
        10.0, R, 14.0, 10.0, 5.0, R, 10.0, 14.0,
        17.0, 14.0, 10.0, 5.0, 10.0, R, R, R,
    ];
    let roots: [f32; 8] = [-21.0, -21.0, -24.0, -24.0, -28.0, -28.0, -26.0, -26.0];
    let total = 32.0 * BEAT;
    let mut noise = Noise(41);
    let mut hat_lp = 0.0f32;
    render(total, |t| {
        let eighth = t / (BEAT * 0.5);
        let i = eighth as usize % 64;
        let lt = (eighth.fract()) * BEAT * 0.5;
        let mut s = 0.0;
        if lead[i] > R {
            let f = note(lead[i]);
            s += square(lt * f) * pluck(lt, 0.004, 0.11) * 0.16;
        }
        // Bass on the beat and the off-beat before 3.
        let beat = t / BEAT;
        let bar = (beat / 4.0) as usize % 8;
        let in_bar = beat % 4.0;
        for (at, len) in [(0.0, 0.9), (1.5, 0.4), (2.0, 0.9), (3.0, 0.45), (3.5, 0.45)] {
            let bt = (in_bar - at) * BEAT;
            if bt >= 0.0 && bt < len * BEAT {
                let semi = roots[bar] + if at == 3.5 { 7.0 } else { 0.0 };
                s += tri(bt * note(semi)) * pluck(bt, 0.005, 0.22) * 0.34;
            }
        }
        // Kick on 1 and 3, snare on 2 and 4, hats on every eighth.
        let bt = (beat % 1.0) * BEAT;
        let which = beat as usize % 4;
        let n = noise.next();
        if which.is_multiple_of(2) {
            s += ((bt * (120.0 - 260.0 * bt).max(40.0)) * TAU).sin() * pluck(bt, 0.002, 0.07) * 0.5;
        } else {
            s += n * pluck(bt, 0.002, 0.05) * 0.2;
        }
        hat_lp += (n - hat_lp) * 0.6;
        s += (n - hat_lp) * pluck(lt, 0.001, 0.018) * 0.08;
        s
    })
}

#[derive(Resource)]
pub struct Bank {
    waves: HashMap<Sound, Handle<Wave>>,
    engine: Handle<Wave>,
    music: Handle<Wave>,
}

#[derive(Component)]
pub struct EngineLoop(Entity);

#[derive(Component)]
pub struct Music;

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Wave>()
            .add_message::<Sfx>()
            .add_systems(Startup, setup)
            .add_systems(Update, (play, engines, music_toggle));
    }
}

fn setup(mut commands: Commands, mut waves: ResMut<Assets<Wave>>, args: Res<Args>) {
    let mut add = |v: Vec<f32>| waves.add(Wave { samples: v.into() });
    let bank = Bank {
        waves: ALL.iter().map(|&s| (s, add(effect(s)))).collect(),
        engine: add(engine()),
        music: add(music()),
    };
    if !args.mute {
        commands.spawn((
            AudioPlayer(bank.music.clone()),
            PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::Linear(0.32),
                ..default()
            },
            Music,
        ));
    }
    commands.insert_resource(bank);
}

fn play(
    mut commands: Commands,
    args: Res<Args>,
    bank: Res<Bank>,
    mut requests: MessageReader<Sfx>,
    mut played: Local<Vec<Sound>>,
) {
    played.clear();
    for Sfx(sound) in requests.read() {
        // The same effect from several karts in one frame plays once.
        if args.mute || played.contains(sound) {
            continue;
        }
        played.push(*sound);
        commands.spawn((
            AudioPlayer(bank.waves[sound].clone()),
            PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::Linear(0.8),
                ..default()
            },
        ));
    }
}

/// One engine drone per player, pitched by speed.
fn engines(
    mut commands: Commands,
    args: Res<Args>,
    bank: Res<Bank>,
    race: Option<Res<Race>>,
    new: Query<(Entity, &Kart), Added<Kart>>,
    karts: Query<&Kart>,
    mut loops: Query<(&EngineLoop, &mut AudioSink)>,
) {
    if args.mute {
        return;
    }
    for (entity, kart) in &new {
        if kart.is_human() {
            commands.spawn((
                AudioPlayer(bank.engine.clone()),
                PlaybackSettings {
                    mode: PlaybackMode::Loop,
                    volume: Volume::Linear(0.0),
                    ..default()
                },
                EngineLoop(entity),
                RaceEntity,
            ));
        }
    }
    let racing =
        race.is_some_and(|r| matches!(r.phase, Phase::Racing | Phase::Countdown | Phase::Finished) && !r.paused);
    let count = loops.iter().count().max(1) as f32;
    for (engine, mut sink) in &mut loops {
        let Ok(k) = karts.get(engine.0) else { continue };
        let speed = (k.speed.abs() / VMAX).clamp(0.0, 1.5);
        let rev = if k.input.gas > 0.1 { 0.25 } else { 0.0 };
        sink.set_speed(0.7 + speed * 1.5 + rev + if k.boost > 0.0 { 0.3 } else { 0.0 });
        let volume = if racing && k.finished.is_none() && k.respawn <= 0.0 {
            (0.16 + 0.1 * speed) / count.sqrt()
        } else {
            0.0
        };
        sink.set_volume(Volume::Linear(volume));
    }
}

fn music_toggle(keys: Res<ButtonInput<KeyCode>>, mut music: Query<&mut AudioSink, With<Music>>) {
    if keys.just_pressed(KeyCode::KeyM) {
        for mut sink in &mut music {
            sink.toggle_mute();
        }
    }
}

/// Writes every synthesized sound as a 16-bit mono WAV, for checking by ear or by script.
pub fn dump(dir: &str) {
    let write = |name: &str, samples: &[f32]| {
        let data: Vec<u8> = samples
            .iter()
            .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
            .collect();
        let mut wav = Vec::with_capacity(44 + data.len());
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&RATE.to_le_bytes());
        wav.extend_from_slice(&(RATE * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        let path = format!("{dir}/{name}.wav");
        match std::fs::write(&path, wav) {
            Ok(()) => println!("{path}: {:.2} s", samples.len() as f32 / RATE as f32),
            Err(e) => eprintln!("{path}: {e}"),
        }
    };
    let _ = std::fs::create_dir_all(dir);
    for sound in ALL {
        write(&format!("{sound:?}").to_lowercase(), &effect(sound));
    }
    write("engine", &engine());
    write("music", &music());
}
