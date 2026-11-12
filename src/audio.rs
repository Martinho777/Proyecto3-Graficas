//! Efectos de sonido, sintetizados por código (no hay archivos de audio).
//!
//! Cada efecto se genera al arrancar como una lista de muestras (ondas
//! seno / cuadrada / triangular / diente de sierra y ruido, con envolventes
//! y barridos de frecuencia), se empaqueta como un .wav en memoria y raylib
//! lo carga como `Sound`. El motor de la nave y el gruñido del enemigo son
//! bucles sin cortes que se cargan como `Music` (se repiten solos) y cambian
//! de volumen y tono según lo que pasa en el juego.
//!
//! Si no hay dispositivo de audio, `Audio::new` devuelve None y el juego
//! sigue en silencio.

use std::f32::consts::TAU;

use raylib::prelude::*;

/// Frecuencia de muestreo de todos los sonidos.
const RATE: u32 = 22050;

/// Efectos cortos.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    /// Mover la selección en el menú.
    MenuMove,
    /// Abrir/cerrar algo en el menú, confirmar.
    MenuSelect,
    /// Despegar (empezar a jugar).
    Start,
    ShipSwitch,
    OrbitOn,
    OrbitOff,
    /// Algo no se puede hacer (órbita lejos, confirmar salida…).
    Denied,
    /// Choque contra un planeta o el sol.
    Contact,
    /// Empieza el turbo.
    Boost,
    Gem,
    Hit,
    Win,
    Lose,
}

const ALL: [Sfx; 13] = [
    Sfx::MenuMove,
    Sfx::MenuSelect,
    Sfx::Start,
    Sfx::ShipSwitch,
    Sfx::OrbitOn,
    Sfx::OrbitOff,
    Sfx::Denied,
    Sfx::Contact,
    Sfx::Boost,
    Sfx::Gem,
    Sfx::Hit,
    Sfx::Win,
    Sfx::Lose,
];

/// Música de fondo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Track {
    /// Menú principal y modo explorador.
    Main,
    /// Modo supervivencia.
    Boss,
}

/// Archivos de música (opcionales: si no están, no suena música).
const MUSIC_FILES: [(Track, &str); 2] =
    [(Track::Main, "assets/music/main_theme.mp3"), (Track::Boss, "assets/music/final_boss.mp3")];
/// Volumen de la música (más bajo que los efectos para que se oigan).
const MUSIC_VOLUME: f32 = 0.45;

