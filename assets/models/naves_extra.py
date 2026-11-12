"""
Naves extra del proyecto 3 (diseño propio), además del cohete de `nave_cohete.py`:

  - "Starfoz" (nave_ligera.obj): dardo delgado con alas en flecha y cola en V.
    Un solo motor pequeño. Pensada para ir rápido.
  - "Tortuga" (nave_pesada.obj): casco ancho y achatado, blindaje dorsal y dos
    motores laterales en góndolas. Pensada para ser lenta pero precisa.
  - "Devorador" (enemigo.obj): la nave maligna del modo supervivencia. Casco
    oscuro, pinzas al frente, alas de murciélago y ojos rojos que brillan.

Misma convención que el cohete: en Blender la nariz apunta a +Y y el techo a
+Z; al exportar (forward=-Z, up=Y) la nariz queda hacia -Z en el .obj.
Un punto (x, y, z) de Blender queda en (x, z, -y) del .obj.

Ejecutar desde la carpeta del proyecto:
  /Applications/Blender.app/Contents/MacOS/Blender -b -P assets/models/naves_extra.py
Genera: assets/models/nave_ligera.*, nave_pesada.* y enemigo.* (.obj, .mtl, .blend)
"""
import math
import os

import bmesh
import bpy
from mathutils import Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))


class Constructor:
    """Junta geometría en un bmesh y la exporta como .obj."""

    def __init__(self, nombre):
        bpy.ops.wm.read_factory_settings(use_empty=True)
        self.mesh = bpy.data.meshes.new(nombre)
        self.obj = bpy.data.objects.new(nombre, self.mesh)
        bpy.context.scene.collection.objects.link(self.obj)
        self.bm = bmesh.new()

    def material(self, nombre, color, emisivo=False):
        m = bpy.data.materials.new(nombre)
        m.diffuse_color = (*color, 1.0)
        m.use_nodes = True
        bsdf = m.node_tree.nodes["Principled BSDF"]
        bsdf.inputs["Base Color"].default_value = (*color, 1.0)
        if emisivo:
            # Se exporta como Ke en el .mtl: el renderer no lo ilumina.
            bsdf.inputs["Emission Color"].default_value = (*color, 1.0)
            bsdf.inputs["Emission Strength"].default_value = 1.0
        self.obj.data.materials.append(m)
        return len(self.obj.data.materials) - 1

    @staticmethod
    def _pintar(caras, mat, smooth=True):
        for f in caras:
            f.material_index = mat
            f.smooth = smooth
        return caras

    def lathe(self, perfil, segmentos, mat_de, centro=(0.0, 0.0, 0.0), escala=(1.0, 1.0)):
        """Gira un perfil [(y, radio)] alrededor de un eje paralelo a Y que
        pasa por `centro`. `escala` deforma la sección (ancho x, alto z).
        mat_de(i) da el material del anillo entre perfil[i] y perfil[i+1]."""
        bm = self.bm
        cx, cy, cz = centro
        anillos = []
        for (y, r) in perfil:
            anillo = []
            for s in range(segmentos):
                a = 2 * math.pi * s / segmentos
                anillo.append(bm.verts.new((cx + escala[0] * r * math.cos(a), cy + y, cz + escala[1] * r * math.sin(a))))
            anillos.append(anillo)
        caras = []
        for i in range(len(anillos) - 1):
            a0, a1 = anillos[i], anillos[i + 1]
            for s in range(segmentos):
                s2 = (s + 1) % segmentos
                if perfil[i][1] < 1e-4:
                    f = bm.faces.new((a0[s], a1[s2], a1[s]))
                elif perfil[i + 1][1] < 1e-4:
                    f = bm.faces.new((a0[s], a0[s2], a1[s]))
                else:
                    f = bm.faces.new((a0[s], a0[s2], a1[s2], a1[s]))
                f.material_index = mat_de(i)
                f.smooth = True
                caras.append(f)
        return caras

    def esfera(self, centro, radios, seg, anillos, mat):
        m = Matrix.Translation(Vector(centro)) @ Matrix.Diagonal((*radios, 1.0))
        r = bmesh.ops.create_uvsphere(self.bm, u_segments=seg, v_segments=anillos, radius=1.0, matrix=m)
        return self._pintar(list({f for v in r["verts"] for f in v.link_faces}), mat)

    def caja(self, centro, medidas, mat):
        m = Matrix.Translation(Vector(centro)) @ Matrix.Diagonal((*medidas, 1.0))
        r = bmesh.ops.create_cube(self.bm, size=1.0, matrix=m)
        return self._pintar(list({f for v in r["verts"] for f in v.link_faces}), mat, smooth=False)

    def placa(self, puntos_2d, grosor, transformar, mat):
        """Polígono 2D (u, v) extruido con grosor, colocado con `transformar`."""
        bm = self.bm
        abajo = [bm.verts.new(transformar(u, v, -grosor / 2)) for (u, v) in puntos_2d]
        arriba = [bm.verts.new(transformar(u, v, grosor / 2)) for (u, v) in puntos_2d]
        caras = [bm.faces.new(list(reversed(abajo))), bm.faces.new(arriba)]
        n = len(puntos_2d)
        for i in range(n):
            j = (i + 1) % n
            caras.append(bm.faces.new((abajo[i], abajo[j], arriba[j], arriba[i])))
        return self._pintar(caras, mat, smooth=False)

    def tobera(self, centro, r_frente, r_atras, largo, mat_metal, mat_fuego, seg=16):
        """Tobera abierta hacia atrás (-Y) con fondo incandescente."""
        cx, cy, cz = centro
        t = bmesh.ops.create_cone(
            self.bm, cap_ends=False, segments=seg, radius1=r_frente, radius2=r_atras, depth=largo,
            matrix=Matrix.Translation((cx, cy, cz)) @ Matrix.Rotation(math.radians(90), 4, "X"),
        )
        self._pintar(list({f for v in t["verts"] for f in v.link_faces}), mat_metal)
        self.lathe([(largo * 0.25, r_frente), (largo * 0.25, 0.0)], seg, lambda i: mat_fuego, centro=centro)

    def exportar(self, archivo):
        bm = self.bm
        bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
        bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
        bm.to_mesh(self.mesh)
        bm.free()
        self.mesh.update()
        bpy.ops.wm.save_as_mainfile(filepath=os.path.join(HERE, archivo + ".blend"))
        bpy.context.view_layer.objects.active = self.obj
        self.obj.select_set(True)
        bpy.ops.wm.obj_export(
            filepath=os.path.join(HERE, archivo + ".obj"),
            export_selected_objects=True,
            export_uv=False,
            export_normals=True,
            export_colors=False,
            export_triangulated_mesh=True,
            export_materials=True,
            forward_axis="NEGATIVE_Z",
            up_axis="Y",
        )
        print(archivo, "triangulos:", sum(len(p.vertices) - 2 for p in self.mesh.polygons))


