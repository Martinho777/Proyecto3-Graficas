//! Nave: estado, física de vuelo y matriz de modelo.
//!
//! La física corre con un paso de tiempo FIJO (`PHYSICS_DT`). Cada cuadro se
//! acumula el tiempo real transcurrido y se ejecutan tantos pasos fijos como
//! quepan; el sobrante se usa para interpolar entre el estado anterior y el
//! actual al dibujar. Así la nave acelera, frena y gira exactamente igual en
//! una computadora a 30 FPS que en otra a 144 FPS.

use crate::collision::{Collider, Surface};
use crate::config::*;
use crate::math::{Mat4, Vec3};

/// Parámetros de vuelo y de dibujo de un tipo de nave (ver `SHIP_SPECS` en
/// `config.rs`).
pub struct ShipSpec {
    pub name: &'static str,
    /// Frase corta para la pantalla de inicio.
    pub tagline: &'static str,
    /// Ruta del .obj (relativa a la carpeta del proyecto).
    pub model: &'static str,
    pub scale: f32,
    pub thrust: f32,
    pub reverse: f32,
    pub lift: f32,
    pub boost_mult: f32,
    pub drag: f32,
    pub brake_drag: f32,
    pub pitch_rate: f32,
    pub yaw_rate: f32,
    pub roll_rate: f32,
    pub turn_response: f32,
    /// Amortiguamiento de la velocidad lateral (1/s).
    pub grip: f32,
    pub restitution: f32,
    /// Toberas: (posición en coordenadas del modelo, ancho de la flama).
    pub flames: &'static [(Vec3, f32)],
    /// Multiplicador del largo de la flama.
    pub flame_len: f32,
    /// Para la pantalla de inicio: (velocidad, control) de 1 a 5.
    pub stats: (u8, u8),
}

/// Lo que pide quien juega en este instante. Ejes en [-1, 1].
#[derive(Clone, Copy, Default)]
pub struct ShipInput {
    /// +1 acelerar (W), -1 frenar / reversa (S).
    pub thrust: f32,
    /// Empuje vertical (Espacio / Ctrl).
    pub lift: f32,
    pub boost: bool,
    /// Cabeceo: +1 sube la nariz.
    pub pitch: f32,
    /// Guiñada: +1 gira a la derecha.
    pub yaw: f32,
    /// Alabeo: +1 rueda a la derecha.
    pub roll: f32,
}

/// Estado físico instantáneo. Se guarda el anterior para interpolar.
#[derive(Clone, Copy)]
pub struct ShipState {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Base ortonormal de la nave (hacia dónde apunta la nariz y el techo).
    pub forward: Vec3,
    pub up: Vec3,
}

impl ShipState {
    pub fn right(&self) -> Vec3 {
        self.forward.cross(self.up).normalize()
    }

    /// Mezcla dos estados (para dibujar entre dos pasos de física).
    fn interpolate(&self, o: &ShipState, t: f32) -> ShipState {
        let forward = self.forward.lerp(o.forward, t).normalize();
        let up = self.up.lerp(o.up, t);
        let right = forward.cross(up).normalize();
        ShipState {
            pos: self.pos.lerp(o.pos, t),
            vel: self.vel.lerp(o.vel, t),
            forward,
            up: right.cross(forward),
        }
    }
}

/// Enganche de órbita automática alrededor de un cuerpo (tecla O).
#[derive(Clone, Copy, Debug)]
pub struct OrbitLock {
    /// Índice del cuerpo en la lista de colliders.
    pub body: usize,
    pub name: &'static str,
    /// Distancia deseada al centro del cuerpo.
    pub radius: f32,
    /// Eje del plano de la órbita (unitario). La nave avanza en la dirección
    /// `axis × n`, donde `n` es la vertical local.
    pub axis: Vec3,
    /// Velocidad tangencial (unidades/s).
    pub speed: f32,
}