pub struct Audio {
    sounds: Vec<Sound<'static>>,
    engine: Music<'static>,
    enemy: Music<'static>,
    songs: Vec<(Track, Music<'static>)>,
    current: Option<Track>,
    pub muted: bool,
}

impl Audio {
    pub fn new() -> Option<Self> {
        // El dispositivo vive todo el programa: así los sonidos (que lo
        // toman prestado) pueden guardarse en esta estructura.
        let device: &'static RaylibAudio = Box::leak(Box::new(RaylibAudio::init_audio_device().ok()?));
        device.set_master_volume(0.8);
        let load = |samples: &[f32]| -> Option<Sound<'static>> {
            let wave = device.new_wave_from_memory(".wav", &to_wav(samples)).ok()?;
            device.new_sound_from_wave(&wave).ok()
        };
        let mut sounds = Vec::with_capacity(ALL.len());
        for s in ALL {
            sounds.push(load(&synth(s))?);
        }
        // raylib lee la música directamente de este buffer mientras suena:
        // tiene que vivir para siempre.
        let music = |samples: Vec<f32>| -> Option<Music<'static>> {
            let bytes: &'static Vec<u8> = Box::leak(Box::new(to_wav(&samples)));
            let m = device.new_music_from_memory(".wav", bytes).ok()?;
            m.set_volume(0.0);
            m.play_stream();
            Some(m)
        };
        let songs = MUSIC_FILES
            .iter()
            .filter_map(|&(t, path)| {
                let path = crate::scene::asset_path(path);
                let m = device.new_music(path.to_str()?).ok()?;
                m.set_volume(MUSIC_VOLUME);
                Some((t, m))
            })
            .collect();
        Some(Self {
            sounds,
            engine: music(engine_loop())?,
            enemy: music(enemy_loop())?,
            songs,
            current: None,
            muted: false,
        })
    }

    /// Cambia la música de fondo. Si ya suena esa canción, sigue sin cortarse
    /// (del menú al modo explorador no se reinicia).
    pub fn set_track(&mut self, track: Option<Track>) {
        if track == self.current {
            return;
        }
        for (t, m) in &self.songs {
            if Some(*t) == track {
                m.seek_stream(0.0);
                m.play_stream();
            } else {
                m.stop_stream();
            }
        }
        self.current = track;
    }

    pub fn play(&self, s: Sfx) {
        if !self.muted {
            self.sounds[ALL.iter().position(|&x| x == s).unwrap()].play();
        }
    }

    /// Llamar una vez por cuadro. `engine`: (potencia 0..1, turbo) o None
    /// para callar el motor; `enemy_dist`: distancia al enemigo o None.
    pub fn update(&self, engine: Option<(f32, bool)>, enemy_dist: Option<f32>) {
        self.engine.update_stream();
        self.enemy.update_stream();
        // La música se repite sola (raylib vuelve al inicio al terminar).
        for (_, m) in &self.songs {
            m.update_stream();
            m.set_volume(if self.muted { 0.0 } else { MUSIC_VOLUME });
        }
        let (vol, pitch) = match engine {
            Some((t, boost)) if !self.muted => {
                (0.12 + 0.45 * t + if boost { 0.15 } else { 0.0 }, 0.75 + 0.5 * t + if boost { 0.35 } else { 0.0 })
            }
            _ => (0.0, 1.0),
        };
        self.engine.set_volume(vol);
        self.engine.set_pitch(pitch);
        // El gruñido se oye desde 70 unidades y sube al acercarse.
        let ev = match enemy_dist {
            Some(d) if !self.muted => (1.0 - d / 70.0).clamp(0.0, 1.0).powf(1.5) * 0.9,
            _ => 0.0,
        };
        self.enemy.set_volume(ev);
        self.enemy.set_pitch(0.9 + 0.3 * ev);
    }
}

// ---------------------------------------------------------------------------
// Síntesis
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Square,
    Triangle,
    Saw,
}

fn osc(w: Wave, phase: f32) -> f32 {
    let p = phase.fract();
    match w {
        Wave::Sine => (p * TAU).sin(),
        Wave::Square => {
            if p < 0.5 { 1.0 } else { -1.0 }
        }
        Wave::Triangle => 1.0 - 4.0 * (p - 0.5).abs(),
        Wave::Saw => 2.0 * p - 1.0,
    }
}

/// Ruido blanco determinista (xorshift), en [-1, 1].
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn samples_for(secs: f32) -> usize {
    (secs * RATE as f32) as usize
}

/// Tono con frecuencia que cambia en el tiempo (`freq(t)` con t en [0, 1]) y
/// envolvente de ataque rápido + caída exponencial.
fn tone(secs: f32, w: Wave, freq: impl Fn(f32) -> f32, attack: f32, decay: f32, gain: f32) -> Vec<f32> {
    let n = samples_for(secs);
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let k = i as f32 / n as f32;
            phase += freq(k) / RATE as f32;
            let env = (t / attack).min(1.0) * (-t * decay).exp() * (1.0 - k).min(1.0).sqrt();
            osc(w, phase) * env * gain
        })
        .collect()
}

/// Suma `b` dentro de `a` empezando en el segundo `at` (alarga `a` si hace falta).
fn mix_at(a: &mut Vec<f32>, b: &[f32], at: f32) {
    let start = samples_for(at);
    if a.len() < start + b.len() {
        a.resize(start + b.len(), 0.0);
    }
    for (i, &s) in b.iter().enumerate() {
        a[start + i] += s;
    }
}

/// Ruido filtrado (pasa-bajos de un polo) con envolvente; `cut(k)` en (0, 1]
/// controla qué tan brillante es en cada momento.
fn noise_burst(secs: f32, seed: u32, cut: impl Fn(f32) -> f32, decay: f32, gain: f32) -> Vec<f32> {
    let n = samples_for(secs);
    let mut rng = Noise(seed);
    let mut y = 0.0f32;
    (0..n)
        .map(|i| {
            let k = i as f32 / n as f32;
            let t = i as f32 / RATE as f32;
            y += cut(k).clamp(0.001, 1.0) * (rng.next() - y);
            let env = (t / 0.005).min(1.0) * (-t * decay).exp() * (1.0 - k);
            y * env * gain
        })
        .collect()
}