# ============================================================ Starfoz (ligera)
def nave_ligera():
    c = Constructor("NaveLigera")
    # Gris metálico con acentos azules (estilo caza espacial clásico).
    TURQUESA = c.material("Gris", (0.62, 0.64, 0.68))
    AMARILLO = c.material("Azul", (0.15, 0.40, 0.95))
    BLANCO = c.material("GrisClaro", (0.86, 0.87, 0.90))
    VIDRIO = c.material("Vidrio", (0.95, 0.75, 0.20))
    METAL = c.material("Metal", (0.36, 0.38, 0.44))
    FUEGO = c.material("Fuego", (0.40, 0.90, 1.00), emisivo=True)
    FOCO = c.material("Foco", (0.30, 0.70, 1.00), emisivo=True)

    # Cuerpo delgado y largo, nariz afilada.
    perfil = [
        (-1.00, 0.13),
        (-0.96, 0.19),
        (-0.80, 0.24),
        (-0.50, 0.27),
        (-0.15, 0.27),
        (0.20, 0.24),
        (0.50, 0.18),
        (0.75, 0.11),
        (0.95, 0.05),
        (1.15, 0.0),
    ]

    def mat_cuerpo(i):
        y0 = perfil[i][0]
        if y0 >= 0.75:
            return AMARILLO  # punta
        if -0.80 <= y0 < -0.50:
            return BLANCO  # franja
        return TURQUESA

    # Sección un poco achatada (más ancha que alta).
    c.lathe(perfil, 20, mat_cuerpo, escala=(1.0, 0.85))
    c.lathe([(-1.00, 0.13), (-1.00, 0.0)], 20, lambda i: METAL)

    # Cabina alargada tipo burbuja.
    c.esfera((0.0, 0.18, 0.17), (0.13, 0.36, 0.13), 16, 8, VIDRIO)

    # Alas en flecha (delta), con un poco de diedro.
    ala = [(0.0, -0.80), (0.0, 0.20), (0.30, -0.25), (0.78, -0.82), (0.74, -0.95)]
    for lado in (1, -1):
        def colocar(u, v, w, lado=lado):
            return (lado * (0.20 + u), v, -0.04 + 0.08 * u + w)
        c.placa(ala if lado == 1 else list(reversed(ala)), 0.04, colocar, BLANCO)
        # Puntas de ala amarillas con luz de navegación.
        c.esfera((lado * 0.97, -0.88, 0.02), (0.035, 0.035, 0.035), 8, 6, FOCO)

    # Bandas amarillas sobre las alas (placa fina encima).
    banda = [(0.35, -0.38), (0.45, -0.50), (0.45, -0.80), (0.35, -0.80)]
    for lado in (1, -1):
        def colocar(u, v, w, lado=lado):
            return (lado * (0.20 + u), v, -0.04 + 0.08 * u + 0.025 + w)
        c.placa(banda if lado == 1 else list(reversed(banda)), 0.01, colocar, AMARILLO)

    # Cola en V: dos aletas inclinadas 35° hacia fuera.
    aleta = [(0.0, -0.98), (0.0, -0.50), (0.30, -0.82), (0.34, -1.06)]
    ang = math.radians(35)
    for lado in (1, -1):
        d = Vector((lado * math.sin(ang), 0.0, math.cos(ang)))
        n = Vector((math.cos(ang), 0.0, -lado * math.sin(ang)))
        base = Vector((lado * 0.06, 0.0, 0.15))

        def colocar(u, v, w, d=d, n=n, base=base):
            p = base + d * u + n * w
            return (p.x, v, p.z)
        pts = aleta if lado == 1 else list(reversed(aleta))
        c.placa(pts, 0.035, colocar, TURQUESA)

    # Motor pequeño.
    c.tobera((0.0, -1.06, 0.0), 0.11, 0.16, 0.16, METAL, FUEGO)
    c.exportar("nave_ligera")


