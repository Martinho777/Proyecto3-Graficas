//! Parámetros ajustables del proyecto. Para la demo se cambian aquí
//! (radios, velocidades, semillas, intensidades) y se recompila.

/// Resolución interna del framebuffer (se escala a la ventana).
pub const RENDER_W: usize = 480;
pub const RENDER_H: usize = 270;

/// Tamaño inicial de la ventana.
pub const WINDOW_W: i32 = 1440;
pub const WINDOW_H: i32 = 810;

/// Cámara.
pub const FOV_Y_DEG: f32 = 65.0;
pub const NEAR: f32 = 0.05;
pub const FAR: f32 = 600.0;

/// Semilla global del sistema (forma de planetas, distribución, estrellas).
pub const WORLD_SEED: u64 = 2026;

/// Sol.
pub const SUN_RADIUS: f32 = 12.0;
/// Velocidad de la animación de la superficie del sol.
pub const SUN_ANIM_SPEED: f32 = 0.35;

/// Niveles de detalle de los planetas: subdivisiones de la icosfera de cada
/// nivel (320, 1280, 5120 y 20480 triángulos).
pub const LOD_SUBDIVISIONS: [u32; 4] = [2, 3, 4, 5];
/// Radio en pantalla (pixeles) a partir del cual se usa el siguiente nivel.
pub const LOD_PIXEL_THRESHOLDS: [f32; 3] = [18.0, 55.0, 140.0];

/// Los objetos de superficie solo se dibujan cuando el planeta mide al menos
/// esto en pantalla (radio en pixeles). De lejos no se distinguen.
pub const PROPS_MIN_PX: f32 = 90.0;

/// Escala global de los planetas y de sus órbitas. Cada planeta se define en
/// su archivo con sus medidas "de diseño" y al final todo el sistema se
/// agranda con estos factores (el planeta, su relieve y sus objetos crecen
/// juntos; la nave no). Las órbitas crecen igual para que no se encimen.
pub const PLANET_SIZE_SCALE: f32 = 1.4;
pub const ORBIT_SCALE: f32 = 1.4;

// Los parámetros de cada planeta (radio, órbita, semilla…) están en
// `src/planets.rs`.

/// Cámara libre de depuración (tecla F2).
pub const FREEFLY_SPEED: f32 = 8.0;
pub const MOUSE_SENS: f32 = 0.0025;

// --- Nave -------------------------------------------------------------------

/// Escala del modelo (el .obj mide ~2 unidades de largo).
pub const SHIP_SCALE: f32 = 0.5;
/// Paso fijo de la física: 120 pasos por segundo sin importar los FPS.
pub const PHYSICS_DT: f32 = 1.0 / 120.0;
/// Aceleración del motor (unidades/s²). Velocidad máxima ≈ THRUST / DRAG.
pub const SHIP_THRUST: f32 = 12.0;
pub const SHIP_REVERSE: f32 = 4.0;
pub const SHIP_LIFT: f32 = 5.0;
pub const SHIP_BOOST_MULT: f32 = 3.5;
/// Rozamiento normal (1/s) y al frenar con S.
pub const SHIP_DRAG: f32 = 0.45;
pub const SHIP_BRAKE_DRAG: f32 = 2.2;
/// Velocidades de giro máximas (rad/s).
pub const SHIP_PITCH_RATE: f32 = 1.6;
pub const SHIP_YAW_RATE: f32 = 1.2;
pub const SHIP_ROLL_RATE: f32 = 2.6;
/// Qué tan rápido la velocidad angular alcanza la deseada (1/s).
pub const SHIP_TURN_RESPONSE: f32 = 5.0;

// --- Tipos de nave -----------------------------------------------------------
// Las constantes SHIP_* de arriba son las del cohete (nave equilibrada) y se
// usan como referencia (p. ej. el efecto de velocidad de la cámara). Cada nave
// tiene su propio juego de parámetros de vuelo:
//   - más THRUST / menos DRAG = más velocidad máxima (≈ thrust / drag);
//   - TURN_RESPONSE alto = el giro responde y se detiene al instante (preciso),
//     bajo = la nave "flota" y se pasa un poco del giro;
//   - GRIP = cuánto se elimina por segundo la velocidad lateral (0 = derrapa
//     como en el espacio, alto = la nave va hacia donde apunta la nariz).