/// Resultado de presionar O.
pub enum OrbitToggle {
    Engaged(&'static str),
    Released,
    /// No hay ningún planeta lo bastante cerca.
    TooFar,
}

pub struct Ship {
    prev: ShipState,
    /// Índice en `SHIP_SPECS` (qué nave es).
    pub kind: usize,
    /// Órbita automática activa (None = vuelo libre).
    pub orbit: Option<OrbitLock>,
    pub state: ShipState,
    /// Velocidad angular actual (rad/s) en ejes locales: (cabeceo, guiñada, alabeo).
    ang_vel: Vec3,
    /// Potencia del motor suavizada en [0, 1] (para la flama).
    pub throttle: f32,
    pub boosting: bool,
    /// Segundos que quedan mostrando el aviso de contacto en el HUD.
    pub contact_timer: f32,
    /// Con qué cuerpo fue el último contacto.
    pub contact_with: &'static str,
    accumulator: f32,
}

impl Ship {
    /// Nave equilibrada (el cohete). Solo la usan las pruebas.
    #[cfg(test)]
    pub fn new(pos: Vec3, forward: Vec3) -> Self {
        Self::with_kind(0, pos, forward)
    }

    pub fn with_kind(kind: usize, pos: Vec3, forward: Vec3) -> Self {
        let forward = forward.normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let state = ShipState { pos, vel: Vec3::ZERO, forward, up: right.cross(forward) };
        Self {
            prev: state,
            kind: kind.min(SHIP_SPECS.len() - 1),
            state,
            orbit: None,
            ang_vel: Vec3::ZERO,
            throttle: 0.0,
            boosting: false,
            contact_timer: 0.0,
            contact_with: "",
            accumulator: 0.0,
        }
    }

    /// Avanza la simulación `dt` segundos reales con pasos fijos.
    /// `colliders` son los cuerpos sólidos en este instante.
    pub fn update(&mut self, input: &ShipInput, dt: f32, colliders: &[Collider]) {
        self.contact_timer = (self.contact_timer - dt).max(0.0);
        self.accumulator += dt;
        while self.accumulator >= PHYSICS_DT {
            self.prev = self.state;
            self.step(input, PHYSICS_DT, colliders);
            self.accumulator -= PHYSICS_DT;
        }
    }

    /// Estado listo para dibujar (interpolado entre los dos últimos pasos).
    pub fn render_state(&self) -> ShipState {
        self.prev.interpolate(&self.state, self.accumulator / PHYSICS_DT)
    }

    pub fn spec(&self) -> &'static ShipSpec {
        &SHIP_SPECS[self.kind]
    }

    pub fn speed(&self) -> f32 {
        self.state.vel.length()
    }

    /// Empujón instantáneo (golpe del enemigo). Suelta la órbita automática.
    pub fn knockback(&mut self, dv: Vec3) {
        self.orbit = None;
        self.state.vel += dv;
        self.contact_timer = 0.0;
    }