# ============================================================ Tortuga (pesada)
def nave_pesada():
    c = Constructor("NavePesada")
    MOSTAZA = c.material("Mostaza", (0.93, 0.66, 0.16))
    GRAFITO = c.material("Grafito", (0.24, 0.25, 0.29))
    METAL = c.material("Metal", (0.45, 0.47, 0.52))
    VIDRIO = c.material("Vidrio", (0.35, 0.80, 0.95))
    NEGRO = c.material("Negro", (0.10, 0.10, 0.12))
    FUEGO = c.material("Fuego", (1.00, 0.40, 0.12), emisivo=True)
    FOCO = c.material("Foco", (0.60, 1.00, 0.40), emisivo=True)

    # Casco ancho y achatado, nariz roma.
    perfil = [
        (-0.88, 0.36),
        (-0.84, 0.48),
        (-0.65, 0.56),
        (-0.30, 0.60),
        (0.10, 0.58),
        (0.40, 0.50),
        (0.62, 0.38),
        (0.76, 0.22),
        (0.82, 0.08),
        (0.83, 0.0),
    ]

    def mat_cuerpo(i):
        y0 = perfil[i][0]
        if y0 >= 0.62:
            return GRAFITO  # parachoques delantero
        if -0.65 <= y0 < -0.30:
            return NEGRO  # franja
        return MOSTAZA

    c.lathe(perfil, 24, mat_cuerpo, escala=(1.30, 0.80))
    c.lathe([(-0.88, 0.36), (-0.88, 0.0)], 24, lambda i: GRAFITO, escala=(1.30, 0.80))

    # Cabina ancha y baja al frente.
    c.esfera((0.0, 0.30, 0.36), (0.34, 0.24, 0.15), 16, 8, VIDRIO)

    # Placas de blindaje sobre el lomo.
    c.caja((0.0, -0.30, 0.47), (0.50, 0.55, 0.10), GRAFITO)
    c.caja((0.0, -0.30, 0.53), (0.30, 0.40, 0.06), MOSTAZA)

    # Góndolas de motor a los lados, unidas con alas cortas y gruesas.
    pod_x, pod_z = 1.08, -0.08
    gondola = [(-0.92, 0.17), (-0.88, 0.23), (-0.40, 0.26), (0.20, 0.25), (0.42, 0.17), (0.50, 0.0)]
    for lado in (1, -1):
        centro = (lado * pod_x, 0.0, pod_z)
        c.lathe(
            gondola, 14,
            lambda i: GRAFITO if gondola[i][0] >= 0.20 else MOSTAZA,
            centro=centro,
        )
        c.tobera((lado * pod_x, -0.98, pod_z), 0.17, 0.24, 0.20, METAL, FUEGO, seg=14)

        soporte = [(0.0, -0.65), (0.0, 0.15), (0.42, 0.05), (0.42, -0.65)]

        def colocar(u, v, w, lado=lado):
            return (lado * (0.62 + u), v, pod_z + w)
        c.placa(soporte if lado == 1 else list(reversed(soporte)), 0.12, colocar, GRAFITO)
        # Faro verde al frente de cada góndola.
        c.esfera((lado * pod_x, 0.50, pod_z), (0.06, 0.04, 0.06), 8, 6, FOCO)

    # Dos aletas traseras bajas.
    aleta = [(0.0, -0.85), (0.0, -0.45), (0.22, -0.70), (0.26, -0.95)]
    for lado in (1, -1):
        def colocar(u, v, w, lado=lado):
            return (lado * 0.30 + w, v, 0.40 + u)
        pts = aleta if lado == 1 else list(reversed(aleta))
        c.placa(pts, 0.06, colocar, MOSTAZA)

    # Sin motor central: la tapa trasera es lisa.
    c.exportar("nave_pesada")


