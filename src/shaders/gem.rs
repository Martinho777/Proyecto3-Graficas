//! Gemas del modo supervivencia.
//!
//! - `GemDraw`: la gema sólida. Cada faceta tiene su normal plana; el brillo
//!   depende de cómo mira cada faceta a la cámara y al sol, así las caras
//!   "destellan" al girar. Un pulso hace que la gema respire.
//! - `GlowDraw`: resplandor aditivo (halo alrededor de la gema y fragmentos
//!   que salen volando al recogerla). Más fuerte al centro que en el borde.

use crate::math::{Vec3, smoothstep};
use crate::shader::{Shader, Uniforms, Varyings, rim};

pub struct GemDraw {
    pub color: Vec3,
}

impl Shader for GemDraw {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let n = v.normal.normalize();
        let to_cam = (u.camera_pos - v.world_pos).normalize();
        let to_sun = (u.sun_pos - v.world_pos).normalize();
        // Facetas que miran a la cámara: más claras (efecto de luz interna).
        let facing = n.dot(to_cam).max(0.0);
        let sun = n.dot(to_sun).max(0.0);
        // Destello: el reflejo del sol apunta casi exacto a la cámara.
        let h = (to_cam + to_sun).normalize();
        let sparkle = smoothstep(0.93, 0.99, n.dot(h).max(0.0));
        let pulse = 0.85 + 0.15 * (u.time * 3.0).sin();
        // Potencia alta: facetas casi de frente brillan mucho más que las
        // inclinadas, así se distingue cada cara.
        let body = self.color * (0.22 + 1.0 * facing.powf(3.0) + 0.3 * sun) * pulse;
        body + Vec3::splat(sparkle * 0.9) + self.color.lerp(Vec3::ONE, 0.5) * (0.25 * (1.0 - facing))
    }
}

pub struct GlowDraw {
    pub color: Vec3,
    /// 0 = invisible, 1 = brillo completo.
    pub intensity: f32,
}

impl Shader for GlowDraw {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let r = rim(v.normal.normalize(), v.world_pos, u.camera_pos);
        let core = 1.0 - smoothstep(0.0, 0.9, r);
        self.color * (core * core * self.intensity)
    }
}
