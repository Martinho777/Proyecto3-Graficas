//! La escena: qué objetos hay, dónde están en cada instante y cómo se dibujan.
//!
//! Antes de mandar un cuerpo al pipeline se hacen dos decisiones que cambian
//! el trabajo del renderizador (y se ven en los contadores del HUD):
//!  - Culling de objetos: si su esfera envolvente está fuera del frustum, no
//!    se procesa ni un solo vértice.
//!  - Nivel de detalle: según cuántos pixeles ocupa en pantalla, se elige una
//!    malla con más o menos triángulos.

use std::path::{Path, PathBuf};

use crate::camera::Camera;
use crate::collision::{Collider, Surface};
use crate::config::*;
use crate::math::{Mat4, Vec3};
use crate::mesh::{self, Mesh};
use crate::noise::Perlin;
use crate::planet::{Planet, Terrain, build_terrain_mesh};
use crate::planets;
use crate::renderer::{DrawOptions, Renderer};
use crate::shader::{Shader, Uniforms};
use crate::shaders::{GhostDraw, GhostShader, SunShader, ThrusterDraw, ThrusterParams, ThrusterShader, ToonShader};
use crate::ship::{self, ShipState};
use crate::starfield::Starfield;

/// Lo que la escena necesita saber de la nave para dibujarla.
#[derive(Clone, Copy)]
pub struct ShipView {
    pub state: ShipState,
    /// Índice en `SHIP_SPECS`.
    pub kind: usize,
    pub throttle: f32,
    pub boosting: bool,
    /// En vista de cabina no se dibuja el casco (estaríamos dentro).
    pub visible: bool,
    /// Luz propia para la nave (vitrina del menú); None = la ilumina el sol.
    pub light: Option<Vec3>,
}

/// Qué pasó con cada cuerpo en el último cuadro (para el HUD).
#[derive(Clone, Copy, Debug)]
pub struct BodyReport {
    pub name: &'static str,
    /// None = descartado por estar fuera de la vista.
    pub lod: Option<usize>,
    pub triangles: usize,
    /// Triángulos de objetos de superficie dibujados (0 si no tocaba).
    pub props_triangles: usize,
    pub pixels: f32,
}

pub struct Scene {
    stars: Starfield,
    sun_lods: Vec<Mesh>,
    pub planets: Vec<Planet>,
    /// Un modelo por tipo de nave (mismo orden que `SHIP_SPECS`).
    ship_meshes: Vec<Mesh>,
    flame_mesh: Mesh,
    sun_shader: SunShader,
    toon: ToonShader,
    thruster: ThrusterShader,
    ghost: GhostShader,
    /// Interruptores para comparar en vivo (F3 / F4).
    pub culling_enabled: bool,
    pub lod_enabled: bool,
}