use crate::math::Vec3;
use crate::ship::ShipSpec;

pub const SHIP_SPECS: [ShipSpec; 3] = [
    ShipSpec {
        name: "Pikmin Ship",
        tagline: "Equilibrada: ni muy rapida ni muy lenta",
        model: "assets/models/nave.obj",
        scale: SHIP_SCALE,
        thrust: SHIP_THRUST,
        reverse: SHIP_REVERSE,
        lift: SHIP_LIFT,
        boost_mult: SHIP_BOOST_MULT,
        drag: SHIP_DRAG,
        brake_drag: SHIP_BRAKE_DRAG,
        pitch_rate: SHIP_PITCH_RATE,
        yaw_rate: SHIP_YAW_RATE,
        roll_rate: SHIP_ROLL_RATE,
        turn_response: SHIP_TURN_RESPONSE,
        grip: 0.0,
        restitution: SHIP_RESTITUTION,
        flames: &[(Vec3::new(0.0, 0.0, 1.1), 0.17)],
        flame_len: 1.0,
        stats: (3, 3),
    },
    ShipSpec {
        name: "Starfoz",
        tagline: "Ligera: muy rapida, pero derrapa y cuesta frenarla",
        model: "assets/models/nave_ligera.obj",
        scale: 0.45,
        thrust: 22.0,
        reverse: 3.0,
        lift: 6.0,
        boost_mult: 2.8,
        drag: 0.42,
        brake_drag: 1.3,
        pitch_rate: 2.0,
        yaw_rate: 1.6,
        roll_rate: 3.6,
        turn_response: 2.5,
        grip: 0.0,
        restitution: 0.45,
        flames: &[(Vec3::new(0.0, 0.0, 1.12), 0.11)],
        flame_len: 1.5,
        stats: (5, 1),
    },
    ShipSpec {
        name: "Tortuga",
        tagline: "Pesada: lenta, pero precisa y frena en seco",
        model: "assets/models/nave_pesada.obj",
        scale: 0.5,
        thrust: 6.5,
        reverse: 5.0,
        lift: 4.5,
        boost_mult: 2.5,
        drag: 0.45,
        brake_drag: 4.5,
        pitch_rate: 1.3,
        yaw_rate: 1.0,
        roll_rate: 1.8,
        turn_response: 12.0,
        grip: 3.0,
        restitution: 0.08,
        flames: &[(Vec3::new(1.08, -0.08, 1.06), 0.15), (Vec3::new(-1.08, -0.08, 1.06), 0.15)],
        flame_len: 0.8,
        stats: (1, 5),
    },
];

// --- Modo órbita (tecla O) ---------------------------------------------------

/// Altura máxima sobre la superficie para poder engancharse a una órbita.
pub const ORBIT_ENGAGE_ALTITUDE: f32 = 18.0;
/// Altura mínima de la órbita sobre el terreno (no rozar montañas ni objetos).
pub const ORBIT_MIN_ALTITUDE: f32 = 1.5;
/// Al engancharse, la nave baja hasta esta altura sobre el terreno (si venía
/// más alta) para ver los detalles de cerca. Espacio la vuelve a subir.
pub const ORBIT_PREFERRED_ALTITUDE: f32 = 3.0;
/// Velocidad de la órbita (unidades/s): la inicial, la mínima y la máxima.
pub const ORBIT_DEFAULT_SPEED: f32 = 5.0;
pub const ORBIT_MIN_SPEED: f32 = 1.5;
pub const ORBIT_MAX_SPEED: f32 = 18.0;
/// Cuánto cambian velocidad (W/S), altura (Espacio/Ctrl) e inclinación del
/// plano de la órbita (A/D) por segundo.
pub const ORBIT_ACCEL: f32 = 6.0;
pub const ORBIT_CLIMB: f32 = 3.0;
pub const ORBIT_TILT_RATE: f32 = 0.8;
/// Rigidez del resorte que mantiene la altura y rapidez con la que la nave
/// alcanza la velocidad de la órbita (1/s).
pub const ORBIT_SPRING: f32 = 2.5;
pub const ORBIT_RESPONSE: f32 = 3.0;