/// Nota MIDI → Hz.
fn midi(n: f32) -> f32 {
    440.0 * 2f32.powf((n - 69.0) / 12.0)
}

/// Arpegio: notas MIDI separadas `step` segundos, cada una dura `len`.
fn arpeggio(notes: &[f32], step: f32, len: f32, w: Wave, gain: f32) -> Vec<f32> {
    let mut out = Vec::new();
    for (i, &m) in notes.iter().enumerate() {
        let f = midi(m);
        mix_at(&mut out, &tone(len, w, |_| f, 0.004, 6.0, gain), i as f32 * step);
    }
    out
}

fn synth(s: Sfx) -> Vec<f32> {
    let mut out = match s {
        Sfx::MenuMove => tone(0.07, Wave::Square, |k| 900.0 + 300.0 * k, 0.002, 30.0, 0.25),
        Sfx::MenuSelect => {
            let mut a = tone(0.08, Wave::Square, |_| 660.0, 0.002, 20.0, 0.22);
            mix_at(&mut a, &tone(0.12, Wave::Square, |_| 990.0, 0.002, 18.0, 0.22), 0.07);
            a
        }
        Sfx::Start => {
            // Barrido hacia arriba + soplido de ruido: "¡despegue!".
            let mut a = tone(0.7, Wave::Saw, |k| 120.0 + 900.0 * k * k, 0.02, 2.0, 0.22);
            mix_at(&mut a, &noise_burst(0.8, 7, |k| 0.05 + 0.5 * k, 2.5, 0.6), 0.0);
            a
        }
        Sfx::ShipSwitch => tone(0.14, Wave::Triangle, |k| 520.0 + 360.0 * k, 0.003, 12.0, 0.45),
        Sfx::OrbitOn => arpeggio(&[72.0, 76.0, 79.0, 84.0], 0.07, 0.25, Wave::Sine, 0.4),
        Sfx::OrbitOff => arpeggio(&[84.0, 79.0, 76.0, 72.0], 0.07, 0.25, Wave::Sine, 0.4),
        Sfx::Denied => {
            let mut a = tone(0.09, Wave::Square, |_| 220.0, 0.002, 10.0, 0.22);
            mix_at(&mut a, &tone(0.12, Wave::Square, |_| 185.0, 0.002, 10.0, 0.22), 0.11);
            a
        }
        Sfx::Contact => {
            // Golpe sordo: tono grave que cae + ruido opaco.
            let mut a = tone(0.3, Wave::Sine, |k| 110.0 - 60.0 * k, 0.002, 12.0, 0.9);
            mix_at(&mut a, &noise_burst(0.25, 3, |_| 0.08, 14.0, 0.9), 0.0);
            a
        }
        Sfx::Boost => noise_burst(0.55, 11, |k| 0.03 + 0.35 * k, 3.0, 0.9),
        Sfx::Gem => {
            // Arpegio brillante y un destello agudo que tiembla.
            let mut a = arpeggio(&[84.0, 88.0, 91.0, 96.0], 0.055, 0.35, Wave::Triangle, 0.35);
            mix_at(&mut a, &tone(0.6, Wave::Sine, |k| 2093.0 * (1.0 + 0.01 * (k * 60.0).sin()), 0.01, 5.0, 0.18), 0.2);
            a
        }
        Sfx::Hit => {
            // Golpe áspero: ruido fuerte + onda cuadrada que se hunde.
            let mut a = noise_burst(0.35, 5, |k| 0.6 - 0.5 * k, 7.0, 0.9);
            mix_at(&mut a, &tone(0.45, Wave::Square, |k| 160.0 - 110.0 * k, 0.002, 5.0, 0.35), 0.0);
            a
        }
        Sfx::Win => {
            let mut a = arpeggio(&[72.0, 76.0, 79.0], 0.13, 0.3, Wave::Square, 0.18);
            mix_at(&mut a, &tone(0.9, Wave::Square, |_| midi(84.0), 0.005, 2.0, 0.2), 0.42);
            mix_at(&mut a, &tone(0.9, Wave::Sine, |_| midi(76.0), 0.005, 2.0, 0.25), 0.42);
            a
        }
        Sfx::Lose => {
            let mut a = Vec::new();
            for (i, m) in [67.0f32, 63.0, 60.0].iter().enumerate() {
                let f = midi(*m);
                mix_at(&mut a, &tone(0.35, Wave::Triangle, |_| f, 0.005, 4.0, 0.4), i as f32 * 0.3);
            }
            mix_at(&mut a, &tone(0.9, Wave::Triangle, |k| midi(55.0) * (1.0 - 0.25 * k), 0.005, 2.0, 0.4), 0.9);
            a
        }
    };
    normalize(&mut out, 0.9);
    out
}

