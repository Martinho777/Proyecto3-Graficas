//! Proyecto 3 — Software Renderer: sistema solar explorable.
//!
//! raylib se usa SOLO para: abrir la ventana, leer teclado/mouse, subir el
//! framebuffer a una textura y dibujar el HUD 2D. Todo el 3D (transformación,
//! recorte, rasterización, profundidad y shaders) está en este proyecto.

mod audio;
mod camera;
mod collision;
mod config;
mod framebuffer;
mod math;
mod mesh;
mod mundos;
mod noise;
mod planet;
mod planets;
mod props;
mod renderer;
mod scene;
mod shader;
mod shaders;
mod ship;
mod starfield;
mod survival;

use raylib::prelude::*;

use camera::{CameraMode, FreeFly, ShipCamera};
use config::*;
use math::Vec3;
use renderer::Renderer;
use scene::{Scene, ShipView};
use ship::{OrbitToggle, Ship, ShipInput};
use audio::{Audio, Sfx, Track};
use survival::{Event, Outcome, Survival, SurvivalArt};

/// Reproduce un efecto (si hay audio).
fn sfx(audio: &Option<Audio>, s: Sfx) {
    if let Some(a) = audio {
        a.play(s);
    }
}

/// Modos de juego de la pantalla de inicio.
const MODES: [(&str, &str); 2] = [
    ("EXPLORADOR", "Vuela libre y aprecia el sistema solar"),
    ("SUPERVIVENCIA", "Recoge la gema de cada planeta sin que el Devorador te atrape"),
];

/// Posición inicial: afuera de la órbita más lejana, mirando hacia el sol.
fn spawn_ship(scene: &Scene, kind: usize) -> Ship {
    let outer = scene.planets.iter().map(|p| p.def.orbit.radius).fold(SUN_RADIUS * 3.0, f32::max);
    let start = Vec3::new(0.0, 6.0, outer + 30.0);
    Ship::with_kind(kind, start, Vec3::ZERO - start)
}

