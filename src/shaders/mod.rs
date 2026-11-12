//! Fragment shaders propios. Cada archivo es un efecto distinto.

pub mod gem;
pub mod ghost;
pub mod haunted;
pub mod jungle;
pub mod sun;
pub mod thruster;
pub mod toon;

pub use gem::{GemDraw, GlowDraw};
pub use ghost::{GhostDraw, GhostShader};
pub use haunted::HauntedShader;
pub use jungle::JungleShader;
pub use sun::SunShader;
pub use thruster::{ThrusterDraw, ThrusterParams, ThrusterShader};
pub use toon::ToonShader;