/// Motor: zumbido grave (dos dientes de sierra casi iguales que "laten") +
/// ruido suave. 1 s exacto con número entero de ciclos: se repite sin cortes.
fn engine_loop() -> Vec<f32> {
    let n = RATE as usize;
    // Se genera un poco más de ruido y el sobrante se funde con el
    // principio: así el final empalma justo con el inicio del bucle.
    let fade = n / 10;
    let mut rng = Noise(99);
    let mut y = 0.0f32;
    let noise: Vec<f32> = (0..n + fade)
        .map(|_| {
            y += 0.04 * (rng.next() - y);
            y
        })
        .collect();
    let mut out: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let nz = if i < fade {
                let w = i as f32 / fade as f32;
                noise[i] * w + noise[n + i] * (1.0 - w)
            } else {
                noise[i]
            };
            // Fases desfasadas: si todas las ondas "saltaran" en la misma
            // muestra (el empalme del bucle), se oiría un clic.
            0.35 * osc(Wave::Saw, 55.0 * t + 0.31) + 0.3 * osc(Wave::Saw, 56.0 * t + 0.67)
                + 0.075 * osc(Wave::Square, 110.0 * t + 0.13)
                + 2.5 * nz
        })
        .collect();
    normalize(&mut out, 0.7);
    out
}

/// Gruñido del enemigo: tonos muy graves que laten y un trémolo lento.
/// 2 s con ciclos enteros (40 Hz y 43 Hz) para que el bucle no se note.
fn enemy_loop() -> Vec<f32> {
    let n = 2 * RATE as usize;
    let mut out: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let trem = 0.7 + 0.3 * (TAU * 2.0 * t).sin();
            (0.5 * osc(Wave::Saw, 40.0 * t + 0.37) + 0.4 * osc(Wave::Square, 43.0 * t + 0.11) + 0.2 * osc(Wave::Sine, 80.0 * t)) * trem
        })
        .collect();
    normalize(&mut out, 0.8);
    out
}

fn normalize(s: &mut [f32], peak: f32) {
    let max = s.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    if max > 0.0 {
        for x in s.iter_mut() {
            *x *= peak / max;
        }
    }
}

/// Empaqueta muestras mono como WAV PCM de 16 bits.
fn to_wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes()); // tamaño del bloque fmt
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes()); // bytes por segundo
    b.extend_from_slice(&2u16.to_le_bytes()); // bytes por muestra
    b.extend_from_slice(&16u16.to_le_bytes()); // bits
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_is_short_and_not_clipped() {
        for s in ALL {
            let v = synth(s);
            assert!(!v.is_empty() && v.len() < samples_for(2.0), "{s:?}: {} muestras", v.len());
            assert!(v.iter().all(|x| x.abs() <= 0.9 + 1e-4), "{s:?} satura");
            assert!(v.iter().any(|x| x.abs() > 0.5), "{s:?} casi no suena");
        }
    }

    #[test]
    fn loops_are_seamless() {
        for (name, v) in [("motor", engine_loop()), ("enemigo", enemy_loop())] {
            // El salto entre la última muestra y la primera debe parecerse al
            // salto normal entre muestras vecinas.
            let jump = (v[v.len() - 1] - v[0]).abs();
            let typical = v.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
            assert!(jump <= typical * 1.05, "{name}: salto {jump} vs {typical}");
        }
    }

    #[test]
    fn wav_header_is_valid() {
        let w = to_wav(&[0.0, 0.5, -0.5]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()), 6);
        assert_eq!(w.len(), 44 + 6);
    }
}