/// Teclas 1 / 2 / 3 → índice de nave.
fn ship_key(rl: &RaylibHandle) -> Option<usize> {
    [KeyboardKey::KEY_ONE, KeyboardKey::KEY_TWO, KeyboardKey::KEY_THREE]
        .iter()
        .position(|&k| rl.is_key_pressed(k))
        .filter(|&i| i < SHIP_SPECS.len())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--snapshot") {
        snapshot(&args);
        return;
    }

    let mut scene = match Scene::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error cargando la escena: {e}");
            std::process::exit(1);
        }
    };

    let (mut rl, thread) = raylib::init()
        .size(WINDOW_W, WINDOW_H)
        .title("SPACE CRAFT MASTERS PRO +")
        .resizable()
        .build();
    rl.set_target_fps(144);
    // En la pantalla de inicio el mouse queda libre.
    rl.enable_cursor();
    // ESC no cierra: se usa para soltar el mouse. Se sale cerrando la ventana.
    rl.set_exit_key(None);
    // Efectos de sonido (sintetizados al arrancar). Sin dispositivo de
    // audio el juego sigue en silencio.
    let mut audio = Audio::new();
    // Para detectar cuándo empieza algo (turbo, choque, fin de partida).
    let mut prev_boost = false;
    let mut prev_contact = 0.0f32;
    let mut prev_outcome: Option<Outcome> = None;

    // Textura donde se copia el framebuffer cada cuadro.
    let img = Image::gen_image_color(RENDER_W as i32, RENDER_H as i32, Color::BLACK);
    let mut screen_tex = rl.load_texture_from_image(&thread, &img).expect("textura");
    screen_tex.set_texture_filter(&thread, TextureFilter::TEXTURE_FILTER_POINT);

    let mut renderer = Renderer::new(RENDER_W, RENDER_H);
    let art = match SurvivalArt::new() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error cargando el modo supervivencia: {e}");
            std::process::exit(1);
        }
    };
    // Modo elegido (índice en MODES) y partida de supervivencia en curso.
    let mut modo = 0usize;
    let mut survival: Option<Survival> = None;
    // Vista previa de la nave del menú (su propio framebuffer y textura).
    let mut preview_r = Renderer::new(PREVIEW_W, PREVIEW_H);
    let img = Image::gen_image_color(PREVIEW_W as i32, PREVIEW_H as i32, Color::BLANK);
    let mut preview_tex = rl.load_texture_from_image(&thread, &img).expect("textura");
    preview_tex.set_texture_filter(&thread, TextureFilter::TEXTURE_FILTER_POINT);
    // Nave elegida en la pantalla de inicio (índice en SHIP_SPECS).
    let mut nave_elegida = 0usize;
    let mut ship = spawn_ship(&scene, nave_elegida);
    let mut ship_cam = ShipCamera::new();
    let mut free_cam: Option<FreeFly> = None;
    // Palanca virtual del mouse (x = guiñada, y = cabeceo), en [-1, 1].
    let mut stick = (0.0f32, 0.0f32);
    let mut time = 0.0f32;
    let mut mouse_locked = false;
    let mut show_debug = false;
    let mut en_inicio = true;
    // Ventana de controles abierta en la pantalla de inicio.
    let mut ver_controles = false;
    // Hasta qué instante una segunda M confirma volver al menú.
    let mut confirmar_salida = 0.0f32;
    // Mensaje temporal en pantalla (texto, segundos restantes).
    let mut aviso: Option<(String, f32)> = None;

    while !rl.window_should_close() {
        // Delta time acotado para que una pausa larga no "teletransporte" nada.
        let dt = rl.get_frame_time().min(0.1);
        time += dt;

        // N: silenciar / activar el sonido (en el menú y en el juego).
        if rl.is_key_pressed(KeyboardKey::KEY_N) {
            if let Some(a) = audio.as_mut() {
                a.muted = !a.muted;
                aviso = Some((if a.muted { "Sonido apagado (N)" } else { "Sonido encendido (N)" }.to_string(), 1.5));
            }
        }

        // --- Pantalla de inicio ----------------------------------------------
        // El sistema solar sigue vivo de fondo (mismo renderer, misma escena):
        // una cámara cinemática lo rodea despacio.
        if en_inicio {
            let antes = (nave_elegida, modo, ver_controles);
            // Elegir nave: flechas izquierda/derecha o 1/2/3.
            let n = SHIP_SPECS.len();
            if rl.is_key_pressed(KeyboardKey::KEY_RIGHT) {
                nave_elegida = (nave_elegida + 1) % n;
            }
            if rl.is_key_pressed(KeyboardKey::KEY_LEFT) {
                nave_elegida = (nave_elegida + n - 1) % n;
            }
            if let Some(k) = ship_key(&rl) {
                nave_elegida = k;
            }
            // Elegir modo: flechas arriba/abajo.
            if rl.is_key_pressed(KeyboardKey::KEY_UP) || rl.is_key_pressed(KeyboardKey::KEY_DOWN) {
                modo = (modo + 1) % MODES.len();
            }
            // Mouse: flechas de nave, botones de modo y botón de controles.
            let (sw, sh) = (rl.get_screen_width() as f32, rl.get_screen_height() as f32);
            let click = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
            let mouse = rl.get_mouse_position();
            if ver_controles {
                // Con la ventana de controles abierta solo se puede cerrarla.
                if click
                    || rl.is_key_pressed(KeyboardKey::KEY_C)
                    || rl.is_key_pressed(KeyboardKey::KEY_ESCAPE)
                    || rl.is_key_pressed(KeyboardKey::KEY_ENTER)
                {
                    ver_controles = false;
                }
            } else {
                let rects = MenuRects::new(sw, sh);
                if rl.is_key_pressed(KeyboardKey::KEY_C) || (click && rects.controls.check_collision_point_rec(mouse)) {
                    ver_controles = true;
                }
                if click && rects.prev_ship.check_collision_point_rec(mouse) {
                    nave_elegida = (nave_elegida + n - 1) % n;
                }
                if click && rects.next_ship.check_collision_point_rec(mouse) {
                    nave_elegida = (nave_elegida + 1) % n;
                }
                for (i, r) in rects.modes.iter().enumerate() {
                    if click && r.check_collision_point_rec(mouse) {
                        modo = i;
                    }
                }
            }
            if ver_controles != antes.2 {
                sfx(&audio, Sfx::MenuSelect);
            } else if (nave_elegida, modo) != (antes.0, antes.1) {
                sfx(&audio, Sfx::MenuMove);
            }
            if let Some(a) = audio.as_mut() {
                a.set_track(Some(Track::Main));
                a.update(None, None);
            }
            if !ver_controles && rl.is_key_pressed(KeyboardKey::KEY_ENTER) {
                sfx(&audio, Sfx::Start);
                prev_outcome = None;
                en_inicio = false;
                ship = spawn_ship(&scene, nave_elegida);
                survival = (modo == 1).then(|| Survival::new(&scene.planets, &ship));
                aviso = survival.as_ref().map(|_| ("El Devorador viene por ti: busca las gemas!".to_string(), 4.0));
                ship_cam = ShipCamera::new();
                stick = (0.0, 0.0);
                mouse_locked = true;
                rl.disable_cursor();
            } else {
                let cam = start_screen_view(time);
                scene.render(&mut renderer, &cam, time, None);
                // La nave elegida se dibuja aparte (fondo transparente) y se
                // pone encima del menú, para que nada la tape.
                scene.render_ship_preview(&mut preview_r, time, nave_elegida);
                preview_tex.update_texture(&preview_r.fb.color).expect("update_texture");
                screen_tex.update_texture(&renderer.fb.color).expect("update_texture");
                let (sw, sh) = (rl.get_screen_width() as f32, rl.get_screen_height() as f32);
                let mut d = rl.begin_drawing(&thread);
                draw_framebuffer(&mut d, &screen_tex, sw, sh);
                draw_start_screen(&mut d, sw, sh, time, nave_elegida, modo, ver_controles);
                if !ver_controles {
                    draw_ship_preview(&mut d, &preview_tex, sw, sh);
                }
                continue;
            }
        }

        // --- Entrada general -------------------------------------------------
        if rl.is_key_pressed(KeyboardKey::KEY_ESCAPE) || rl.is_key_pressed(KeyboardKey::KEY_TAB) {
            mouse_locked = !mouse_locked;
            if mouse_locked { rl.disable_cursor() } else { rl.enable_cursor() }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F1) {
            show_debug = !show_debug;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_C) {
            ship_cam.toggle();
        }
        if rl.is_key_pressed(KeyboardKey::KEY_R) {
            ship = spawn_ship(&scene, nave_elegida);
            stick = (0.0, 0.0);
            // En supervivencia, R empieza la partida de nuevo.
            if survival.is_some() {
                survival = Some(Survival::new(&scene.planets, &ship));
                prev_outcome = None;
            }
            sfx(&audio, Sfx::Start);
        }
        // Volver a la pantalla de inicio: M en cualquier modo (en una partida
        // de supervivencia en curso hay que presionarla dos veces, para no
        // perderla sin querer), o ENTER cuando la partida ya terminó.
        let terminado = survival.as_ref().is_some_and(|s| s.outcome.is_some());
        let en_partida = survival.is_some() && !terminado;
        let mut volver = terminado && rl.is_key_pressed(KeyboardKey::KEY_ENTER);
        if rl.is_key_pressed(KeyboardKey::KEY_M) {
            if en_partida && time > confirmar_salida {
                confirmar_salida = time + 2.5;
                sfx(&audio, Sfx::Denied);
                aviso = Some(("Presiona M otra vez para volver al menu (se pierde la partida)".to_string(), 2.5));
            } else {
                volver = true;
            }
        }
        if volver {
            sfx(&audio, Sfx::MenuSelect);
            if let Some(a) = &audio {
                a.update(None, None);
            }
            confirmar_salida = 0.0;
            aviso = None;
            en_inicio = true;
            survival = None;
            free_cam = None;
            mouse_locked = false;
            rl.enable_cursor();
            continue;
        }
        // 1 / 2 / 3: cambiar de nave en pleno vuelo (conserva posición y
        // velocidad; solo cambian el modelo y cómo responde).
        if let Some(k) = ship_key(&rl) {
            if k != ship.kind {
                nave_elegida = k;
                ship.kind = k;
                sfx(&audio, Sfx::ShipSwitch);
                aviso = Some((format!("Nave: {}", SHIP_SPECS[k].name), 2.0));
            }
        }
        // F3 / F4: apagar culling o LOD para comparar los contadores.
        if rl.is_key_pressed(KeyboardKey::KEY_F3) {
            scene.culling_enabled = !scene.culling_enabled;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F4) {
            scene.lod_enabled = !scene.lod_enabled;
        }
        let colliders = scene.colliders(time);
        // O: órbita automática alrededor del planeta más cercano.
        if rl.is_key_pressed(KeyboardKey::KEY_O) {
            let msg = match ship.toggle_orbit(&colliders) {
                OrbitToggle::Engaged(name) => {
                    sfx(&audio, Sfx::OrbitOn);
                    format!("Orbitando {name}")
                }
                OrbitToggle::Released => {
                    sfx(&audio, Sfx::OrbitOff);
                    "Orbita liberada".to_string()
                }
                OrbitToggle::TooFar => {
                    sfx(&audio, Sfx::Denied);
                    "Acercate a un planeta para orbitar".to_string()
                }
            };
            aviso = Some((msg, 2.0));
        }
        if let Some((_, t)) = aviso.as_mut() {
            *t -= dt;
        }
        if aviso.as_ref().is_some_and(|(_, t)| *t <= 0.0) {
            aviso = None;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F2) {
            // Cámara libre de depuración: arranca donde está la nave.
            free_cam = match free_cam {
                Some(_) => None,
                None => {
                    let s = ship.render_state();
                    let f = s.forward;
                    Some(FreeFly { pos: s.pos - f * 3.0, yaw: f.x.atan2(-f.z), pitch: f.y.asin() })
                }
            };
        }

        let key = |k| rl.is_key_down(k);
        let axis = |pos: bool, neg: bool| (pos as i32 - neg as i32) as f32;
        let md = if mouse_locked { rl.get_mouse_delta() } else { Vector2::zero() };

        let ship_input = if let Some(fc) = free_cam.as_mut() {
            // --- Cámara libre (la nave queda flotando) ------------------------
            fc.yaw += md.x * MOUSE_SENS;
            fc.pitch = (fc.pitch - md.y * MOUSE_SENS).clamp(-1.5, 1.5);
            let speed = FREEFLY_SPEED * if key(KeyboardKey::KEY_LEFT_SHIFT) { 6.0 } else { 1.0 };
            let (f, r) = (fc.forward(), fc.right());
            let mv = f * axis(key(KeyboardKey::KEY_W), key(KeyboardKey::KEY_S))
                + r * axis(key(KeyboardKey::KEY_D), key(KeyboardKey::KEY_A))
                + Vec3::Y * axis(key(KeyboardKey::KEY_SPACE), key(KeyboardKey::KEY_LEFT_CONTROL));
            fc.pos += mv.normalize() * speed * dt;
            ShipInput::default()
        } else {
            // --- Controles de la nave ----------------------------------------
            // El mouse mueve una palanca virtual que vuelve sola al centro.
            // Así la cantidad de giro depende de cuánto se mueve el mouse,
            // no de cuántos cuadros hay.
            stick.0 += md.x * MOUSE_STICK_SENS;
            stick.1 += md.y * MOUSE_STICK_SENS;
            let len = (stick.0 * stick.0 + stick.1 * stick.1).sqrt();
            if len > 1.0 {
                stick = (stick.0 / len, stick.1 / len);
            }
            let decay = (-MOUSE_STICK_RECENTER * dt).exp();
            stick = (stick.0 * decay, stick.1 * decay);

            ShipInput {
                thrust: axis(key(KeyboardKey::KEY_W), key(KeyboardKey::KEY_S)),
                lift: axis(key(KeyboardKey::KEY_SPACE), key(KeyboardKey::KEY_LEFT_CONTROL)),
                boost: key(KeyboardKey::KEY_LEFT_SHIFT),
                pitch: (-stick.1 + axis(key(KeyboardKey::KEY_UP), key(KeyboardKey::KEY_DOWN))).clamp(-1.0, 1.0),
                yaw: (stick.0 + axis(key(KeyboardKey::KEY_RIGHT), key(KeyboardKey::KEY_LEFT))).clamp(-1.0, 1.0),
                roll: axis(key(KeyboardKey::KEY_D), key(KeyboardKey::KEY_A)),
            }
        };

        // --- Simulación ------------------------------------------------------
        // Al terminar la partida la nave queda flotando (sin controles).
        let ship_input = if terminado { ShipInput::default() } else { ship_input };
        ship.update(&ship_input, dt, &colliders);
        if let Some(sv) = survival.as_mut() {
            for ev in sv.update(dt, time, &mut ship, &scene.planets, &colliders) {
                // El último golpe suena como derrota (más abajo).
                if ev == Event::Hit && sv.hearts > 0 {
                    sfx(&audio, Sfx::Hit);
                }
                let msg = match ev {
                    Event::GemCollected { planet } => {
                        sfx(&audio, Sfx::Gem);
                        format!("Gema de {planet}!  {}/{}", sv.collected(), sv.total())
                    }
                    Event::Hit => match sv.hearts {
                        0 => "Te atraparon...".to_string(),
                        1 => "AUCH! Te queda 1 corazon".to_string(),
                        h => format!("AUCH! Te quedan {h} corazones"),
                    },
                };
                aviso = Some((msg, 2.5));
            }
        }
        // Sonidos que dependen de "cuándo empieza" algo.
        let outcome = survival.as_ref().and_then(|s| s.outcome);
        if outcome != prev_outcome {
            match outcome {
                Some(Outcome::Won) => sfx(&audio, Sfx::Win),
                Some(Outcome::Lost) => sfx(&audio, Sfx::Lose),
                None => {}
            }
            prev_outcome = outcome;
        }
        if ship.boosting && !prev_boost {
            sfx(&audio, Sfx::Boost);
        }
        prev_boost = ship.boosting;
        // contact_timer vuelve a 1 con cada golpe fuerte contra una superficie.
        if ship.contact_timer > prev_contact + 0.5 {
            sfx(&audio, Sfx::Contact);
        }
        prev_contact = ship.contact_timer;
        if let Some(a) = audio.as_mut() {
            // Explorador: la misma canción del menú. Supervivencia: la de jefe.
            a.set_track(Some(if survival.is_some() { Track::Boss } else { Track::Main }));
            let playing = !terminado && free_cam.is_none();
            let enemy = survival.as_ref().filter(|s| s.outcome.is_none()).map(|s| (s.enemy.pos - ship.state.pos).length());
            a.update(playing.then_some((ship.throttle, ship.boosting)), enemy);
        }
        let ship_state = ship.render_state();
        // En modo órbita la cámara se enfoca en el planeta.
        let focus = ship.orbit.and_then(|o| colliders.get(o.body)).map(|c| c.center);
        let mut camera = ship_cam.update(&ship_state, ship.speed(), dt, &colliders, focus);
        if let Some(fc) = &free_cam {
            camera = fc.camera(FOV_Y_DEG.to_radians(), NEAR, FAR);
        }

        // --- Render ----------------------------------------------------------
        let ship_view = ShipView {
            state: ship_state,
            kind: ship.kind,
            throttle: ship.throttle,
            boosting: ship.boosting,
            // Tras un golpe la nave parpadea mientras es invulnerable.
            visible: (free_cam.is_some() || ship_cam.mode == CameraMode::ThirdPerson)
                && !survival.as_ref().is_some_and(|s| s.invulnerable > 0.0 && (time * 12.0) as i32 % 2 == 0),
            light: None,
        };
        let bodies = scene.render(&mut renderer, &camera, time, Some(&ship_view));
        if let Some(sv) = &survival {
            art.draw(&mut renderer, camera.eye, time, sv, &scene.planets);
        }

        // --- Presentar -------------------------------------------------------
        screen_tex.update_texture(&renderer.fb.color).expect("update_texture");

        let (sw, sh) = (rl.get_screen_width() as f32, rl.get_screen_height() as f32);
        let fps = rl.get_fps();
        let stats = renderer.stats;
        let mut d = rl.begin_drawing(&thread);
        draw_framebuffer(&mut d, &screen_tex, sw, sh);

        // Mira y palanca virtual del mouse.
        let (cx, cy) = (sw * 0.5, sh * 0.5);
        if free_cam.is_none() {
            let r = 60.0;
            d.draw_circle_lines(cx as i32, cy as i32, r, Color::new(255, 255, 255, 50));
            d.draw_circle_v(Vector2::new(cx + stick.0 * r, cy + stick.1 * r), 4.0, Color::new(255, 220, 120, 200));
            if ship_cam.mode == CameraMode::Cockpit {
                d.draw_line(cx as i32 - 12, cy as i32, cx as i32 + 12, cy as i32, Color::new(120, 255, 220, 160));
                d.draw_line(cx as i32, cy as i32 - 12, cx as i32, cy as i32 + 12, Color::new(120, 255, 220, 160));
            }
        }

        // Panel de vuelo.
        let speed = ship.speed();
        let cam_name = if free_cam.is_some() {
            "Libre (F2)"
        } else if ship_cam.mode == CameraMode::Cockpit {
            "Cabina"
        } else {
            "Tercera persona"
        };
        // Cuerpo más cercano y altura sobre su superficie.
        let nearest = colliders
            .iter()
            .map(|c| (c, c.signed_distance(ship_state.pos)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let flight = [
            format!("Velocidad: {speed:5.1}{}", if ship.boosting { "  TURBO" } else { "" }),
            format!("Nave: {} (1/2/3)", ship.spec().name),
            format!("Camara: {cam_name}"),
            match nearest {
                Some((c, alt)) => format!("{}: altura {:.1}", c.name, (alt - SHIP_COLLISION_RADIUS).max(0.0)),
                None => String::new(),
            },
        ];
        for (i, l) in flight.iter().enumerate() {
            d.draw_text(l, sw as i32 - 300, 18 + 24 * i as i32, 20, Color::new(255, 230, 170, 230));
        }
        // Órbita automática: estado y controles, o sugerencia si hay un
        // planeta cerca.
        if let Some(o) = &ship.orbit {
            let msg = format!(
                "ORBITA AUTOMATICA: {}  -  W/S velocidad ({:.1})   Espacio/Ctrl altura   A/D inclinar   O soltar",
                o.name, o.speed
            );
            let w = d.measure_text(&msg, 20);
            d.draw_rectangle((sw as i32 - w) / 2 - 12, 14, w + 24, 34, Color::new(10, 40, 50, 170));
            d.draw_text(&msg, (sw as i32 - w) / 2, 22, 20, Color::new(140, 255, 230, 240));
        } else if free_cam.is_none() {
            if let Some((i, alt)) = ship.nearest_planet(&colliders) {
                if alt < ORBIT_ENGAGE_ALTITUDE {
                    let msg = format!("Presiona O para orbitar {}", colliders[i].name);
                    let w = d.measure_text(&msg, 22);
                    d.draw_text(&msg, (sw as i32 - w) / 2, (sh * 0.72) as i32, 22, Color::new(140, 255, 230, 220));
                }
            }
        }
        if let Some(sv) = &survival {
            draw_survival_hud(&mut d, sv, &camera, &scene.planets, ship_state.pos, time, sw, sh);
        }
        if let Some((msg, t)) = &aviso {
            let w = d.measure_text(msg, 26);
            let a = (255.0 * t.min(1.0)) as u8;
            d.draw_text(msg, (sw as i32 - w) / 2, (sh * 0.22) as i32, 26, Color::new(255, 240, 170, a));
        }
        if ship.contact_timer > 0.0 {
            let msg = format!("CONTACTO CON {}", ship.contact_with.to_uppercase());
            let w = d.measure_text(&msg, 28);
            let a = (255.0 * ship.contact_timer.min(1.0)) as u8;
            d.draw_text(&msg, (sw as i32 - w) / 2, (sh * 0.28) as i32, 28, Color::new(255, 110, 80, a));
        }

        if show_debug {
            let onoff = |b: bool| if b { "ON" } else { "OFF" };
            let mut lines = vec![
                format!("FPS: {fps}"),
                format!("Culling (F3): {}   LOD (F4): {}", onoff(scene.culling_enabled), onoff(scene.lod_enabled)),
                format!("Objetos: {} dibujados / {} descartados", stats.objects_drawn, stats.objects_culled),
                format!("Triangulos entrada: {}", stats.tris_in),
                format!("  fuera del frustum: {}", stats.tris_frustum),
                format!("  caras traseras:    {}", stats.tris_backface),
                format!("  rasterizados:      {}", stats.tris_raster),
                format!("Pixeles sombreados: {}", stats.pixels_shaded),
            ];
            for b in &bodies {
                lines.push(match b.lod {
                    Some(l) => format!(
                        "  {}: LOD {} ({} tris{}, {:.0} px)",
                        b.name,
                        l,
                        b.triangles,
                        if b.props_triangles > 0 { format!(" + {} objetos", b.props_triangles) } else { String::new() },
                        b.pixels.min(9999.0)
                    ),
                    None => format!("  {}: fuera de vista", b.name),
                });
            }
            d.draw_rectangle(8, 8, 400, 24 + 22 * lines.len() as i32, Color::new(0, 0, 0, 150));
            for (i, l) in lines.iter().enumerate() {
                d.draw_text(l, 18, 18 + 22 * i as i32, 20, Color::RAYWHITE);
            }
        }
        d.draw_text(
            "W/S acelerar/frenar | Mouse/flechas girar | A/D rodar | Espacio/Ctrl subir/bajar | Shift turbo | O orbitar | C camara | R reiniciar | M menu | N sonido | Tab mouse | F1 debug | F2 libre | F3 culling | F4 LOD",
            12,
            sh as i32 - 26,
            16,
            Color::new(200, 200, 220, 200),
        );
    }
}

/// Dibuja el framebuffer escalado a la ventana, manteniendo 16:9.
fn draw_framebuffer(d: &mut RaylibDrawHandle, tex: &Texture2D, sw: f32, sh: f32) {
    d.clear_background(Color::BLACK);
    let scale = (sw / RENDER_W as f32).min(sh / RENDER_H as f32);
    let (dw, dh) = (RENDER_W as f32 * scale, RENDER_H as f32 * scale);
    d.draw_texture_pro(
        tex,
        Rectangle::new(0.0, 0.0, RENDER_W as f32, RENDER_H as f32),
        Rectangle::new((sw - dw) * 0.5, (sh - dh) * 0.5, dw, dh),
        Vector2::zero(),
        0.0,
        Color::WHITE,
    );
}

/// Texto centrado horizontalmente.
fn centered(d: &mut RaylibDrawHandle, text: &str, y: f32, size: i32, sw: f32, color: Color) {
    let w = d.measure_text(text, size);
    d.draw_text(text, (sw as i32 - w) / 2, y as i32, size, color);
}

/// Pantalla de inicio (HUD 2D sobre el sistema solar renderizado).
fn draw_start_screen(d: &mut RaylibDrawHandle, sw: f32, sh: f32, time: f32, nave: usize, modo: usize, show_controls: bool) {
    // Velo oscuro sobre toda la pantalla para que el texto se lea.
    d.draw_rectangle(0, 0, sw as i32, sh as i32, Color::new(5, 5, 20, 160));

    // Sombra + título (se achica si no cabe en el ancho de la ventana).
    let title = "SPACE CRAFT MASTERS PRO +";
    let mut title_size = (sh * 0.10) as i32;
    while title_size > 20 && d.measure_text(title, title_size) as f32 > sw * 0.92 {
        title_size -= 2;
    }
    let w = d.measure_text(title, title_size);
    let (tx, ty) = ((sw as i32 - w) / 2, (sh * 0.16) as i32);
    d.draw_text(title, tx + 4, ty + 4, title_size, Color::new(120, 40, 10, 200));
    d.draw_text(title, tx, ty, title_size, Color::new(255, 200, 70, 255));

    let mouse = d.get_mouse_position();
    draw_ship_selector(d, sw, sh, nave, mouse);
    draw_mode_selector(d, sw, sh, modo, mouse);
    draw_controls_button(d, sw, sh, show_controls, mouse);

    // "Presiona ENTER" parpadeando suavemente.
    let a = (170.0 + 85.0 * (time * 3.0).sin()) as u8;
    centered(d, "Presiona ENTER para despegar", sh * 0.94, (sh * 0.032) as i32, sw, Color::new(255, 240, 170, a));

    if show_controls {
        draw_controls_panel(d, sw, sh);
    }
}

/// Zona de la pantalla de inicio donde se ve la nave elegida, en fracciones
/// de la pantalla: ((x0, x1), (y0, y1)).
/// Recuadro de la pantalla de inicio donde se dibuja la nave elegida (en
/// fracciones de la pantalla: x0, y0, ancho, alto). Va encima del velo.
const SHOWCASE: (f32, f32, f32, f32) = (0.395, 0.525, 0.21, 0.14);
/// Resolución de la vista previa (misma proporción que el recuadro en 16:9).
const PREVIEW_W: usize = 240;
const PREVIEW_H: usize = 90;

/// Cámara cinemática de la pantalla de inicio: rodea el sistema despacio.
fn start_screen_view(time: f32) -> camera::Camera {
    let a = time * 0.05;
    let eye = Vec3::new(a.cos() * 210.0, 70.0, a.sin() * 210.0);
    camera::Camera {
        eye,
        forward: (Vec3::new(0.0, -10.0, 0.0) - eye).normalize(),
        up: Vec3::Y,
        fov_y: FOV_Y_DEG.to_radians(),
        near: NEAR,
        far: FAR,
    }
}

/// Dibuja la vista previa de la nave (textura con transparencia) en su
/// recuadro, centrada y sin deformarla.
fn draw_ship_preview(d: &mut RaylibDrawHandle, tex: &Texture2D, sw: f32, sh: f32) {
    let (x, y, w, h) = (SHOWCASE.0 * sw, SHOWCASE.1 * sh, SHOWCASE.2 * sw, SHOWCASE.3 * sh);
    let scale = (w / PREVIEW_W as f32).min(h / PREVIEW_H as f32);
    let (dw, dh) = (PREVIEW_W as f32 * scale, PREVIEW_H as f32 * scale);
    d.draw_texture_pro(
        tex,
        Rectangle::new(0.0, 0.0, PREVIEW_W as f32, PREVIEW_H as f32),
        Rectangle::new(x + (w - dw) * 0.5, y + (h - dh) * 0.5, dw, dh),
        Vector2::zero(),
        0.0,
        Color::WHITE,
    );
}

/// Dos botones de modo de juego (el elegido resaltado) y su descripción.
fn draw_mode_selector(d: &mut RaylibDrawHandle, sw: f32, sh: f32, modo: usize, mouse: Vector2) {
    let size = (sh * 0.034) as i32;
    let rects = MenuRects::new(sw, sh);
    for (i, (name, _)) in MODES.iter().enumerate() {
        let r = rects.modes[i];
        let sel = i == modo;
        let hover = r.check_collision_point_rec(mouse);
        // Supervivencia en rojo, explorador en turquesa.
        let tint = if i == 1 { Color::new(255, 90, 80, 255) } else { Color::new(120, 230, 255, 255) };
        if sel {
            d.draw_rectangle_rec(r, tint.alpha(0.85));
        } else if hover {
            d.draw_rectangle_rec(r, tint.alpha(0.2));
        }
        d.draw_rectangle_lines_ex(r, 2.0, tint.alpha(if sel || hover { 1.0 } else { 0.5 }));
        let w = d.measure_text(name, size);
        let col = if sel { Color::new(15, 10, 30, 255) } else { tint.alpha(0.8) };
        d.draw_text(name, (r.x + (r.width - w as f32) * 0.5) as i32, (r.y + (r.height - size as f32) * 0.5) as i32, size, col);
    }
    centered(d, MODES[modo].1, sh * 0.845, (sh * 0.022) as i32, sw, Color::new(220, 220, 240, 220));
}

/// Zonas clicables de la pantalla de inicio (las usan el dibujo y el mouse).
struct MenuRects {
    prev_ship: Rectangle,
    next_ship: Rectangle,
    modes: [Rectangle; 2],
    controls: Rectangle,
}

impl MenuRects {
    fn new(sw: f32, sh: f32) -> Self {
        let arrow = sh * 0.06;
        let ay = sh * 0.437;
        let (bw, bh, gap) = (sw * 0.22, sh * 0.055, sw * 0.03);
        let (my, x0) = (sh * 0.78, sw * 0.5 - bw - gap * 0.5);
        let cw = sw * 0.16;
        Self {
            prev_ship: Rectangle::new(sw * 0.5 - sw * 0.2 - arrow * 0.5, ay, arrow, arrow),
            next_ship: Rectangle::new(sw * 0.5 + sw * 0.2 - arrow * 0.5, ay, arrow, arrow),
            modes: [Rectangle::new(x0, my, bw, bh), Rectangle::new(x0 + bw + gap, my, bw, bh)],
            controls: Rectangle::new(sw * 0.5 - cw * 0.5, sh * 0.873, cw, sh * 0.042),
        }
    }
}

/// Botón "CONTROLES" debajo de los modos.
fn draw_controls_button(d: &mut RaylibDrawHandle, sw: f32, sh: f32, open: bool, mouse: Vector2) {
    let r = MenuRects::new(sw, sh).controls;
    let hover = r.check_collision_point_rec(mouse);
    let c = Color::new(255, 220, 140, 255);
    if open || hover {
        d.draw_rectangle_rec(r, c.alpha(0.2));
    }
    d.draw_rectangle_lines_ex(r, 2.0, c.alpha(if hover { 1.0 } else { 0.6 }));
    let size = (sh * 0.024) as i32;
    let t = "CONTROLES  (C)";
    let w = d.measure_text(t, size);
    d.draw_text(t, (r.x + (r.width - w as f32) * 0.5) as i32, (r.y + (r.height - size as f32) * 0.5) as i32, size, c);
}

/// Ventana con todos los controles (se abre con el botón o con C).
fn draw_controls_panel(d: &mut RaylibDrawHandle, sw: f32, sh: f32) {
    d.draw_rectangle(0, 0, sw as i32, sh as i32, Color::new(0, 0, 10, 140));
    let (pw, ph) = (sw * 0.62, sh * 0.70);
    let panel = Rectangle::new((sw - pw) * 0.5, (sh - ph) * 0.5, pw, ph);
    d.draw_rectangle_rec(panel, Color::new(12, 12, 35, 240));
    d.draw_rectangle_lines_ex(panel, 3.0, Color::new(255, 200, 70, 230));
    let title = (sh * 0.05) as i32;
    centered(d, "CONTROLES", panel.y + sh * 0.03, title, sw, Color::new(255, 200, 70, 255));

    let controls = [
        ("W / S", "acelerar / frenar"),
        ("Mouse o flechas", "girar la nave"),
        ("A / D", "rodar"),
        ("Espacio / Ctrl", "subir / bajar"),
        ("Shift", "turbo"),
        ("O", "orbitar el planeta cercano"),
        ("C", "camara: tercera persona / cabina"),
        ("1 / 2 / 3", "cambiar de nave"),
        ("R", "reiniciar la nave (o la partida)"),
        ("M", "volver a la pantalla de inicio"),
        ("Tab / Esc", "soltar o atrapar el mouse"),
        ("N", "sonido: apagar / encender"),
        ("F1 / F3 / F4", "contadores / culling / LOD"),
    ];
    let size = (sh * 0.028) as i32;
    let (col_key, col_desc) = (sw * 0.5 - sh * 0.02, sw * 0.5 + sh * 0.02);
    for (i, (k, desc)) in controls.iter().enumerate() {
        let y = (panel.y + sh * 0.11 + i as f32 * sh * 0.039) as i32;
        let kw = d.measure_text(k, size);
        d.draw_text(k, col_key as i32 - kw, y, size, Color::new(140, 255, 230, 240));
        d.draw_text(desc, col_desc as i32, y, size, Color::new(235, 235, 245, 230));
    }
    centered(
        d,
        "En este menu: flechas izq/der nave, arriba/abajo modo, ENTER despegar",
        panel.y + ph - sh * 0.085,
        (sh * 0.022) as i32,
        sw,
        Color::new(200, 200, 230, 220),
    );
    centered(d, "C, Esc o clic para cerrar", panel.y + ph - sh * 0.05, (sh * 0.024) as i32, sw, Color::new(255, 220, 140, 230));
}

/// Pantalla 2D de la partida de supervivencia: corazones, gemas, aviso del
/// enemigo, marcadores sobre las gemas y el resultado al terminar.
#[allow(clippy::too_many_arguments)]
fn draw_survival_hud(
    d: &mut RaylibDrawHandle,
    sv: &Survival,
    cam: &camera::Camera,
    planets: &[planet::Planet],
    ship_pos: Vec3,
    time: f32,
    sw: f32,
    sh: f32,
) {
    // --- Corazones -----------------------------------------------------------
    let hs = (sh * 0.045).max(22.0);
    let hurt = sv.invulnerable > 0.0;
    for i in 0..SURVIVAL_HEARTS {
        let x = 22.0 + i as f32 * hs * 1.25;
        let full = i < sv.hearts;
        let c = if full {
            if hurt && (time * 10.0) as i32 % 2 == 0 { Color::WHITE } else { Color::new(255, 60, 80, 255) }
        } else {
            Color::new(70, 60, 80, 200)
        };
        draw_heart(d, x, 20.0, hs, c);
    }
    // --- Gemas: contador debajo de los corazones ------------------------------
    let gy = 20.0 + hs * 1.15;
    let gs = hs * 0.42;
    let (gx, gcy) = (22.0 + gs, gy + gs);
    d.draw_poly(Vector2::new(gx, gcy), 4, gs, 0.0, Color::new(120, 255, 200, 255));
    d.draw_poly_lines(Vector2::new(gx, gcy), 4, gs, 0.0, Color::WHITE);
    let text = format!("Gemas {}/{}", sv.collected(), sv.total());
    let size = (hs * 0.7) as i32;
    d.draw_text(&text, (gx + gs * 1.8) as i32, (gcy - size as f32 * 0.5) as i32, size, Color::new(235, 255, 245, 240));
    // Enemigo: distancia (en rojo si está cerca) y tiempo.
    let dist = (sv.enemy.pos - ship_pos).length();
    let size2 = (hs * 0.5) as i32;
    let ec = if dist < 25.0 { Color::new(255, 80, 70, 255) } else { Color::new(230, 200, 220, 220) };
    let t = sv.elapsed as u32;
    d.draw_text(&format!("Devorador a {dist:.0} m   -   Tiempo {}:{:02}", t / 60, t % 60), 22, (gcy + gs * 1.6) as i32, size2, ec);

    // --- Marcadores en pantalla -----------------------------------------------
    // Cada gema que falta: rombo de su color con la distancia.
    for g in sv.gems.iter().filter(|g| g.collected_at.is_none()) {
        let p = g.position(planets, time);
        if let Some((x, y)) = project(cam, p, sw, sh) {
            let c = Color::new((g.color.x * 255.0) as u8, (g.color.y * 255.0) as u8, (g.color.z * 255.0) as u8, 230);
            d.draw_poly_lines_ex(Vector2::new(x, y), 4, 11.0, 0.0, 2.0, c);
            let txt = format!("{:.0}", (p - ship_pos).length());
            let w = d.measure_text(&txt, 16);
            d.draw_text(&txt, x as i32 - w / 2, y as i32 + 14, 16, c);
        }
    }
    // Enemigo: marca roja si está a la vista; si no, una flecha en el borde.
    match project(cam, sv.enemy.pos, sw, sh) {
        Some((x, y)) => {
            d.draw_circle_lines(x as i32, y as i32, 16.0, Color::new(255, 60, 60, 220));
            d.draw_text("!", x as i32 - 3, y as i32 - 32, 22, Color::new(255, 60, 60, 240));
        }
        None if dist < 70.0 => {
            // Dirección del enemigo en el plano de la pantalla.
            let to = (sv.enemy.pos - cam.eye).normalize();
            let right = cam.forward.cross(cam.up).normalize();
            let up = right.cross(cam.forward);
            let (dx, dy) = (to.dot(right), -to.dot(up));
            let l = (dx * dx + dy * dy).sqrt().max(1e-3);
            let (cx, cy) = (sw * 0.5 + dx / l * sh * 0.38, sh * 0.5 + dy / l * sh * 0.38);
            let a = (dy / l).atan2(dx / l).to_degrees();
            d.draw_poly(Vector2::new(cx, cy), 3, 18.0, a, Color::new(255, 60, 60, 230));
        }
        None => {}
    }
    // Borde rojo cuando el enemigo está muy cerca.
    if dist < 25.0 && sv.outcome.is_none() {
        let k = 1.0 - dist / 25.0;
        let a = (k * (150.0 + 60.0 * (time * 8.0).sin())) as u8;
        d.draw_rectangle_lines_ex(Rectangle::new(0.0, 0.0, sw, sh), 6.0 + 18.0 * k, Color::new(255, 30, 40, a));
    }

    // --- Fin de la partida ------------------------------------------------------
    if let Some(o) = sv.outcome {
        d.draw_rectangle(0, 0, sw as i32, sh as i32, Color::new(5, 5, 20, 170));
        let (title, color) = match o {
            Outcome::Won => ("GANASTE!", Color::new(255, 215, 80, 255)),
            Outcome::Lost => ("TE ATRAPO EL DEVORADOR", Color::new(255, 80, 80, 255)),
        };
        let mut ts = (sh * 0.1) as i32;
        while ts > 20 && d.measure_text(title, ts) as f32 > sw * 0.9 {
            ts -= 2;
        }
        centered(d, title, sh * 0.32, ts, sw, color);
        let t = sv.elapsed as u32;
        let sub = format!("Gemas {}/{}   -   Tiempo {}:{:02}", sv.collected(), sv.total(), t / 60, t % 60);
        centered(d, &sub, sh * 0.47, (sh * 0.04) as i32, sw, Color::new(235, 235, 250, 240));
        let a = (170.0 + 85.0 * (time * 3.0).sin()) as u8;
        centered(d, "ENTER: volver al menu    R: jugar otra vez", sh * 0.6, (sh * 0.035) as i32, sw, Color::new(255, 240, 170, a));
    }
}

/// Corazón: dos círculos y un triángulo. (x, y) es la esquina superior izquierda.
fn draw_heart(d: &mut RaylibDrawHandle, x: f32, y: f32, size: f32, c: Color) {
    let r = size * 0.28;
    d.draw_circle_v(Vector2::new(x + r, y + r), r, c);
    d.draw_circle_v(Vector2::new(x + size - r, y + r), r, c);
    d.draw_triangle(
        Vector2::new(x + size * 0.02, y + r * 1.25),
        Vector2::new(x + size * 0.5, y + size * 0.95),
        Vector2::new(x + size * 0.98, y + r * 1.25),
        c,
    );
}

/// Proyecta un punto del mundo a pixeles de la ventana (teniendo en cuenta
/// el framebuffer escalado y centrado). None si queda detrás o fuera.
fn project(cam: &camera::Camera, p: Vec3, sw: f32, sh: f32) -> Option<(f32, f32)> {
    let clip = (cam.projection(RENDER_W as f32 / RENDER_H as f32) * cam.view()).mul_vec4(math::Vec4::from_point(p));
    if clip.w <= 0.0 {
        return None;
    }
    let (nx, ny) = (clip.x / clip.w, clip.y / clip.w);
    if nx.abs() > 1.0 || ny.abs() > 1.0 {
        return None;
    }
    let scale = (sw / RENDER_W as f32).min(sh / RENDER_H as f32);
    let (dw, dh) = (RENDER_W as f32 * scale, RENDER_H as f32 * scale);
    Some(((sw - dw) * 0.5 + (nx * 0.5 + 0.5) * dw, (sh - dh) * 0.5 + (0.5 - ny * 0.5) * dh))
}

/// Selector de nave de la pantalla de inicio (el modelo 3D girando lo dibuja
/// la escena; aquí van el nombre, la descripción y las barras).
fn draw_ship_selector(d: &mut RaylibDrawHandle, sw: f32, sh: f32, nave: usize, mouse: Vector2) {
    let spec = &SHIP_SPECS[nave];
    let cx = sw * 0.5;
    let text_c = |d: &mut RaylibDrawHandle, t: &str, y: f32, size: i32, color: Color| {
        let w = d.measure_text(t, size);
        d.draw_text(t, (cx as i32) - w / 2, y as i32, size, color);
    };
    text_c(d, "ELIGE TU NAVE", sh * 0.405, (sh * 0.026) as i32, Color::new(190, 190, 220, 220));
    text_c(d, &spec.name.to_uppercase(), sh * 0.44, (sh * 0.05) as i32, Color::new(255, 200, 70, 255));
    // Flechas clicables a los lados del nombre.
    let rects = MenuRects::new(sw, sh);
    for (r, t) in [(rects.prev_ship, "<"), (rects.next_ship, ">")] {
        let hover = r.check_collision_point_rec(mouse);
        let c = Color::new(255, 200, 70, if hover { 255 } else { 170 });
        if hover {
            d.draw_rectangle_rec(r, c.alpha(0.2));
        }
        let size = (sh * 0.05) as i32;
        let w = d.measure_text(t, size);
        d.draw_text(t, (r.x + (r.width - w as f32) * 0.5) as i32, (r.y + (r.height - size as f32) * 0.5) as i32, size, c);
    }
    text_c(d, spec.tagline, sh * 0.50, (sh * 0.022) as i32, Color::new(235, 235, 245, 230));

    // Barras de velocidad y control (1 a 5), debajo de la nave.
    let size = (sh * 0.024) as i32;
    let cell = sh * 0.032;
    for (row, (label, val, color)) in [
        ("Velocidad", spec.stats.0, Color::new(255, 150, 80, 240)),
        ("Control", spec.stats.1, Color::new(120, 230, 255, 240)),
    ]
    .into_iter()
    .enumerate()
    {
        let y = sh * (0.675 + 0.035 * row as f32);
        let lw = d.measure_text(label, size);
        let x0 = cx - cell * 1.5;
        d.draw_text(label, (x0 - cell * 0.4) as i32 - lw, y as i32, size, Color::new(235, 235, 245, 230));
        for i in 0..5u8 {
            let r = Rectangle::new(x0 + i as f32 * cell, y, cell * 0.8, size as f32);
            if i < val {
                d.draw_rectangle_rec(r, color);
            } else {
                d.draw_rectangle_lines_ex(r, 1.5, Color::new(200, 200, 220, 120));
            }
        }
    }
    // Puntos: cuál de las naves está elegida.
    for i in 0..SHIP_SPECS.len() {
        let x = cx + (i as f32 - (SHIP_SPECS.len() - 1) as f32 * 0.5) * sh * 0.03;
        let c = if i == nave { Color::new(255, 200, 70, 255) } else { Color::new(200, 200, 220, 110) };
        d.draw_circle(x as i32, (sh * 0.758) as i32, sh * 0.007, c);
    }
}

/// Renderiza un cuadro sin abrir ventana y lo guarda como PPM. Sirve para
/// capturas del README y para probar el pipeline sin interfaz.
///
/// Uso: `cargo run --release -- --snapshot <segundos> <archivo.ppm> [lejos|cerca|rasante|cabina|nave|choque|inicio|supervivencia] [--sin-culling] [--sin-lod] [--planeta N] [--nave N] [--bench]`
fn snapshot(args: &[String]) {
    let time: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10.0);
    let out = args.get(3).cloned().unwrap_or_else(|| "snapshot.ppm".into());
    let mode = args.get(4).map(String::as_str).unwrap_or("lejos");
    let mut scene = Scene::new().expect("escena");
    // "inicio": fondo 3D de la pantalla de inicio con la nave de --nave N
    // (sin el texto, que lo dibuja raylib).
    if mode == "inicio" {
        let kind = args.iter().position(|a| a == "--nave").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(0);
        // Solo la vista previa de la nave, sobre gris para ver la transparencia.
        let mut renderer = Renderer::new(PREVIEW_W, PREVIEW_H);
        scene.render_ship_preview(&mut renderer, time, kind);
        let mut ppm = format!("P6\n{} {}\n255\n", PREVIEW_W, PREVIEW_H).into_bytes();
        for px in renderer.fb.color.chunks_exact(4) {
            let a = px[3] as u32;
            for c in &px[..3] {
                ppm.push(((*c as u32 * a + 60 * (255 - a)) / 255) as u8);
            }
        }
        std::fs::write(&out, ppm).expect("no se pudo escribir la captura");
        return;
    }
    // "supervivencia": vista de cerca de una gema (y otra recogiéndose a
    // medio efecto) con el Devorador al lado, para revisar los modelos.
    if mode == "supervivencia" {
        let art = SurvivalArt::new().expect("supervivencia");
        let pi: usize = args.iter().position(|a| a == "--planeta").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(0);
        let ship = Ship::with_kind(0, Vec3::ZERO, -Vec3::Z);
        let mut sv = Survival::new(&scene.planets, &ship);
        let gem = sv.gems[pi].position(&scene.planets, time);
        let planet = scene.planets[pi].position(time);
        let out_dir = (gem - planet).normalize();
        let side = out_dir.cross(Vec3::Y).normalize();
        let eye = gem + out_dir * 7.0 + side * 1.5 + Vec3::Y * 1.0;
        // Otra gema recogiéndose: se dibuja en su posición de ese instante.
        let other = (pi + 1) % sv.gems.len();
        sv.gems[other].collected_at = Some(time - 0.35);
        sv.enemy.pos = gem + side * 3.2 + out_dir * 2.0 + Vec3::Y * 0.8;
        sv.enemy.forward = (eye - sv.enemy.pos).normalize();
        let r = sv.enemy.forward.cross(Vec3::Y).normalize();
        sv.enemy.up = r.cross(sv.enemy.forward);
        sv.enemy.stun = 0.0;
        let look = (gem + sv.enemy.pos) * 0.5;
        let cam = camera::Camera { eye, forward: (look - eye).normalize(), up: Vec3::Y, fov_y: FOV_Y_DEG.to_radians(), near: NEAR, far: FAR };
        let mut renderer = Renderer::new(RENDER_W, RENDER_H);
        scene.render(&mut renderer, &cam, time, None);
        art.draw(&mut renderer, cam.eye, time, &sv, &scene.planets);
        // Efecto: vista aparte mirando a la gema que se está recogiendo.
        let burst = sv.gems[other].position(&scene.planets, time - 0.35);
        let beye = burst + Vec3::new(0.0, 2.0, 9.0);
        let bcam = camera::Camera { eye: beye, forward: (burst - beye).normalize(), up: Vec3::Y, ..cam };
        let mut r2 = Renderer::new(RENDER_W, RENDER_H);
        scene.render(&mut r2, &bcam, time, None);
        art.draw(&mut r2, bcam.eye, time, &sv, &scene.planets);
        for (rr, path) in [(&renderer, out.clone()), (&r2, out.replace(".ppm", "_efecto.ppm"))] {
            let mut ppm = format!("P6\n{} {}\n255\n", RENDER_W, RENDER_H).into_bytes();
            for px in rr.fb.color.chunks_exact(4) {
                ppm.extend_from_slice(&px[..3]);
            }
            std::fs::write(&path, ppm).expect("no se pudo escribir la captura");
        }
        println!("enemigo {} tris; {} gemas; radio de la gema {:.1} sobre el planeta (radio {:.1})",
            art.enemy_triangles(), sv.total(), (gem - planet).length(), scene.planets[pi].def.radius);
        return;
    }
    // Para comparar el trabajo del renderer: --sin-culling / --sin-lod.
    scene.culling_enabled = !args.iter().any(|a| a == "--sin-culling");
    scene.lod_enabled = !args.iter().any(|a| a == "--sin-lod");
    let mut renderer = Renderer::new(RENDER_W, RENDER_H);

    // Se simula la nave volando unos segundos hacia el objetivo elegido.
    // --planeta N elige a qué planeta acercarse (0 = el primero).
    let pi: usize = args
        .iter()
        .position(|a| a == "--planeta")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
        .min(scene.planets.len() - 1);
    let p0 = &scene.planets[pi];
    let planet = p0.position(time);
    let pr = p0.def.radius;
    let (start, target) = match mode {
        "cerca" | "cabina" => {
            let side = Vec3::new(0.3, 0.25, 1.0).normalize();
            (planet + side * (pr + 4.0), planet + Vec3::new(-6.0, 0.0, 0.0))
        }
        "nave" => (Vec3::new(0.0, 4.0, 80.0), Vec3::new(-20.0, 0.0, 0.0)),
        // Pegado al terreno, mirando al horizonte: se ve el relieve.
        "rasante" => {
            let up = Vec3::new(0.35, 0.6, 1.0).normalize();
            let tangent = up.cross(Vec3::Y).normalize();
            (planet + up * (pr * 1.12 + 1.0), planet + up * (pr * 1.12) + tangent * 20.0)
        }
        // Órbita automática: la nave se engancha y lo rodea unos segundos.
        "orbita" => {
            let side = Vec3::new(0.3, 0.4, 1.0).normalize();
            let tangent = side.cross(Vec3::Y).normalize();
            (planet + side * (pr + 5.0), planet + side * (pr + 5.0) + tangent * 10.0)
        }
        // Directo contra el planeta, con turbo: debe quedar rozando la superficie.
        "choque" => {
            let side = Vec3::new(0.2, 0.5, 1.0).normalize();
            (planet + side * (pr + 12.0), planet)
        }
        _ => (Vec3::new(0.0, 10.0, p0.def.orbit.radius + 25.0), Vec3::ZERO),
    };
    // --nave N: qué nave usar (0 cohete, 1 ligera, 2 pesada).
    let kind: usize = args
        .iter()
        .position(|a| a == "--nave")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut ship = Ship::with_kind(kind, start, target - start);
    let input = if mode == "orbita" {
        ShipInput::default()
    } else {
        ShipInput { thrust: 1.0, boost: mode == "choque", ..Default::default() }
    };
    let colliders = scene.colliders(time);
    if mode == "orbita" {
        ship.toggle_orbit(&colliders);
    }
    let mut cam = ShipCamera::new();
    if mode == "cabina" {
        cam.toggle();
    }
    let frames = match mode {
        "choque" => 150,
        "orbita" => 240,
        _ => 30,
    };
    let focus = |s: &Ship| s.orbit.and_then(|o| colliders.get(o.body)).map(|c| c.center);
    let mut camera = cam.update(&ship.render_state(), 0.0, 1.0 / 60.0, &colliders, focus(&ship));
    for _ in 0..frames {
        ship.update(&input, 1.0 / 60.0, &colliders);
        camera = cam.update(&ship.render_state(), ship.speed(), 1.0 / 60.0, &colliders, focus(&ship));
    }
    let state = ship.render_state();
    if mode == "nave" {
        // Vista lateral de cerca para ver el modelo.
        let eye = state.pos + state.right() * 1.6 + state.up * 0.5 + state.forward * 0.6;
        camera.eye = eye;
        camera.forward = (state.pos - eye).normalize();
        camera.up = state.up;
    }
    let view = ShipView {
        state,
        kind: ship.kind,
        throttle: ship.throttle.max(0.6),
        boosting: false,
        visible: mode != "cabina",
        light: None,
    };

    let t0 = std::time::Instant::now();
    let bodies = scene.render(&mut renderer, &camera, time, Some(&view));
    let ms = t0.elapsed().as_secs_f32() * 1000.0;

    // --bench: repite el cuadro 60 veces y mide promedio y peor caso.
    if args.iter().any(|a| a == "--bench") {
        let mut times = Vec::with_capacity(60);
        for i in 0..60 {
            let t = std::time::Instant::now();
            scene.render(&mut renderer, &camera, time + i as f32 / 60.0, Some(&view));
            times.push(t.elapsed().as_secs_f32() * 1000.0);
        }
        let avg = times.iter().sum::<f32>() / times.len() as f32;
        let max = times.iter().copied().fold(0.0, f32::max);
        println!("BENCH {mode}: promedio {avg:.2} ms ({:.0} FPS), peor {max:.2} ms", 1000.0 / avg);
    }

    let mut ppm = format!("P6\n{} {}\n255\n", RENDER_W, RENDER_H).into_bytes();
    for px in renderer.fb.color.chunks_exact(4) {
        ppm.extend_from_slice(&px[..3]);
    }
    std::fs::write(&out, ppm).expect("no se pudo escribir la captura");
    let alt = colliders.iter().map(|c| c.signed_distance(state.pos)).fold(f32::INFINITY, f32::min);
    for b in &bodies {
        println!("  {}: lod {:?}, {} tris + {} objetos, {:.0} px", b.name, b.lod, b.triangles, b.props_triangles, b.pixels);
    }
    println!(
        "{out}: {ms:.1} ms, nave {} tris, distancia a superficie {alt:.2}, contacto '{}', stats = {:?}",
        scene.ship_triangles(),
        ship.contact_with,
        renderer.stats
    );
}
