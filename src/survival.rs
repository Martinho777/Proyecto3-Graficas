//! Modo SUPERVIVENCIA: una nave enemiga (el Devorador) nos persigue y hay
//! que recoger la gema que orbita cada planeta antes de perder los 3
//! corazones.
//!
//! - `Survival`: estado de la partida (corazones, gemas, enemigo). No dibuja.
//! - `SurvivalArt`: mallas y shaders para dibujar el enemigo, las gemas y el
//!   efecto de recogerlas. Se dibuja después de la escena con el mismo
//!   renderer (comparte el z-buffer, así que planetas y nave tapan bien).

use crate::collision::Collider;
use crate::config::*;
use crate::math::{Mat4, Vec3};
use crate::mesh::{self, Mesh};
use crate::noise::Perlin;
use crate::planet::Planet;
use crate::renderer::{DrawOptions, Renderer};
use crate::scene::asset_path;
use crate::shader::Uniforms;
use crate::shaders::{GemDraw, GlowDraw, ThrusterDraw, ThrusterParams, ThrusterShader, ToonShader};
use crate::ship::Ship;

/// Gema que orbita un planeta.
pub struct Gem {
    /// Índice del planeta en `scene.planets`.
    pub planet: usize,
    pub color: Vec3,
    /// Plano de la órbita de la gema alrededor de su planeta.
    axis: Vec3,
    phase: f32,
    /// Instante (tiempo de juego) en que se recogió; None = sigue ahí.
    pub collected_at: Option<f32>,
}

impl Gem {
    /// Posición en mundo en el instante `t`: da vueltas despacio alrededor
    /// del planeta, un poco por fuera de sus montañas y objetos.
    pub fn position(&self, planets: &[Planet], t: f32) -> Vec3 {
        let p = &planets[self.planet];
        let dist = p.bounding_radius + GEM_ALTITUDE;
        // Vector perpendicular al eje, girado según el tiempo.
        let base = self.axis.cross(if self.axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X }).normalize();
        let dir = base.rotate_around(self.axis, self.phase + t * GEM_ORBIT_SPEED);
        p.position(t) + dir * dist
    }
}

/// La nave enemiga.
pub struct Enemy {
    pub pos: Vec3,
    pub vel: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    /// Segundos en que el motor está apagado (al principio y tras un golpe).
    pub stun: f32,
}