/// Radio de la esfera de colisión de la nave (en mundo).
pub const SHIP_COLLISION_RADIUS: f32 = 0.5;
/// Rebote al chocar (0 = nada, 1 = rebote perfecto) y fricción con la superficie.
pub const SHIP_RESTITUTION: f32 = 0.25;
pub const SHIP_FRICTION: f32 = 0.02;
/// La cámara se mantiene al menos a esta distancia de las superficies.
pub const CAM_SURFACE_MARGIN: f32 = 0.35;

/// Mouse como palanca virtual: el movimiento del mouse desplaza la palanca,
/// que vuelve sola al centro con esta rapidez (1/s).
pub const MOUSE_STICK_SENS: f32 = 0.006;
pub const MOUSE_STICK_RECENTER: f32 = 1.8;

/// Cámara en tercera persona.
pub const CAM_DISTANCE: f32 = 2.6;
pub const CAM_HEIGHT: f32 = 0.75;
pub const CAM_LOOK_AHEAD: f32 = 2.0;
/// Cámara en modo órbita: cuánto se aleja hacia atrás y hacia arriba de la
/// nave, y qué tanto inclina la mirada hacia el centro del planeta (0 = al
/// frente, 1 = directo al centro).
pub const CAM_ORBIT_BACK: f32 = 3.0;
pub const CAM_ORBIT_UP: f32 = 2.4;
pub const CAM_ORBIT_TILT: f32 = 0.45;
/// Rapidez del seguimiento suave (1/s).
pub const CAM_FOLLOW: f32 = 7.0;

/// Número de estrellas del fondo.
pub const STAR_COUNT: usize = 900;

// --- Modo supervivencia -------------------------------------------------------

/// Corazones al empezar (cada golpe del enemigo quita uno).
pub const SURVIVAL_HEARTS: u32 = 3;
/// Segundos sin poder recibir otro golpe tras uno (la nave parpadea).
pub const HIT_INVULNERABLE: f32 = 2.0;
/// Empujón que recibe la nave al ser golpeada (unidades/s).
pub const HIT_KNOCKBACK: f32 = 16.0;

/// Nave enemiga: tamaño del modelo, radio de colisión con planetas y radio
/// de "toque" con la nave.
pub const ENEMY_SCALE: f32 = 0.9;
pub const ENEMY_RADIUS: f32 = 1.0;
pub const ENEMY_HIT_RADIUS: f32 = 1.2;
/// Velocidad máxima al empezar y cuánto sube con cada gema recogida.
pub const ENEMY_SPEED: f32 = 17.0;
pub const ENEMY_SPEED_PER_GEM: f32 = 1.2;
/// Qué tan rápido alcanza esa velocidad (1/s) y cuánto puede girar (rad/s).
/// Gira más lento que las naves del jugador: se le puede esquivar.
pub const ENEMY_RESPONSE: f32 = 1.2;
pub const ENEMY_TURN_RATE: f32 = 1.1;
/// Segundos que tarda en arrancar al empezar y que queda aturdido tras golpear.
pub const ENEMY_START_DELAY: f32 = 4.0;
pub const ENEMY_STUN: f32 = 1.5;
/// Distancia a la que aparece (detrás de la nave).
pub const ENEMY_SPAWN_DISTANCE: f32 = 80.0;

/// Gemas: tamaño, altura sobre el planeta (contando montañas y objetos),
/// velocidad de su órbita (rad/s) y distancia para recogerlas.
pub const GEM_SIZE: f32 = 0.7;
pub const GEM_ALTITUDE: f32 = 3.0;
pub const GEM_ORBIT_SPEED: f32 = 0.12;
pub const GEM_PICK_RADIUS: f32 = 1.8;