# ============================================================ Devorador (enemigo)
def nave_enemiga():
    """Nave enemiga del modo supervivencia: grande, oscura y con pinzas."""
    c = Constructor("NaveEnemiga")
    CASCO = c.material("Casco", (0.36, 0.22, 0.50))
    GUINDA = c.material("Guinda", (0.75, 0.10, 0.20))
    METAL = c.material("Metal", (0.30, 0.30, 0.36))
    OJO = c.material("Ojo", (1.00, 0.12, 0.08), emisivo=True)
    FUEGO = c.material("Fuego", (1.00, 0.20, 0.35), emisivo=True)

    # Casco ancho, aplastado, con "frente" ancho donde van los ojos.
    perfil = [
        (-1.15, 0.40),
        (-1.08, 0.55),
        (-0.80, 0.70),
        (-0.30, 0.78),
        (0.20, 0.74),
        (0.55, 0.62),
        (0.80, 0.45),
        (0.95, 0.25),
        (1.00, 0.0),
    ]

    def mat_cuerpo(i):
        y0 = perfil[i][0]
        if -0.80 <= y0 < -0.30:
            return GUINDA
        return CASCO

    c.lathe(perfil, 24, mat_cuerpo, escala=(1.35, 0.60))
    c.lathe([(-1.15, 0.40), (-1.15, 0.0)], 24, lambda i: METAL, escala=(1.35, 0.60))

    # Ojos rojos brillantes (dos grandes y dos chicos).
    for lado in (1, -1):
        c.esfera((lado * 0.30, 0.74, 0.20), (0.13, 0.08, 0.08), 10, 6, OJO)
        c.esfera((lado * 0.55, 0.55, 0.16), (0.07, 0.05, 0.05), 8, 6, OJO)

    # Pinzas al frente (como mandíbulas).
    pinza = [(0.0, 0.10), (0.0, 0.80), (0.12, 1.45), (0.34, 1.32), (0.42, 0.60), (0.32, 0.10)]
    for lado in (1, -1):
        def colocar(u, v, w, lado=lado):
            return (lado * (0.55 + u - 0.25 * max(0.0, v - 0.8)), v, -0.05 + w)
        c.placa(pinza if lado == 1 else list(reversed(pinza)), 0.14, colocar, GUINDA)

    # Alas de murciélago hacia atrás.
    ala = [(0.0, -0.90), (0.0, 0.40), (0.60, 0.10), (1.35, -0.30), (1.60, -1.25),
           (1.20, -0.90), (0.90, -1.40), (0.50, -1.00)]
    for lado in (1, -1):
        def colocar(u, v, w, lado=lado):
            return (lado * (0.80 + u), v, 0.05 - 0.12 * u + w)
        c.placa(ala if lado == 1 else list(reversed(ala)), 0.08, colocar, CASCO)

    # Púas en el lomo.
    for (y, alto) in ((0.25, 0.30), (-0.20, 0.42), (-0.65, 0.34)):
        cono = bmesh.ops.create_cone(
            c.bm, cap_ends=True, segments=6, radius1=0.10, radius2=0.0, depth=alto,
            matrix=Matrix.Translation((0.0, y, 0.40 + alto / 2)) @ Matrix.Rotation(math.radians(-20), 4, "X"),
        )
        c._pintar(list({f for v in cono["verts"] for f in v.link_faces}), GUINDA, smooth=False)

    # Tres motores atrás.
    for x in (-0.55, 0.0, 0.55):
        c.tobera((x, -1.20, 0.0), 0.16, 0.22, 0.20, METAL, FUEGO, seg=12)
    c.exportar("enemigo")


nave_ligera()
nave_pesada()
nave_enemiga()