impl Enemy {
    pub fn right(&self) -> Vec3 {
        self.forward.cross(self.up).normalize()
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Outcome {
    Won,
    Lost,
}

/// Lo que pasó en un `update` (para avisos del HUD).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Event {
    GemCollected { planet: &'static str },
    Hit,
}

pub struct Survival {
    pub hearts: u32,
    pub gems: Vec<Gem>,
    pub enemy: Enemy,
    /// Segundos de invulnerabilidad que quedan tras un golpe.
    pub invulnerable: f32,
    /// Segundos desde que empezó la partida.
    pub elapsed: f32,
    pub outcome: Option<Outcome>,
    accumulator: f32,
}

/// Colores de las gemas (uno por planeta, se repiten si hay más).
const GEM_COLORS: [Vec3; 8] = [
    Vec3::new(0.25, 1.0, 0.45),
    Vec3::new(0.75, 0.40, 1.0),
    Vec3::new(1.0, 0.45, 0.15),
    Vec3::new(1.0, 0.85, 0.25),
    Vec3::new(0.35, 0.85, 1.0),
    Vec3::new(1.0, 0.35, 0.70),
    Vec3::new(0.95, 0.95, 1.0),
    Vec3::new(0.30, 0.55, 1.0),
];

impl Survival {
    /// Nueva partida. El enemigo aparece lejos, detrás de la nave.
    pub fn new(planets: &[Planet], ship: &Ship) -> Self {
        let gems = planets
            .iter()
            .enumerate()
            .map(|(i, _)| {
                // Plano y fase distintos por planeta (deterministas).
                let a = i as f32 * 2.399; // ángulo dorado
                Gem {
                    planet: i,
                    color: GEM_COLORS[i % GEM_COLORS.len()],
                    axis: Vec3::new(0.35 * a.cos(), 1.0, 0.35 * a.sin()).normalize(),
                    phase: a * 1.7,
                    collected_at: None,
                }
            })
            .collect();
        let s = &ship.state;
        let pos = s.pos - s.forward * ENEMY_SPAWN_DISTANCE + Vec3::Y * 15.0;
        Self {
            hearts: SURVIVAL_HEARTS,
            gems,
            enemy: Enemy { pos, vel: Vec3::ZERO, forward: (s.pos - pos).normalize(), up: Vec3::Y, stun: ENEMY_START_DELAY },
            invulnerable: 0.0,
            elapsed: 0.0,
            outcome: None,
            accumulator: 0.0,
        }
    }

    pub fn collected(&self) -> usize {
        self.gems.iter().filter(|g| g.collected_at.is_some()).count()
    }

    pub fn total(&self) -> usize {
        self.gems.len()
    }

    /// Velocidad máxima actual del enemigo: se pone más bravo con cada gema.
    pub fn enemy_max_speed(&self) -> f32 {
        ENEMY_SPEED + ENEMY_SPEED_PER_GEM * self.collected() as f32
    }

    /// Avanza la partida `dt` segundos (pasos fijos, como la nave). `time`
    /// es el reloj de la escena al final del cuadro (para las órbitas).
    pub fn update(&mut self, dt: f32, time: f32, ship: &mut Ship, planets: &[Planet], colliders: &[Collider]) -> Vec<Event> {
        let mut events = Vec::new();
        if self.outcome.is_some() {
            return events;
        }
        self.elapsed += dt;
        self.invulnerable = (self.invulnerable - dt).max(0.0);

        // --- Gemas -----------------------------------------------------------
        for g in &mut self.gems {
            if g.collected_at.is_none() && (g.position(planets, time) - ship.state.pos).length() < GEM_PICK_RADIUS {
                g.collected_at = Some(time);
                events.push(Event::GemCollected { planet: planets[g.planet].def.name });
            }
        }
        if self.collected() == self.total() {
            self.outcome = Some(Outcome::Won);
            return events;
        }

        // --- Enemigo ---------------------------------------------------------
        self.accumulator += dt;
        while self.accumulator >= PHYSICS_DT {
            self.accumulator -= PHYSICS_DT;
            self.enemy_step(PHYSICS_DT, ship, colliders);
        }

        // --- Golpe -----------------------------------------------------------
        events.extend(self.check_hit(ship));
        events
    }

    /// Si el enemigo toca la nave (y no está en su tiempo de gracia), quita
    /// un corazón, empuja la nave y aturde al enemigo.
    fn check_hit(&mut self, ship: &mut Ship) -> Option<Event> {
        let to_ship = ship.state.pos - self.enemy.pos;
        if self.invulnerable > 0.0 || to_ship.length() >= ENEMY_HIT_RADIUS + SHIP_COLLISION_RADIUS {
            return None;
        }
        self.hearts = self.hearts.saturating_sub(1);
        self.invulnerable = HIT_INVULNERABLE;
        let n = to_ship.normalize();
        // La nave sale despedida y el enemigo rebota y se queda aturdido.
        ship.knockback(n * HIT_KNOCKBACK);
        self.enemy.vel = -n * (HIT_KNOCKBACK * 0.5);
        self.enemy.stun = ENEMY_STUN;
        if self.hearts == 0 {
            self.outcome = Some(Outcome::Lost);
        }
        Some(Event::Hit)
    }

    /// Un paso de la IA: persigue el punto donde estará la nave, gira con
    /// un límite (se le puede esquivar) y esquiva planetas y el sol.
    fn enemy_step(&mut self, dt: f32, ship: &Ship, colliders: &[Collider]) {
        let max_speed = self.enemy_max_speed();
        let e = &mut self.enemy;
        e.stun = (e.stun - dt).max(0.0);

        // Persecución con anticipación: apunta adonde va a estar la nave.
        let to = ship.state.pos - e.pos;
        let lead = (to.length() / max_speed.max(1.0)).min(1.5);
        let target = ship.state.pos + ship.state.vel * lead;
        let mut desired = (target - e.pos).normalize();

        // Esquivar cuerpos que tiene enfrente.
        for c in colliders {
            let d = e.pos - c.center;
            let dist = d.length();
            let clear = c.radius_toward(e.pos) + ENEMY_RADIUS + 3.0;
            let look = 14.0;
            if dist < clear + look && e.forward.dot(-d) > 0.0 {
                desired += d.normalize() * (2.5 * (clear + look - dist) / look);
            }
        }
        let desired = desired.normalize();

        // Girar hacia `desired` como mucho ENEMY_TURN_RATE rad/s.
        let ang = e.forward.dot(desired).clamp(-1.0, 1.0).acos();
        if ang > 1e-4 {
            let mut axis = e.forward.cross(desired);
            if axis.length() < 1e-5 {
                axis = e.up;
            }
            let step = ang.min(ENEMY_TURN_RATE * dt);
            e.forward = e.forward.rotate_around(axis.normalize(), step).normalize();
        }
        // El techo se mantiene lo más cerca posible del "arriba" del mundo.
        let right = e.forward.cross(e.up.lerp(Vec3::Y, 0.02)).normalize();
        e.up = right.cross(e.forward);

        // Acelera hacia su velocidad máxima en la dirección de la nariz
        // (aturdido: solo se va frenando).
        let want = if e.stun > 0.0 { Vec3::ZERO } else { e.forward * max_speed };
        e.vel = e.vel.lerp(want, 1.0 - (-ENEMY_RESPONSE * dt).exp());
        e.pos += e.vel * dt;

        for c in colliders {
            c.resolve(&mut e.pos, &mut e.vel, ENEMY_RADIUS, 0.2, 0.05);
        }
    }
}

// ---------------------------------------------------------------------------
// Dibujo
// ---------------------------------------------------------------------------

/// Gema facetada (tipo brillante): mesa arriba, corona, cintura y pabellón
/// que termina en punta. Cada triángulo tiene sus propios vértices con la
/// normal de su cara, así se ven las facetas.
fn gem_mesh(sides: u32) -> Mesh {
    let ring = |y: f32, r: f32, off: f32| -> Vec<Vec3> {
        (0..sides)
            .map(|i| {
                let a = std::f32::consts::TAU * (i as f32 + off) / sides as f32;
                Vec3::new(r * a.cos(), y, r * a.sin())
            })
            .collect()
    };
    let table = ring(0.45, 0.38, 0.0);
    let girdle = ring(0.12, 0.70, 0.5);
    let top = Vec3::new(0.0, 0.45, 0.0);
    let culet = Vec3::new(0.0, -0.95, 0.0);
    let mut tris: Vec<[Vec3; 3]> = Vec::new();
    for i in 0..sides as usize {
        let j = (i + 1) % sides as usize;
        // Mesa (vista desde arriba, antihorario).
        tris.push([top, table[j], table[i]]);
        // Corona: entre mesa y cintura.
        tris.push([table[i], table[j], girdle[i]]);
        tris.push([table[j], girdle[j], girdle[i]]);
        // Pabellón hacia la punta.
        tris.push([girdle[i], girdle[j], culet]);
    }
    let mut m = Mesh::default();
    for t in tris {
        let mut n = (t[1] - t[0]).cross(t[2] - t[0]).normalize();
        // Orientar hacia fuera (el centro de la gema está cerca del origen).
        let c = (t[0] + t[1] + t[2]) * (1.0 / 3.0);
        let (a, b) = if n.dot(c - Vec3::new(0.0, -0.1, 0.0)) < 0.0 {
            n = -n;
            (t[2], t[1])
        } else {
            (t[1], t[2])
        };
        let base = m.positions.len() as u32;
        for p in [t[0], a, b] {
            m.positions.push(p);
            m.normals.push(n);
            m.uvs.push(Default::default());
            m.colors.push(Vec3::ONE);
            m.emissive.push(0.0);
        }
        m.indices.push([base, base + 1, base + 2]);
    }
    m.compute_bounding_radius();
    m
}

/// Dirección pseudoaleatoria (determinista) para el fragmento `i`.
fn shard_dir(i: u32) -> Vec3 {
    let a = i as f32 * 2.399_963;
    let y = 1.0 - 2.0 * (i as f32 + 0.5) / SHARDS as f32;
    let r = (1.0 - y * y).sqrt();
    Vec3::new(r * a.cos(), y, r * a.sin())
}

const SHARDS: u32 = 16;
/// Duración del efecto al recoger una gema (s).
pub const BURST_TIME: f32 = 1.1;

pub struct SurvivalArt {
    enemy_mesh: Mesh,
    gem: Mesh,
    halo: Mesh,
    flame: Mesh,
    toon: ToonShader,
    thruster: ThrusterShader,
}

impl SurvivalArt {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            enemy_mesh: mesh::load_obj(asset_path("assets/models/enemigo.obj"))?,
            gem: gem_mesh(8),
            halo: mesh::icosphere(2),
            flame: mesh::cone(10, 3),
            toon: ToonShader { ambient: 0.32, outline: 0.35 },
            thruster: ThrusterShader { noise: Perlin::new(WORLD_SEED + 99) },
        })
    }

    pub fn enemy_triangles(&self) -> usize {
        self.enemy_mesh.triangle_count()
    }

    /// Dibuja gemas, efectos y enemigo. El renderer ya tiene la vista de la
    /// cámara de este cuadro (se llama justo después de `scene.render`).
    pub fn draw(&self, r: &mut Renderer, camera_pos: Vec3, time: f32, s: &Survival, planets: &[Planet]) {
        let base = Uniforms { time, camera_pos, sun_pos: Vec3::ZERO, model: Mat4::IDENTITY, object_center: Vec3::ZERO };
        let solid = DrawOptions::default();
        let glow = DrawOptions { double_sided: false, depth_write: false, additive: true };

        // --- Enemigo (sólido) --------------------------------------------------
        let e = &s.enemy;
        let model = Mat4::translation(e.pos) * Mat4::from_basis(e.right(), e.up, -e.forward) * Mat4::uniform_scale(ENEMY_SCALE);
        let u = Uniforms { model, object_center: e.pos, ..base };
        r.draw_mesh(&self.enemy_mesh, &model, &self.toon, &u, DrawOptions { double_sided: true, ..solid });

        // --- Gemas (sólidas) ---------------------------------------------------
        for g in s.gems.iter().filter(|g| g.collected_at.is_none()) {
            let p = g.position(planets, time);
            let bob = 0.15 * (time * 2.0 + g.phase).sin();
            let model = Mat4::translation(p + Vec3::Y * bob)
                * Mat4::rotation_y(time * 1.6 + g.phase)
                * Mat4::uniform_scale(GEM_SIZE);
            let u = Uniforms { model, object_center: p, ..base };
            r.draw_mesh(&self.gem, &model, &GemDraw { color: g.color }, &u, solid);
        }

        // --- Brillos (aditivos, al final) ---------------------------------------
        // Flamas de los tres motores del enemigo.
        if e.stun <= 0.0 {
            for x in [-0.55f32, 0.0, 0.55] {
                let fm = model_flame(e, x);
                let u = Uniforms { model: fm, ..base };
                let draw = ThrusterDraw { shader: &self.thruster, params: ThrusterParams { intensity: 1.0, boost: false } };
                r.draw_mesh(&self.flame, &fm, &draw, &u, DrawOptions { double_sided: true, ..glow });
            }
        }
        for g in &s.gems {
            match g.collected_at {
                None => {
                    // Halo que respira alrededor de la gema.
                    let p = g.position(planets, time);
                    let pulse = 1.0 + 0.12 * (time * 3.0 + g.phase).sin();
                    let model = Mat4::translation(p) * Mat4::uniform_scale(GEM_SIZE * 2.2 * pulse);
                    let u = Uniforms { model, ..base };
                    r.draw_mesh(&self.halo, &model, &GlowDraw { color: g.color, intensity: 0.55 }, &u, glow);
                }
                Some(t0) if time - t0 < BURST_TIME => {
                    // Efecto al recogerla: destello que crece y se apaga, y
                    // fragmentos que salen girando en todas direcciones.
                    let k = ((time - t0) / BURST_TIME).clamp(0.0, 1.0);
                    let center = g.position(planets, t0);
                    let fade = (1.0 - k) * (1.0 - k);
                    let flash = Mat4::translation(center) * Mat4::uniform_scale(GEM_SIZE * (1.0 + 3.5 * k.sqrt()));
                    let u = Uniforms { model: flash, ..base };
                    let bright = g.color.lerp(Vec3::ONE, 0.3);
                    r.draw_mesh(&self.halo, &flash, &GlowDraw { color: bright, intensity: 1.2 * fade * fade }, &u, glow);
                    for i in 0..SHARDS {
                        let d = shard_dir(i);
                        let pos = center + d * (4.5 * k.sqrt());
                        let m = Mat4::translation(pos)
                            * Mat4::rotation_y(k * 9.0 + i as f32)
                            * Mat4::rotation_x(k * 7.0 + i as f32 * 0.5)
                            * Mat4::uniform_scale(GEM_SIZE * 0.5 * (1.0 - k * 0.7));
                        let u = Uniforms { model: m, ..base };
                        // Fragmentos sólidos del color de la gema, que se apagan.
                        let shard = GemDraw { color: g.color * (0.4 + 0.8 * fade) };
                        r.draw_mesh(&self.gem, &m, &shard, &u, solid);
                    }
                }
                Some(_) => {}
            }
        }
    }
}

