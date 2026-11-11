# Proyecto 3 — Sistema solar con software renderer

**🎬 [Ver video del recorrido](https://uvggt-my.sharepoint.com/:f:/r/personal/vil24033_uvg_edu_gt/_layouts/15/onedrive.aspx?id=%2Fpersonal%2Fvil24033%5Fuvg%5Fedu%5Fgt%2FDocuments%2F6o%2E%20Semestre%2FGraficas%2FVideo%20Proyecto%203&share=cgrv4iZIeQ6qQ4BN1avyYcV2EgUCR4gyZpII%5FHudufYB7LJgnw)**: vuelo espacial, acercamiento,
vuelta alrededor de un planeta y alejamiento.

Un pequeño sistema solar estilo "mini-mundos" que se explora volando un
cohete. Todo el 3D (transformaciones, recorte, rasterización, z-buffer,
shaders) está implementado en este repositorio; raylib solo se usa para la
ventana, la entrada y mostrar el framebuffer.

| Jungla jurásica | Planeta embrujado |
|---|---|
| ![Jungla](capturas/jungla.png) | ![Embrujado](capturas/embrujado.png) |

![Casona embrujada](capturas/embrujado_casona.png)

| Lava | Desierto y anillo de asteroides |
|---|---|
| ![Lava](capturas/lava.png) | ![Desierto](capturas/desierto.png) |

| Mecánico | Cristalino |
|---|---|
| ![Mecánico](capturas/mecanico.png) | ![Cristalino](capturas/cristalino.png) |

| Ciudad (día) | Ciudad (noche) |
|---|---|
| ![Ciudad](capturas/ciudad.png) | ![Ciudad de noche](capturas/ciudad_noche.png) |

| Hielo / océano | Hielo de noche, con aurora |
|---|---|
| ![Hielo](capturas/hielo.png) | ![Hielo de noche](capturas/hielo_noche.png) |

## Características que solicitamos evaluar

Total: **172 puntos técnicos** del menú (+ presentación). La nota máxima es 100.

| Característica | Pts | Dónde verlo |
|---|---|---|
| Planetas distinguibles que orbitan, rotan y se ven de cerca (hay 8, cuentan 5) | 50 | Jungla y Embrujado (`src/planets.rs`), Lava, Desierto, Mecánico y Cristalino (`src/mundos/esteban.rs`), Ciudad y Hielo (`src/mundos/emily/`) |
| Tipos de modelo propios sobre los planetas (4 × 5) | 20 | Palmera, helecho, esqueleto de dinosaurio, árbol seco, lápida, casona, fantasma, columnas de basalto, obsidiana, cactus, arcos, engranajes, antenas, cristales, monolitos, edificios, torre de radio, icebergs, iglús, faro… (scripts en `assets/models/`) |
| Distribución procedural reproducible con semilla | 10 | `src/props.rs` → `place`: dirección al azar con `Rng` sembrado, filtrada por ruido fBm, altura y pendiente. Test `same_seed_same_positions_and_on_surface` |
| Posición y orientación correctas sobre la superficie curva mientras el planeta rota | 5 | `src/props.rs` → `instance_matrix`: apoyado en la altura real, vertical = dirección radial; horneado en espacio local del planeta |
| Shaders propios con efecto distinto (4 × 6) | 24 | `src/shaders/`: sol, jungla, embrujado, toon, flama, fantasma; y lava, desierto, mecánico, cristalino, ciudad, hielo en `src/mundos/` |
| Shader animado | 8 | Plasma del sol, flama del propulsor, lava de la jungla, niebla y farolitos del embrujado, fantasmas, tráfico de la ciudad, olas y aurora del hielo |
| Mapeo de normales | 8 | Olas del planeta Hielo: la normal se perturba con el gradiente analítico de las olas (`src/mundos/emily/hielo.rs` → `mar`) |
| Deformación de la geometría con ruido | 10 | `src/planet.rs` → `build_terrain_mesh`: cada vértice de la icosfera se desplaza con la función de altura (fBm / ridged) |
| Generación / subdivisión procedural de geometría antes de rasterizar | 10 | `src/mesh.rs` → `icosphere`: icosaedro subdividido 2 a 5 veces (320 a 20 480 triángulos) |
| Aceleración, frenado y giro progresivos, iguales a cualquier FPS | 8 | `src/ship.rs`: física con paso fijo de 120 Hz e interpolación. Test `same_result_at_30_and_144_fps` |
| Colisión que impide atravesar el planeta | 4 | `src/collision.rs`: usa la altura real del terreno; aviso "CONTACTO" en el HUD |
| Descarte de objetos fuera de la vista | 7 | `src/scene.rs` → `draw_body` (esfera envolvente contra el frustum). **F1** muestra los contadores y **F3** lo apaga: vista lejana 7 628 → 43 798 triángulos procesados |
| Niveles de detalle según la distancia | 8 | `src/scene.rs` → `select_lod` (malla según el tamaño en pantalla; objetos de superficie solo de cerca). **F4** lo apaga: vista lejana 7 628 → 255 502 triángulos procesados |

No implementados: modo de control local para rodear un planeta y gravedad.

## Integrantes

| Nombre | Carné | Contribución |
|---|---|---|
| Martin Villatoro | 24033 | Pipeline de rasterización, nave y física, colisiones, planetas/LOD/culling |
| Esteban de la Peña | 24171 | Planetas Lava, Desierto, Mecánico y Cristalino, shaders, modelos y anillo de asteroides (`src/mundos/esteban.rs`) |
| Emily Góngora | 24308 | Planetas Ciudad y Hielo / océano: shaders (cuadrícula de calles por proyección de cubo, tráfico y luces nocturnas; olas con normal mapping procedural, espuma, casquetes y aurora), distribución de edificios en la cuadrícula y colonias de pingüinos, modelos (`src/mundos/emily/`, `assets/models/modelos_emily.py`) |

## Cómo ejecutar

Requisitos: Rust estable y `cmake` (macOS: `brew install cmake`).

```bash
cargo run --release
```

## Controles

| Tecla | Acción |
|---|---|
| W / S | Acelerar / frenar (reversa suave) |
| Mouse o flechas | Cabeceo y guiñada (el mouse mueve una palanca virtual) |
| A / D | Alabeo (rodar) |
| Espacio / Ctrl | Subir / bajar |
| Shift | Turbo |
| C | Cámara: tercera persona / cabina |
| R | Reiniciar la nave |
| Tab / Esc | Soltar / capturar el mouse |
| F1 | Panel de depuración (contadores del pipeline) |
| F2 | Cámara libre de depuración |
| F3 / F4 | Activar / desactivar culling y LOD (comparación) |

## Arquitectura

| Archivo | Qué hace |
|---|---|
| `src/math.rs` | Vectores y matrices propios (look-at, perspectiva, rotaciones) |
| `src/renderer.rs` | Pipeline: vertex stage, recorte, backface, rasterización multihilo, z-buffer |
| `src/mesh.rs` | Icosfera subdividida, cono, cargador `.obj`/`.mtl` |
| `src/noise.rs` | Perlin 3D con semilla, fBm, ridged, ruido celular (Worley) |
| `src/planet.rs` / `src/planets.rs` | Planetas: órbita, giro, terreno deformado, LODs / catálogo |
| `src/scene.rs` | Culling, selección de LOD y orden de dibujo |
| `src/ship.rs` | Física de vuelo con paso fijo de 120 Hz |
| `src/collision.rs` | Colisión nave–planeta con la altura real del terreno |
| `src/props.rs` | Objetos de superficie: distribución con semilla y ruido, orientación, horneado; fantasmas |
| `src/shaders/` | Fragment shaders: sol, jungla, embrujado, toon (nave y objetos), flama, fantasma |
| `assets/models/nave_cohete.py` | Script de Blender que genera la nave |
| `assets/models/modelos_superficie.py` | Script de Blender: palmera, helecho, esqueleto, árbol seco, lápida, casona, fantasma |

## Uso de IA

- **Emily Góngora:** usé un asistente de IA (Claude) como apoyo para escribir
  el código de los planetas Ciudad y Hielo (shaders y reglas de colocación en
  `src/mundos/emily/`), el script de modelos `assets/models/modelos_emily.py`
  y para iterar el aspecto con capturas y `--bench`. Revisé, probé y puedo
  explicar todo el código entregado.