    /// Planeta más cercano (por altura sobre su superficie) y esa altura.
    pub fn nearest_planet(&self, colliders: &[Collider]) -> Option<(usize, f32)> {
        colliders
            .iter()
            .enumerate()
            .filter(|(_, c)| matches!(c.surface, Surface::Terrain { .. }))
            .map(|(i, c)| (i, c.signed_distance(self.state.pos)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Engancha o suelta la órbita automática.
    pub fn toggle_orbit(&mut self, colliders: &[Collider]) -> OrbitToggle {
        if self.orbit.take().is_some() {
            // Al soltar, la nave conserva su velocidad: el vuelo no se corta.
            return OrbitToggle::Released;
        }
        let Some((body, altitude)) = self.nearest_planet(colliders) else { return OrbitToggle::TooFar };
        if altitude > ORBIT_ENGAGE_ALTITUDE {
            return OrbitToggle::TooFar;
        }
        let c = &colliders[body];
        let s = &self.state;
        let r = s.pos - c.center;
        let n = r.normalize();
        // El plano de la órbita sigue la dirección en la que ya se movía la
        // nave (o hacia donde apunta, si iba muy lento).
        let rel = s.vel - c.velocity;
        let tangent = rel - n * rel.dot(n);
        let heading = if tangent.length() > 1.0 { tangent } else { s.forward - n * s.forward.dot(n) };
        let mut axis = n.cross(heading);
        if axis.length() < 1e-3 {
            axis = n.cross(if n.y.abs() < 0.9 { Vec3::Y } else { Vec3::X });
        }
        let min_radius = c.radius_toward(s.pos) + ORBIT_MIN_ALTITUDE;
        self.orbit = Some(OrbitLock {
            body,
            name: c.name,
            radius: r.length().clamp(min_radius, min_radius - ORBIT_MIN_ALTITUDE + ORBIT_PREFERRED_ALTITUDE.max(ORBIT_MIN_ALTITUDE)),
            axis: axis.normalize(),
            speed: tangent.length().clamp(ORBIT_DEFAULT_SPEED, ORBIT_MAX_SPEED),
        });
        OrbitToggle::Engaged(c.name)
    }

    /// Un paso de la órbita automática: la nave se mueve sola en círculo
    /// alrededor del planeta (que también se mueve) a altura constante.
    fn orbit_step(&mut self, input: &ShipInput, dt: f32, colliders: &[Collider]) {
        let Some(mut o) = self.orbit else { return };
        let Some(c) = colliders.get(o.body) else {
            self.orbit = None;
            return;
        };
        let s = &mut self.state;
        let r = s.pos - c.center;
        let n = r.normalize();

        // Controles dentro de la órbita.
        o.speed = (o.speed + input.thrust * ORBIT_ACCEL * dt).clamp(ORBIT_MIN_SPEED, ORBIT_MAX_SPEED);
        o.radius += input.lift * ORBIT_CLIMB * dt;
        o.axis = o.axis.rotate_around(n, input.roll * ORBIT_TILT_RATE * dt);
        // Nunca por debajo del terreno que hay justo abajo.
        o.radius = o.radius.max(c.radius_toward(s.pos) + ORBIT_MIN_ALTITUDE);

        // El eje se mantiene perpendicular a la vertical; la dirección de
        // avance es tangente a la esfera.
        o.axis = (o.axis - n * o.axis.dot(n)).normalize();
        let tangent = o.axis.cross(n);

        // Velocidad deseada: la del planeta + avance tangencial + un resorte
        // que corrige la altura. Se alcanza de forma suave (exponencial).
        let target = c.velocity + tangent * o.speed + n * ((o.radius - r.length()) * ORBIT_SPRING);
        s.vel = s.vel.lerp(target, 1.0 - (-ORBIT_RESPONSE * dt).exp());
        s.pos += s.vel * dt;

        // La nave mira hacia donde avanza, con el techo hacia fuera del planeta.
        let k = 1.0 - (-4.0 * dt).exp();
        s.forward = s.forward.lerp(tangent, k).normalize();
        s.up = s.up.lerp(n, k);
        let right = s.forward.cross(s.up).normalize();
        s.up = right.cross(s.forward);
        self.ang_vel = Vec3::ZERO;

        self.orbit = Some(o);
        self.boosting = false;
        let target_thr = 0.25 + 0.5 * (o.speed / ORBIT_MAX_SPEED);
        self.throttle += (target_thr - self.throttle) * (1.0 - (-8.0 * dt).exp());
        self.resolve_collisions(colliders);
    }

    fn resolve_collisions(&mut self, colliders: &[Collider]) {
        let restitution = self.spec().restitution;
        let s = &mut self.state;
        for c in colliders {
            if let Some(hit) = c.resolve(&mut s.pos, &mut s.vel, SHIP_COLLISION_RADIUS, restitution, SHIP_FRICTION) {
                self.contact_with = c.name;
                if hit.impact_speed > 0.5 || self.contact_timer > 0.0 {
                    self.contact_timer = 1.0;
                }
            }
        }
    }

    /// Un paso de física de duración fija `dt`.
    fn step(&mut self, input: &ShipInput, dt: f32, colliders: &[Collider]) {
        if self.orbit.is_some() {
            self.orbit_step(input, dt, colliders);
            return;
        }
        let spec = self.spec();
        // --- Giro progresivo -------------------------------------------------
        // La velocidad angular se acerca a la deseada de forma exponencial:
        // no gira de golpe ni se detiene de golpe.
        let target = Vec3::new(
            input.pitch * spec.pitch_rate,
            input.yaw * spec.yaw_rate,
            input.roll * spec.roll_rate,
        );
        let k = 1.0 - (-spec.turn_response * dt).exp();
        self.ang_vel = self.ang_vel.lerp(target, k);

        let s = &mut self.state;
        let right = s.right();
        // Cabeceo: alrededor del eje derecho. Guiñada: alrededor del techo.
        // Alabeo: alrededor de la nariz.
        let (pitch, yaw, roll) = (self.ang_vel.x * dt, self.ang_vel.y * dt, self.ang_vel.z * dt);
        s.forward = s.forward.rotate_around(right, pitch);
        s.up = s.up.rotate_around(right, pitch);
        s.forward = s.forward.rotate_around(s.up, -yaw);
        s.up = s.up.rotate_around(s.forward, roll);
        // Re-ortonormalizar para que los errores numéricos no se acumulen.
        s.forward = s.forward.normalize();
        let right = s.forward.cross(s.up).normalize();
        s.up = right.cross(s.forward);

        // --- Aceleración y frenado progresivos -------------------------------
        self.boosting = input.boost && input.thrust > 0.0;
        let boost = if self.boosting { spec.boost_mult } else { 1.0 };
        let mut accel = Vec3::ZERO;
        if input.thrust > 0.0 {
            accel += s.forward * (spec.thrust * boost * input.thrust);
        } else if input.thrust < 0.0 {
            // Reversa suave (útil para retroceder cerca de un planeta).
            accel += s.forward * (spec.reverse * input.thrust);
        }
        accel += s.up * (spec.lift * input.lift);

        s.vel += accel * dt;

        // Rozamiento exponencial: define una velocidad máxima "natural"
        // (thrust / drag) y hace que la nave se detenga sola poco a poco.
        // Al frenar (S) el rozamiento es mucho mayor.
        let drag = if input.thrust < 0.0 { spec.brake_drag } else { spec.drag };
        s.vel = s.vel * (-drag * dt).exp();

        // Agarre: se elimina poco a poco la velocidad que no va en la
        // dirección de la nariz (y del empuje vertical). Con agarre alto la
        // nave "dobla" como un vehículo; con cero derrapa como en el espacio.
        if spec.grip > 0.0 {
            let along = s.forward * s.vel.dot(s.forward) + s.up * s.vel.dot(s.up);
            let lateral = s.vel - along;
            s.vel = along + lateral * (-spec.grip * dt).exp();
        }

        s.pos += s.vel * dt;

        // --- Colisión: la nave no puede atravesar planetas ni el sol ---------
        self.resolve_collisions(colliders);

        // Potencia visual del motor (flama), también suavizada.
        let target_thr = (input.thrust.max(0.0) * boost / spec.boost_mult).max(input.lift.abs() * 0.3);
        self.throttle += (target_thr - self.throttle) * (1.0 - (-8.0 * dt).exp());
    }
}

/// Matriz de modelo de la nave. El modelo .obj mira hacia -Z local, así que
/// el eje Z local corresponde a "atrás" (-forward) en mundo.
pub fn model_matrix(s: &ShipState, scale: f32) -> Mat4 {
    let right = s.right();
    Mat4::translation(s.pos) * Mat4::from_basis(right, s.up, -s.forward) * Mat4::uniform_scale(scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simula `secs` segundos con cuadros de duración `frame_dt` y la misma
    /// secuencia de controles (acelerar + girar, luego frenar).
    fn fly(frame_dt: f32, secs: f32) -> ShipState {
        let mut ship = Ship::new(Vec3::ZERO, -Vec3::Z);
        // Se cuentan cuadros enteros: acumular el tiempo en f32 haría que
        // cada framerate terminara en un instante ligeramente distinto.
        let frames = (secs / frame_dt).round() as u32;
        for i in 0..frames {
            let t = i as f32 * frame_dt;
            let input = if t < secs * 0.6 {
                ShipInput { thrust: 1.0, yaw: 0.5, pitch: 0.2, roll: 0.3, ..Default::default() }
            } else {
                ShipInput { thrust: -1.0, ..Default::default() }
            };
            ship.update(&input, frame_dt, &[]);
        }
        ship.state
    }

    #[test]
    fn same_result_at_30_and_144_fps() {
        // 3 s es múltiplo exacto de 1/30, 1/60, 1/120 y casi de 1/144.
        let a = fly(1.0 / 30.0, 3.0);
        let b = fly(1.0 / 144.0, 3.0);
        let c = fly(1.0 / 60.0, 3.0);
        // A lo sumo un paso de física de diferencia (el cambio de controles
        // cae dentro de un cuadro distinto según el framerate).
        assert!((a.pos - b.pos).length() < 0.05, "{:?} vs {:?}", a.pos, b.pos);
        assert!((a.pos - c.pos).length() < 1e-3);
        assert!((a.forward - c.forward).length() < 1e-4);
    }

    #[test]
    fn basis_stays_orthonormal() {
        let s = fly(1.0 / 60.0, 20.0);
        assert!((s.forward.length() - 1.0).abs() < 1e-4);
        assert!((s.up.length() - 1.0).abs() < 1e-4);
        assert!(s.forward.dot(s.up).abs() < 1e-4);
    }

    #[test]
    fn cannot_fly_through_planet() {
        use crate::collision::Collider;
        let planet = Collider::sphere(Vec3::new(0.0, 0.0, -30.0), Vec3::ZERO, 7.0, "p");
        let mut ship = Ship::new(Vec3::ZERO, -Vec3::Z);
        // Turbo a fondo directo hacia el planeta durante 5 s.
        let input = ShipInput { thrust: 1.0, boost: true, ..Default::default() };
        for _ in 0..300 {
            ship.update(&input, 1.0 / 60.0, &[planet]);
            let d = planet.signed_distance(ship.state.pos);
            assert!(d >= SHIP_COLLISION_RADIUS - 1e-3, "atravesó la superficie: {d}");
        }
        assert_eq!(ship.contact_with, "p");
    }

    /// Acelera a fondo en línea recta `secs` segundos y devuelve la rapidez.
    fn top_speed(kind: usize, secs: f32) -> f32 {
        let mut ship = Ship::with_kind(kind, Vec3::ZERO, -Vec3::Z);
        let input = ShipInput { thrust: 1.0, ..Default::default() };
        for _ in 0..(secs * 60.0) as u32 {
            ship.update(&input, 1.0 / 60.0, &[]);
        }
        ship.speed()
    }

    #[test]
    fn light_is_fastest_and_heavy_slowest() {
        let (bal, light, heavy) = (top_speed(0, 15.0), top_speed(1, 15.0), top_speed(2, 15.0));
        assert!(light > bal * 1.5, "ligera {light} vs cohete {bal}");
        assert!(heavy < bal * 0.7, "pesada {heavy} vs cohete {bal}");
    }

    /// Acelera, luego gira 90° a la derecha sin soltar W y mide cuánta
    /// velocidad queda "de lado" (derrape) respecto de la nariz.
    fn drift_after_turn(kind: usize) -> f32 {
        let mut ship = Ship::with_kind(kind, Vec3::ZERO, -Vec3::Z);
        let go = ShipInput { thrust: 1.0, ..Default::default() };
        for _ in 0..240 {
            ship.update(&go, 1.0 / 60.0, &[]);
        }
        let turn = ShipInput { thrust: 1.0, yaw: 1.0, ..Default::default() };
        let mut t = 0.0;
        while ship.state.forward.dot(-Vec3::Z) > 0.0 && t < 10.0 {
            ship.update(&turn, 1.0 / 60.0, &[]);
            t += 1.0 / 60.0;
        }
        let v = ship.state.vel;
        let lateral = v - ship.state.forward * v.dot(ship.state.forward);
        lateral.length() / v.length()
    }

    #[test]
    fn heavy_goes_where_it_points_light_drifts() {
        let (light, heavy) = (drift_after_turn(1), drift_after_turn(2));
        assert!(heavy < 0.3, "la pesada casi no derrapa: {heavy}");
        assert!(light > heavy * 2.0, "la ligera derrapa más: {light} vs {heavy}");
    }

    #[test]
    fn heavy_stops_and_settles_faster() {
        // Misma velocidad inicial: la pesada frena antes.
        let brake = ShipInput { thrust: -1.0, ..Default::default() };
        let stop_time = |kind: usize| {
            let mut ship = Ship::with_kind(kind, Vec3::ZERO, -Vec3::Z);
            ship.state.vel = -Vec3::Z * 12.0;
            let mut t = 0.0;
            while ship.speed() > 1.0 && t < 30.0 {
                ship.update(&brake, 1.0 / 60.0, &[]);
                t += 1.0 / 60.0;
            }
            t
        };
        assert!(stop_time(2) < stop_time(0) && stop_time(0) < stop_time(1));
    }

    #[test]
    fn orbit_mode_circles_planet_at_constant_altitude() {
        use crate::collision::{Collider, Surface};
        use crate::math::Mat4;
        use crate::planet::{Orbit, Planet, PlanetDef, Terrain};
        use crate::shaders::ToonShader;

        let def = PlanetDef {
            name: "p",
            radius: 7.0,
            orbit: Orbit { radius: 0.0, speed: 0.0, phase: 0.0, inclination: 0.0 },
            spin_speed: 0.0,
            axial_tilt: 0.0,
            seed: 1,
            terrain: Terrain::FLAT,
        };
        let planet = Planet::new(def, Box::new(ToonShader { ambient: 0.3, outline: 0.0 }));
        let c = Collider {
            center: Vec3::ZERO,
            velocity: Vec3::ZERO,
            surface: Surface::Terrain { planet: &planet, inv_rotation: Mat4::IDENTITY },
            name: "p",
        };

        // Muy lejos: no se puede enganchar.
        let mut far = Ship::new(Vec3::new(0.0, 0.0, 60.0), -Vec3::X);
        assert!(matches!(far.toggle_orbit(&[c]), OrbitToggle::TooFar));

        let mut ship = Ship::new(Vec3::new(0.0, 0.0, 12.0), -Vec3::X);
        assert!(matches!(ship.toggle_orbit(&[c]), OrbitToggle::Engaged("p")));

        let idle = ShipInput::default();
        let mut angle = 0.0;
        let mut prev = ship.state.pos;
        for i in 0..600 {
            ship.update(&idle, 1.0 / 60.0, &[c]);
            let p = ship.state.pos;
            angle += prev.normalize().dot(p.normalize()).clamp(-1.0, 1.0).acos();
            prev = p;
            // Baja a la altura preferida sobre el terreno y se queda ahí.
            if i > 240 {
                let expected = 7.0 + ORBIT_PREFERRED_ALTITUDE;
                // El resorte deja un desfase pequeño por la fuerza centrífuga.
                assert!((p.length() - expected).abs() < 0.5, "altura estable: {}", p.length());
            }
        }
        // 10 s a 5 u/s sobre un radio de ~10: más de 4 radianes de vuelta.
        assert!(angle > 3.0, "debe rodear el planeta solo: {angle}");
        // La nave mira hacia donde avanza y su techo apunta hacia fuera.
        assert!(ship.state.up.dot(ship.state.pos.normalize()) > 0.95);

        // Al soltar conserva la velocidad (el vuelo no se corta).
        let v = ship.state.vel;
        assert!(matches!(ship.toggle_orbit(&[c]), OrbitToggle::Released));
        assert!(ship.orbit.is_none() && (ship.state.vel - v).length() < 1e-6);
    }
}