/// Matriz de la flama de un motor del enemigo (en el .obj las toberas
/// terminan en z = 1.3, en x = -0.55, 0 y 0.55).
fn model_flame(e: &Enemy, x: f32) -> Mat4 {
    Mat4::translation(e.pos)
        * Mat4::from_basis(e.right(), e.up, -e.forward)
        * Mat4::uniform_scale(ENEMY_SCALE)
        * Mat4::translation(Vec3::new(x, 0.0, 1.28))
        * Mat4::scale(Vec3::new(0.18, 0.18, 1.4))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::ShipInput;

    #[test]
    fn gem_mesh_is_closed_and_outward() {
        let m = gem_mesh(8);
        assert_eq!(m.triangle_count(), 32);
        for t in &m.indices {
            let [a, b, c] = t.map(|i| m.positions[i as usize]);
            let n = (b - a).cross(c - a);
            let center = (a + b + c) * (1.0 / 3.0) - Vec3::new(0.0, -0.1, 0.0);
            assert!(n.dot(center) > 0.0, "cara hacia dentro");
        }
    }

    #[test]
    fn enemy_chases_and_hits_three_times_then_lost() {
        let planets = Vec::new();
        let mut ship = Ship::new(Vec3::new(0.0, 0.0, 0.0), -Vec3::Z);
        let mut s = Survival::new(&planets, &ship);
        // Sin gemas, "ganar" sería inmediato: se agrega una inalcanzable.
        s.gems.clear();
        s.gems.push(Gem { planet: 0, color: Vec3::ONE, axis: Vec3::Y, phase: 0.0, collected_at: Some(0.0) });
        s.gems.push(Gem { planet: 0, color: Vec3::ONE, axis: Vec3::Y, phase: 0.0, collected_at: None });
        // La nave quieta: el enemigo debe alcanzarla y golpearla 3 veces.
        let idle = ShipInput::default();
        let mut hits = 0;
        for _ in 0..(60 * 60) {
            ship.update(&idle, 1.0 / 60.0, &[]);
            // Sin planetas no hay gemas que revisar: solo enemigo y golpes.
            let ev = s.update_enemy_only(1.0 / 60.0, &mut ship);
            hits += ev.iter().filter(|e| **e == Event::Hit).count();
            if s.outcome.is_some() {
                break;
            }
        }
        assert_eq!(hits, 3);
        assert_eq!(s.hearts, 0);
        assert_eq!(s.outcome, Some(Outcome::Lost));
    }
}

#[cfg(test)]
impl Survival {
    /// Igual que `update` pero sin gemas (para probar al enemigo solo).
    fn update_enemy_only(&mut self, dt: f32, ship: &mut Ship) -> Vec<Event> {
        self.invulnerable = (self.invulnerable - dt).max(0.0);
        self.accumulator += dt;
        while self.accumulator >= PHYSICS_DT {
            self.accumulator -= PHYSICS_DT;
            self.enemy_step(PHYSICS_DT, ship, &[]);
        }
        self.check_hit(ship).into_iter().collect()
    }
}