/// Busca un recurso relativo a la carpeta actual o a la del proyecto.
pub fn asset_path(rel: &str) -> PathBuf {
    let local = Path::new(rel);
    if local.exists() {
        return local.to_path_buf();
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// Elige el nivel de detalle según el tamaño en pantalla (radio en pixeles).
pub fn select_lod(radius_px: f32, levels: usize) -> usize {
    let lod = LOD_PIXEL_THRESHOLDS.iter().filter(|&&t| radius_px >= t).count();
    lod.min(levels - 1)
}

impl Scene {
    pub fn new() -> Result<Self, String> {
        let sun_noise = Perlin::new(WORLD_SEED);
        Ok(Self {
            stars: Starfield::new(STAR_COUNT, WORLD_SEED),
            sun_lods: LOD_SUBDIVISIONS
                .iter()
                .map(|&s| build_terrain_mesh(&Terrain::FLAT, &sun_noise, s.min(4)))
                .collect(),
            planets: planets::solar_system()?,
            ship_meshes: SHIP_SPECS.iter().map(|s| mesh::load_obj(asset_path(s.model))).collect::<Result<_, _>>()?,
            flame_mesh: mesh::cone(12, 4),
            sun_shader: SunShader { noise: sun_noise, speed: SUN_ANIM_SPEED, bands: 6.0 },
            toon: ToonShader { ambient: 0.36, outline: 0.35 },
            thruster: ThrusterShader { noise: Perlin::new(WORLD_SEED + 7) },
            ghost: GhostShader { glow: 1.6 },
            culling_enabled: true,
            lod_enabled: true,
        })
    }

    pub fn sun_pos(&self) -> Vec3 {
        Vec3::ZERO
    }

    /// Cuerpos sólidos en el instante `time`.
    pub fn colliders(&self, time: f32) -> Vec<Collider<'_>> {
        let mut out = vec![Collider::sphere(self.sun_pos(), Vec3::ZERO, SUN_RADIUS, "Sol")];
        for p in &self.planets {
            out.push(Collider {
                center: p.position(time),
                velocity: p.velocity(time),
                surface: Surface::Terrain { planet: p, inv_rotation: p.rotation(time).transpose() },
                name: p.def.name,
            });
        }
        out
    }

    pub fn ship_triangles(&self) -> usize {
        self.ship_meshes[0].triangle_count()
    }

    /// Culling + LOD + draw de un cuerpo esférico.
    #[allow(clippy::too_many_arguments)]
    fn draw_body(
        &self,
        r: &mut Renderer,
        name: &'static str,
        lods: &[Mesh],
        model: &Mat4,
        center: Vec3,
        bounding_radius: f32,
        shader: &dyn Shader,
        base: &Uniforms,
    ) -> BodyReport {
        let pixels = r.projected_radius_px(center, bounding_radius);
        if self.culling_enabled && !r.sphere_visible(center, bounding_radius) {
            r.stats.objects_culled += 1;
            return BodyReport { name, lod: None, triangles: 0, props_triangles: 0, pixels };
        }
        let lod = if self.lod_enabled { select_lod(pixels, lods.len()) } else { lods.len() - 1 };
        let mesh = &lods[lod];
        let u = Uniforms { model: *model, object_center: center, ..*base };
        r.draw_mesh(mesh, model, shader, &u, DrawOptions::default());
        BodyReport { name, lod: Some(lod), triangles: mesh.triangle_count(), props_triangles: 0, pixels }
    }

    pub fn render(&self, r: &mut Renderer, camera: &Camera, time: f32, ship: Option<&ShipView>) -> Vec<BodyReport> {
        let view = camera.view();
        let proj = camera.projection(RENDER_W as f32 / RENDER_H as f32);
        r.begin_frame(view, proj, Vec3::new(0.01, 0.01, 0.035));
        self.stars.draw(&mut r.fb, &view, &proj, time);

        let sun_pos = self.sun_pos();
        let base = Uniforms {
            time,
            camera_pos: camera.eye,
            sun_pos,
            model: Mat4::IDENTITY,
            object_center: Vec3::ZERO,
        };
        let mut report = Vec::with_capacity(self.planets.len() + 1);

        // --- Sol: rota despacio sobre su eje --------------------------------
        let sun_model = Mat4::translation(sun_pos) * Mat4::rotation_y(time * 0.05) * Mat4::uniform_scale(SUN_RADIUS);
        report.push(self.draw_body(r, "Sol", &self.sun_lods, &sun_model, sun_pos, SUN_RADIUS, &self.sun_shader, &base));

        // --- Planetas -------------------------------------------------------
        let mut ghosts = Vec::new();
        for p in &self.planets {
            let model = p.model(time);
            let center = p.position(time);
            let mut rep = self.draw_body(r, p.def.name, &p.lods, &model, center, p.bounding_radius, p.shader.as_ref(), &base);
            // Objetos de superficie: solo si el planeta es visible y lo
            // bastante grande en pantalla (con LOD apagado, siempre).
            let near = !self.lod_enabled || rep.pixels >= PROPS_MIN_PX;
            if rep.lod.is_some() && near {
                if let Some(props) = &p.props {
                    let u = Uniforms { model, object_center: center, ..base };
                    let opts = DrawOptions { double_sided: true, ..DrawOptions::default() };
                    r.draw_mesh(&props.mesh, &model, &self.toon, &u, opts);
                    rep.props_triangles += props.mesh.triangle_count();
                }
                if let Some(f) = &p.floaters {
                    ghosts.push((p, f, center));
                }
            }
            report.push(rep);
        }

        // --- Fantasmas (aditivos: después de lo opaco) -----------------------
        for (p, floaters, center) in ghosts {
            let rot = p.rotation(time);
            for g in &floaters.items {
                let vis = g.visibility(time);
                if vis <= 0.01 {
                    continue;
                }
                let up = rot.transform_dir(g.dir(time));
                let pos = center + up * (p.def.radius + g.height(time));
                // Siempre de frente a la cámara (el modelo mira hacia -Z).
                let to_cam = camera.eye - pos;
                let facing = (to_cam - up * to_cam.dot(up)).normalize();
                let z = -facing;
                let right = up.cross(z).normalize();
                let model = Mat4::translation(pos) * Mat4::from_basis(right, up, right.cross(up)) * Mat4::uniform_scale(g.scale);
                let u = Uniforms { model, object_center: pos, ..base };
                let draw = GhostDraw { shader: &self.ghost, visibility: vis };
                let opts = DrawOptions { double_sided: false, depth_write: false, additive: true };
                r.draw_mesh(&floaters.mesh, &model, &draw, &u, opts);
            }
        }

        // --- Nave -----------------------------------------------------------
        if let Some(sv) = ship.filter(|s| s.visible) {
            self.draw_ship(r, &base, sv);
        }
        report
    }

    /// Dibuja la nave (casco + flamas). `base` trae cámara, tiempo y sol.
    fn draw_ship(&self, r: &mut Renderer, base: &Uniforms, sv: &ShipView) {
        let base = *base;
        let kind = sv.kind.min(SHIP_SPECS.len() - 1);
        let spec = &SHIP_SPECS[kind];
        let model = ship::model_matrix(&sv.state, spec.scale);
        let u = Uniforms { model, object_center: sv.state.pos, sun_pos: sv.light.unwrap_or(base.sun_pos), ..base };
        // Dos caras: se ve el interior de la tobera.
        let opts = DrawOptions { double_sided: true, ..DrawOptions::default() };
        r.draw_mesh(&self.ship_meshes[kind], &model, &self.toon, &u, opts);

        // Flamas (una por tobera): al final porque son aditivas y no
        // escriben profundidad.
        if sv.throttle > 0.02 {
            let len = (0.25 + sv.throttle * 1.6) * spec.flame_len;
            for &(offset, w) in spec.flames {
                // `offset` está en coordenadas del modelo (detrás de la tobera).
                let width = w + 0.05 * sv.throttle;
                let flame_model = model * Mat4::translation(offset) * Mat4::scale(Vec3::new(width, width, len));
                let u = Uniforms { model: flame_model, ..u };
                let draw = ThrusterDraw {
                    shader: &self.thruster,
                    params: ThrusterParams { intensity: 0.6 + 0.6 * sv.throttle, boost: sv.boosting },
                };
                let opts = DrawOptions { double_sided: true, depth_write: false, additive: true };
                r.draw_mesh(&self.flame_mesh, &flame_model, &draw, &u, opts);
            }
        }
    }

    /// Vista previa de una nave para el menú: solo la nave, girando, sobre
    /// fondo transparente (alfa 0), lista para dibujarse encima del HUD.
    pub fn render_ship_preview(&self, r: &mut Renderer, time: f32, kind: usize) {
        let (w, h) = (r.fb.width as f32, r.fb.height as f32);
        let cam = Camera {
            eye: Vec3::new(0.0, 0.62, 2.15),
            forward: Vec3::new(0.0, -0.62, -2.15).normalize(),
            up: Vec3::Y,
            fov_y: 28f32.to_radians(),
            near: 0.05,
            far: 20.0,
        };
        r.begin_frame(cam.view(), cam.projection(w / h), Vec3::ZERO);
        let spin = time * 0.9;
        let forward = Vec3::new(spin.cos(), 0.12, spin.sin()).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let sv = ShipView {
            state: ShipState { pos: Vec3::ZERO, vel: Vec3::ZERO, forward, up: right.cross(forward) },
            kind,
            throttle: 0.45 + 0.15 * (time * 4.0).sin(),
            boosting: false,
            visible: true,
            light: None,
        };
        // Luz de estudio: arriba, adelante y a la izquierda de la cámara.
        let base = Uniforms {
            time,
            camera_pos: cam.eye,
            sun_pos: Vec3::new(-6.0, 8.0, 10.0),
            model: Mat4::IDENTITY,
            object_center: Vec3::ZERO,
        };
        self.draw_ship(r, &base, &sv);
        // Alfa: opaco donde el casco escribió profundidad; en el resto (fondo
        // negro + flama aditiva, que no escribe profundidad) el alfa sale del
        // brillo, así la flama se ve como resplandor y el fondo desaparece.
        for (px, &d) in r.fb.color.chunks_exact_mut(4).zip(r.fb.depth.iter()) {
            px[3] = if d < f32::INFINITY { 255 } else { px[0].max(px[1]).max(px[2]) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_grows_with_screen_size() {
        let n = LOD_SUBDIVISIONS.len();
        assert_eq!(select_lod(1.0, n), 0);
        assert_eq!(select_lod(f32::INFINITY, n), n - 1);
        let mut prev = 0;
        for px in [5.0, 20.0, 50.0, 100.0, 200.0, 400.0] {
            let l = select_lod(px, n);
            assert!(l >= prev);
            prev = l;
        }
    }
}
