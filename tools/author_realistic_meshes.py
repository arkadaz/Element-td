"""Original Blender-authored production meshes for the realistic campaign.

Run headlessly with Blender 4.5:
  blender --background --python tools/author_realistic_meshes.py

The models deliberately use construction and anatomy rather than recolouring
the former toy pack.  They are exported as OBJ because the existing reproducible
baker already quantises OBJ triangles into the browser runtime blob.
"""
import os
import bpy
from math import cos, radians, sin
from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "original_meshes")
os.makedirs(OUT, exist_ok=True)

def clear():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)

def material(name, color, metallic=0.0, roughness=0.6):
    m = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    m.diffuse_color = (*color, 1.0)
    m.metallic, m.roughness = metallic, roughness
    return m

STONE = material("Limestone", (0.065, .075, .060), 0, .91)
# Wilderness stone must not inherit the pale cut-limestone used by tower
# footings. Its darker, cooler value keeps perimeter boulders grounded rather
# than turning them into bright egg-shaped map markers.
FIELDSTONE = material("Fieldstone", (.075, .085, .073), 0, .92)
# Keep the construction in an earthy brown range. The old near-red oak and
# blue-black iron were physically separate values but read as plastic primary
# colour blocks once reduced into a 64px command card.
WOOD = material("OiledOak", (.105, .052, .016), 0, .72)
# Trees need a separate, weather-dark bark material.  Reusing the warmer oak
# stock made the actual full-board forest read as rows of orange sticks even
# though the tower wood was appropriate.  This remains a real WOOD-response
# physical part in the renderer; it simply has the albedo of bark rather than
# a freshly oiled weapon carriage.
BARK = material("WeatheredBark", (.060, .032, .010), 0, .90)
# Blue-black forged steel is dark enough to ground the scene, but the former
# near-zero iron vanished into a black silhouette at the fixed whole-board
# camera.  This cooler value carries the reference's armour read under the
# same physically based light.
IRON = material("BlackIron", (.045, .052, .060), .85, .42)
BRONZE = material("WornBronze", (.125, .075, .022), .82, .43)
# The first browser pass mapped every broad character plate through the same
# pale stone detail as a tower edge.  Keep the construction language separate:
# the knight has cold, worn blue-black war steel while the shell runner carries
# a dull, rusted raid plate.  These source materials receive their own runtime
# part ids below, rather than asking one whole-object tint to pretend cloth,
# iron and trophy metal are the same thing.
WARPLATE = material("Warplate", (.040, .060, .072), .22, .74)
RAIDPLATE = material("RaiderPlate", (.058, .024, .007), .12, .82)
HIDE = material("Hide", (.075, .035, .012), 0, .8)
# Packrunner fur is deliberately a separate, very rough physical substance.
# The first live Swarm used a pale imported wolf and became a white stripe in
# a Brood Tide; this gives its built source mesh dark brindled fur, leather
# harness and tooth/eye accents rather than another global recolour.
FUR = material("GnollFur", (.082, .050, .020), 0, .94)
FUR_DARK = material("GnollMane", (.026, .020, .010), 0, .97)
# Keep exposed skin a subordinate, earthy material. The former saturated orange
# faces became the brightest part of a horde instead of its armour silhouette.
KNIGHT_SKIN = material("KnightSkin", (.145, .065, .018), 0, .76)
BRUTE_SKIN = material("MarauderSkin", (.060, .026, .008), 0, .82)
CHITIN = material("BeetleChitin", (.052, .022, .008), .4, .48)
# PlateEdge is a weathered dark steel/bone edge, not white paint. Its prior
# value was the brightest broad material on both opening bodies, so whole
# formations collapsed into a white-and-blue toy wall at normal board scale.
# Keep a small warm bronze trim for readable glints instead of a pale torso.
BONE = material("PlateEdge", (.045, .048, .037), .40, .68)
# The first two live creature families are deliberately not one coloured blob.
# The cloth is a separate, rough dark-blue material: it gives the armoured
# infantry its readable Warcraft-like value break without turning its steel or
# leather into team-coloured plastic.
CLOTH = material("WarBannerCloth", (.020, .042, .055), 0, .88)
GLOW = material("RunicAmber", (.86, .24, .015), .10, .34)
# Keep foliage within the dark woodland hierarchy, but not so close to black
# that every real canopy collapses to an unreadable dot at the full board view.
# Keep the foliage materially darker than an interface accent. The browser
# lighting lifts these values substantially; the former lime source albedo is
# why a supposedly woodland battlefield rendered as bright toy-green blobs.
# Foliage is deliberately an olive hierarchy rather than saturated game-green.
# Browser daylight and the filmic toe lift low albedo significantly at tactical
# distance; the previous source values therefore became fluorescent map icons
# even though their Blender swatches looked restrained.  These are still three
# material values, not a whole-object colour tint: branch shade, leaf body and
# sunward tips remain separately readable in the runtime mesh.
LEAF_DEEP = material("LeafDeep", (.016, .040, .005), 0, .96)
LEAF = material("Leaf", (.028, .064, .009), 0, .95)
LEAF_SUN = material("LeafSun", (.045, .085, .012), 0, .93)
PINE_DEEP = material("PineNeedleDeep", (.012, .030, .004), 0, .96)
PINE_LEAF = material("PineNeedle", (.018, .046, .006), 0, .95)
PINE_SUN = material("PineNeedleSun", (.030, .072, .010), 0, .93)
MOSS = material("Moss", (.020, .050, .008), 0, .96)
BUSH = material("BushLeaf", (.020, .055, .008), 0, .95)
# Ground cover lives against a very dark, oblique meadow.  Keep it an olive
# material rather than a near-black silhouette: at tactical scale a real grass
# tuft should soften a road edge, not read as a row of ink-star sprites.
VERGE = material("VergeGrass", (.055, .125, .018), 0, .88)
FERN = material("FernLeaf", (.040, .095, .014), 0, .90)

def assign(o, mat):
    o.data.materials.append(mat)
    return o

def smooth(o):
    """Preserve curved organic surfaces through OBJ export.

    The first original foliage pass had real clustered volumes, but exported
    every icosphere with flat face normals. At gameplay distance those faces
    read as a low-poly toy tree even though the silhouette was intentional.
    Smooth normals retain the physical branch/crown geometry and its shadows
    while allowing leaves, bark and moss to read as natural material.
    """
    for face in o.data.polygons:
        face.use_smooth = True
    return o

def cube(name, p, s, mat, bevel=.04, rot=None):
    bpy.ops.mesh.primitive_cube_add(location=p)
    o=bpy.context.object; o.name=name; o.scale=[x*.5 for x in s]
    if rot: o.rotation_euler=rot
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    if bevel:
        mod=o.modifiers.new("edge wear", "BEVEL"); mod.width=bevel; mod.segments=3
        bpy.context.view_layer.objects.active=o; bpy.ops.object.modifier_apply(modifier=mod.name)
    return assign(o,mat)

def cyl(name,p,r,d,mat,rot=None,verts=20):
    bpy.ops.mesh.primitive_cylinder_add(vertices=verts, radius=r, depth=d, location=p)
    o=bpy.context.object; o.name=name
    if rot:o.rotation_euler=rot
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    return smooth(assign(o,mat))

def limb(name, a, b, radius_a, radius_b, mat, verts=12):
    """A tapered branch actually joining two points in the source tree.

    The early woodland authored cylinders by Euler guesswork.  They were
    physically real but visibly stabbed through the crown, especially at the
    oblique camera.  A branch endpoint is clearer, deterministic and gives a
    forked trunk the slight bends a living tree needs without painted bark or
    alpha cards.
    """
    start, end = Vector(a), Vector(b)
    direction = end - start
    if direction.length < .002:
        return None
    bpy.ops.mesh.primitive_cone_add(
        vertices=verts,
        radius1=radius_a,
        radius2=max(radius_b, .003),
        depth=direction.length,
        location=(start + end) * .5,
    )
    o = bpy.context.object
    o.name = name
    o.rotation_euler = direction.to_track_quat('Z', 'Y').to_euler()
    bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
    return smooth(assign(o, mat))

def foundation(name, p, radius, height):
    """A weighty, cut-stone tower footing rather than a bright hockey puck.

    Every live tower used the same smooth circular limestone socket.  At the
    overview distance that became an exposed white disc underneath otherwise
    useful constructed weapons.  This short, octagonal two-course footing has
    a dark broad lower course, a smaller worn cap, and real bevels that catch
    the directional light independently.  It is intentionally shared by all
    families so a player reads a common built foundation before reading the
    weapon mounted on it.
    """
    bpy.ops.mesh.primitive_cone_add(
        vertices=8,
        radius1=radius * 1.08,
        radius2=radius * .84,
        depth=height,
        location=p,
    )
    lower = bpy.context.object
    lower.name = name + " lower course"
    bevel = lower.modifiers.new("weathered stone edges", "BEVEL")
    bevel.width = .026
    bevel.segments = 2
    bpy.context.view_layer.objects.active = lower
    bpy.ops.object.modifier_apply(modifier=bevel.name)
    assign(lower, STONE)

    bpy.ops.mesh.primitive_cylinder_add(
        vertices=8,
        radius=radius * .79,
        depth=max(.060, height * .30),
        location=(p[0], p[1], p[2] + height * .54),
    )
    cap = bpy.context.object
    cap.name = name + " upper course"
    bevel = cap.modifiers.new("worn cap edges", "BEVEL")
    bevel.width = .018
    bevel.segments = 2
    bpy.context.view_layer.objects.active = cap
    bpy.ops.object.modifier_apply(modifier=bevel.name)
    assign(cap, STONE)

def cone(name,p,r,h,mat,rot=None,verts=6):
    bpy.ops.mesh.primitive_cone_add(
        vertices=verts, radius1=r, radius2=max(r*.16, .002), depth=h, location=p
    )
    o=bpy.context.object; o.name=name
    if rot:o.rotation_euler=rot
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    return assign(o,mat)

def sphere(name,p,s,mat):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=20, ring_count=12, location=p)
    o=bpy.context.object; o.name=name; o.scale=s
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    return smooth(assign(o,mat))

def ico(name, p, s, mat, seed=0, subdivisions=2, smooth_surface=True):
    """An irregular organic volume, used for foliage and weathered stone.

    Smooth UV spheres read as toy balls from the fixed tactical camera.  An
    icosphere with deterministic, low-amplitude vertex variation gives each
    cluster a broken living silhouette without a texture-only fake or a random
    export that would invalidate the reproducible bake.
    """
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=subdivisions, radius=1.0, location=p)
    o = bpy.context.object; o.name = name; o.scale = s
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    for i, v in enumerate(o.data.vertices):
        wobble = 1.0 + .075 * sin((i + 1) * 7.13 + seed * 3.17)
        wobble += .040 * sin((i + 1) * 13.71 + seed * 1.29)
        v.co *= wobble
    o = assign(o, mat)
    return smooth(o) if smooth_surface else o

def leaf(name, p, length, width, mat, yaw, pitch=0.0):
    """A short, solid oval leaf rather than a crossed card or a sharp spike.

    The first individual-leaf pass made every leaf a long diamond.  At the
    fixed battle camera those diamonds read as a plastic starburst even though
    their material and shadow were real.  This rounded, faceted leaflet has a
    broad body, a tapered tip, an underside and a thin physical edge.  It still
    gives each canopy an actual mesh silhouette, but overlapping leaves now
    resolve into foliage rather than a rack of knives.
    """
    half = length * .5
    thick = max(.010, width * .10)
    outline = [
        (-half, 0.0),
        (-half * .52, width * .34),
        (-half * .05, width * .50),
        (half * .55, width * .31),
        (half, 0.0),
        (half * .55, -width * .31),
        (-half * .05, -width * .50),
        (-half * .52, -width * .34),
    ]
    verts = [(x, y, thick * .5) for x, y in outline]
    verts.extend((x, y, -thick * .5) for x, y in outline)
    count = len(outline)
    faces = [tuple(range(count)), tuple(range(count * 2 - 1, count - 1, -1))]
    for i in range(count):
        j = (i + 1) % count
        faces.append((i, j, count + j, count + i))
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces); mesh.update()
    o = bpy.data.objects.new(name, mesh); bpy.context.collection.objects.link(o)
    o.location = p
    o.rotation_euler = (radians(pitch), radians((pitch * .43) % 19.0), yaw)
    bpy.ops.object.select_all(action="DESELECT")
    bpy.context.view_layer.objects.active = o
    o.select_set(True)
    bpy.ops.object.transform_apply(location=False, rotation=True, scale=False)
    o.select_set(False)
    return assign(o, mat)


def leaf_cluster(name, p, sx, sy, sz, mat, seed):
    """A shallow, irregular canopy lobe made from real faceted leaf mass.

    A tree needs enough opaque, shadow-casting mass to read from the whole-map
    camera. Individual leaves alone were technically solid but collapsed into
    a bare, spidery crown; smooth spheres fixed the coverage but looked like
    green baubles. This three-ring volume is deliberately *not* round: it has
    a broad broken middle, pinched underside, uneven top and flat-catching
    facets. Several small overlapping lobes make one mature crown without
    reverting to a lollipop or a stack of decorative shelves.
    """
    sides = 11
    rings = [(-.52, .62), (-.08, 1.00), (.36, .73)]
    verts = []
    for ring, (z_mul, radial) in enumerate(rings):
        for i in range(sides):
            a = i * 6.2831853 / sides + seed * .27 + ring * .11
            wobble = (
                .83
                + .12 * sin(seed * 1.43 + i * 2.17 + ring * .67)
                + .055 * cos(seed * .81 + i * 4.11)
            )
            # Keep the crown broadly oval rather than circular, with a
            # different horizontal bias on each lobe.
            x = cos(a) * sx * radial * wobble
            y = sin(a) * sy * radial * (1.0 + .055 * sin(seed + i * 1.71))
            z = sz * (z_mul + .10 * sin(seed * 2.31 + i * 1.93))
            verts.append((x, y, z))
    bottom = len(verts)
    verts.append((0.0, 0.0, -sz * .62))
    top = len(verts)
    verts.append((sx * .05 * sin(seed), sy * .04 * cos(seed), sz * .58))
    faces = []
    for i in range(sides):
        j = (i + 1) % sides
        # Triangulation makes leaf mass facets catch light independently instead
        # of producing the plastic highlight of a smooth icosphere.
        faces.append((bottom, j, i))
        for ring in range(len(rings) - 1):
            a0, a1 = ring * sides + i, ring * sides + j
            b0, b1 = (ring + 1) * sides + i, (ring + 1) * sides + j
            if (i + ring) % 2 == 0:
                faces.extend(((a0, b0, b1), (a0, b1, a1)))
            else:
                faces.extend(((a0, b0, a1), (a1, b0, b1)))
        faces.append((top, (len(rings) - 1) * sides + i,
                      (len(rings) - 1) * sides + j))
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces); mesh.update()
    o = bpy.data.objects.new(name, mesh); bpy.context.collection.objects.link(o)
    o.location = p
    # The original three-ring mesh was intentionally irregular, but its 66
    # broad triangles were still visible as a low-poly lollipop in the actual
    # browser.  One real Catmull-Clark pass keeps the broken authored outline
    # while giving each lobe a dense, continuous leaf volume.  The separate
    # solid contour leaves still provide the small edge breakup; this is not a
    # billboard, texture trick, or a smooth proxy substituted for the canopy.
    mod = o.modifiers.new("joined living leaf volume", "SUBSURF")
    mod.subdivision_type = 'CATMULL_CLARK'
    mod.levels = 1
    mod.render_levels = 1
    bpy.context.view_layer.objects.active = o
    bpy.ops.object.modifier_apply(modifier=mod.name)
    return smooth(assign(o, mat))


def canopy_pad(name, p, sx, sy, thickness, mat, seed):
    """A layered, irregular crown shelf with real top, underside and bark-facing edge.

    The old broadleaf crown used smooth icosphere lobes.  It had plenty of
    triangles, but at the tactical camera it still reduced to a stack of green
    balls on a pole.  A small set of faceted, uneven leaf shelves has broken
    silhouette and daylight gaps between branches while remaining solid mesh
    geometry rather than billboard cards or a painted texture.
    """
    sides = 11
    verts = [(0.0, 0.0, thickness * .52)]
    for i in range(sides):
        a = i * 6.2831853 / sides
        wobble = .78 + .12 * sin(seed * 1.71 + i * 2.37) + .06 * cos(seed * .83 + i * 4.19)
        verts.append((
            cos(a) * sx * wobble,
            sin(a) * sy * wobble,
            thickness * (.30 + .09 * sin(seed + i * 1.31)),
        ))
    bottom_centre = len(verts)
    verts.append((0.0, 0.0, -thickness * .48))
    bottom_start = len(verts)
    for i in range(sides):
        a = i * 6.2831853 / sides
        wobble = .78 + .12 * sin(seed * 1.71 + i * 2.37) + .06 * cos(seed * .83 + i * 4.19)
        verts.append((
            cos(a) * sx * wobble,
            sin(a) * sy * wobble,
            -thickness * (.29 + .06 * cos(seed + i * 1.07)),
        ))
    faces = []
    for i in range(sides):
        j = (i + 1) % sides
        top_i, top_j = 1 + i, 1 + j
        bot_i, bot_j = bottom_start + i, bottom_start + j
        faces.extend(((0, top_i, top_j),
                      (bottom_centre, bot_j, bot_i),
                      (top_i, bot_i, bot_j, top_j)))
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces); mesh.update()
    o = bpy.data.objects.new(name, mesh); bpy.context.collection.objects.link(o)
    o.location = p
    return assign(o, mat)

def frustum(name, p, base, top, height, mat, yaw=0.0, bevel=0.0):
    """An actual tapered constructed volume, rather than another sphere.

    Humanoid torsos, armour plates and boots need flat catching planes and a
    readable change of width.  A bevelled box stays a box at tactical scale;
    this little eight-vertex frustum creates a real waist, chest and shoulder
    hierarchy while keeping browser horde meshes inexpensive.
    """
    bx, by = base[0] * .5, base[1] * .5
    tx, ty = top[0] * .5, top[1] * .5
    z0, z1 = -height * .5, height * .5
    verts = [(-bx,-by,z0), (bx,-by,z0), (bx,by,z0), (-bx,by,z0),
             (-tx,-ty,z1), (tx,-ty,z1), (tx,ty,z1), (-tx,ty,z1)]
    faces = [(0,1,2,3), (4,7,6,5), (0,4,5,1), (1,5,6,2),
             (2,6,7,3), (3,7,4,0)]
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces); mesh.update()
    o = bpy.data.objects.new(name, mesh); bpy.context.collection.objects.link(o)
    o.location = p; o.rotation_euler[2] = yaw
    assign(o, mat)
    if bevel:
        mod = o.modifiers.new("edge wear", "BEVEL")
        mod.width = bevel; mod.segments = 2
        bpy.context.view_layer.objects.active = o
        bpy.ops.object.modifier_apply(modifier=mod.name)
    return o

def kite_plate(name, p, width, height, depth, mat, yaw=0.0, bevel=0.0):
    """A solid, faceted kite shield or breast plate facing local -Y.

    The previous rounded shield was a shiny oval that made every infantryman
    resemble a toy soldier.  This mesh has a pointed lower edge, shoulder
    corners and real thickness, so its silhouette survives a dense horde.
    """
    profile = [(0.0, height * .50), (width * .43, height * .28),
               (width * .50, -height * .08), (0.0, -height * .50),
               (-width * .50, -height * .08), (-width * .43, height * .28)]
    verts = []
    for y in (-depth * .5, depth * .5):
        verts.extend([(x, y, z) for x, z in profile])
    n = len(profile)
    faces = [tuple(range(n)), tuple(range(2 * n - 1, n - 1, -1))]
    for i in range(n):
        j = (i + 1) % n
        faces.append((i, j, n + j, n + i))
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces); mesh.update()
    o = bpy.data.objects.new(name, mesh); bpy.context.collection.objects.link(o)
    o.location = p; o.rotation_euler[2] = yaw
    assign(o, mat)
    if bevel:
        mod = o.modifiers.new("edge wear", "BEVEL")
        mod.width = bevel; mod.segments = 2
        bpy.context.view_layer.objects.active = o
        bpy.ops.object.modifier_apply(modifier=mod.name)
    return o

def export(name):
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.wm.obj_export(filepath=os.path.join(OUT, name + ".obj"), export_materials=True)

def tower_seed(stage, recoil=0.0):
    clear()
    # A compact, weight-bearing ballista: stone socket, oak bed, a visible
    # iron windlass, and a wide bow silhouette.  The prior version had the
    # right parts but its rotated limbs separated in the tactical view and
    # read as a few floating brown sticks.  Keep the primary silhouette broad
    # and attached before adding upgrade detail.
    kick = .11 * recoil
    grade = stage / 3.0
    foundation("limestone socket", (0, 0, .12), .70 + .045 * stage, .24)
    cyl("cut stone course", (0, 0, .27), .57 + .025 * stage, .11, STONE)
    cube("oak bed", (0, 0, .42), (1.10, .86, .18), WOOD, .055)
    cube("iron turntable", (0, .02, .54), (.72, .58, .13), IRON, .035)
    # Two planted A-frame legs carry the stock.  They create a readable
    # constructed base at overview scale instead of a hovering weapon sprite.
    for x in (-.34, .34):
        cube("oak A frame", (x, .12, .66), (.14, .18, .54), WOOD, .028,
             (0, radians(-18 if x < 0 else 18), 0))
        cyl("bronze foot pin", (x, .18, .47), .075, .12, BRONZE, verts=12)
    cube("ballista stock", (0, -.18 + kick, .77), (.22, 1.18, .18), WOOD, .040)
    cube("iron guide", (0, -.43 + kick, .89), (.10, .72, .09), IRON, .020)
    # The bow is deliberately one connected crossbeam with short swept tips;
    # it stays recognisable as a weapon when reduced to a 64px command icon.
    cube("oak bow crossbar", (0, -.57 + kick, .86), (1.34, .16, .15), WOOD, .035)
    for x in (-.61, .61):
        cube("bow tip", (x, -.60 + kick, .94), (.18, .14, .34), WOOD, .026,
             (0, radians(-24 if x < 0 else 24), 0))
        cube("string brace", (x * .48, -.57 + kick, .91), (.025, .12, .21), HIDE, .006,
             (0, radians(32 if x < 0 else -32), 0))
    cyl("bronze windlass", (0, .20 + kick, .67), .105, .82, BRONZE,
         (0, radians(90), 0), 14)
    for x in (-.50, .50):
        cyl("windlass handle", (x, .20 + kick, .67), .048, .26, IRON,
             (radians(90), 0, 0), 12)
    cyl("iron bolt", (0, -.93 + kick, .89), .040, 1.16, IRON,
         (radians(90), 0, 0), 12)
    cone("bolt head", (0, -1.54 + kick, .89), .10, .24, BONE,
         (radians(90), 0, 0), 8)
    # Upgrade construction is deliberately local: braces, a shield plate and
    # an amber sight reinforce the same ballista rather than swapping it for a
    # differently coloured toy.
    if stage >= 1:
        for x in (-.47, .47):
            cube("iron limb strap", (x, -.56 + kick, .86), (.11, .19, .20), IRON, .016)
    if stage >= 2:
        cube("armoured mantlet", (0, .31, .79), (.86, .10, .43), IRON, .030)
        for x in (-.30, .30):
            cyl("mantlet rivet", (x, .245, .82), .045, .07, BRONZE,
                (radians(90), 0, 0), 10)
    if stage >= 3:
        sphere("amber sight", (0, -.68 + kick, 1.03), (.105, .075, .105), GLOW)
        cube("apex bow crown", (0, -.56 + kick, 1.08), (.34, .11, .10), BRONZE, .018)
    export("TowerSeed%d%s" % (stage, "_pose" if recoil else ""))

def tower_siege(stage, recoil=0.0):
    clear()
    kick = .09 * recoil
    # A field mortar is a squat carriage with a clearly hollow, iron barrel.
    # The former tilted tube presented its end caps as two giant blue discs;
    # this low, horizontal construction preserves the cannon silhouette from
    # the fixed overhead view while still giving it real iron/bronze/wood parts.
    foundation("limestone socket", (0, 0, .13), .77 + .045 * stage, .26)
    cube("oak carriage", (0, .04, .36), (1.20, .94, .22), WOOD, .055)
    cube("iron cradle", (0, -.02 + kick, .57), (.66, .70, .18), IRON, .040)
    for x in (-.46, .46):
        cyl("wood wheel", (x, .16, .43), .25, .13, WOOD,
             (0, radians(90), 0), 18)
        cyl("bronze hub", (x, .16, .43), .10, .16, BRONZE,
             (0, radians(90), 0), 14)
        cube("carriage cheek", (x, -.04 + kick, .67), (.14, .74, .39), WOOD, .030)
        cyl("iron trunnion", (x, -.18 + kick, .78), .105, .22, IRON,
             (0, radians(90), 0), 14)
    # Axis along local Y.  A small raised rear and a wider muzzle make this
    # a barrel rather than a stack of discs under the nearly overhead camera.
    cyl("forged mortar barrel", (0, -.28 + kick, .88), .245, 1.12, IRON,
         (radians(90), 0, 0), 28)
    cyl("bronze reinforce collar", (0, -.49 + kick, .88), .286, .16, BRONZE,
         (radians(90), 0, 0), 28)
    cyl("black muzzle rim", (0, -.82 + kick, .88), .300, .13, IRON,
         (radians(90), 0, 0), 28)
    cyl("dark bore", (0, -.90 + kick, .88), .215, .010, HIDE,
         (radians(90), 0, 0), 24)
    cube("recoil stop", (0, .33 + kick, .69), (.72, .15, .32), WOOD, .030)
    if stage >= 1:
        for x in (-.27, .27):
            cube("iron carriage brace", (x, .07, .65), (.09, .84, .12), IRON, .020)
    if stage >= 2:
        cyl("counterweight", (0, .42 + kick, .80), .22, .22, BRONZE, verts=18)
        cube("stone ammo chest", (.53, .29, .58), (.28, .42, .28), STONE, .030)
    if stage >= 3:
        sphere("ember fuse", (0, .20 + kick, 1.05), (.095, .095, .095), GLOW)
        for x in (-.36, .36):
            cyl("apex barrel band", (x * .10, -.67 + kick, .88), .265, .075, BRONZE,
                (radians(90), 0, 0), 24)
    export("TowerSiege%d%s" % (stage, "_pose" if recoil else ""))

def tower_bounce(stage, recoil=0.0):
    """A constructed ricochet thrower, not a recoloured imported gun."""
    clear()
    kick = .10 * recoil
    foundation("cut stone socket", (0, 0, .13), .72 + stage * .04, .26)
    cube("heavy oak table", (0, 0, .38), (1.18, .96, .24), WOOD, .055)
    cube("iron swivel", (0, -.02, .56), (.68, .62, .14), IRON, .035)
    for x in (-.36, .36):
        cube("oak throwing cheek", (x, .04, .76), (.15, .22, .66), WOOD, .030,
             (0, radians(-14 if x < 0 else 14), 0))
        cyl("bronze pivot pin", (x, -.08, .82), .085, .18, BRONZE,
             (0, radians(90), 0), 12)
    # A forked iron arm and an actual raised throwing disc give this family a
    # readable silhouette from above. Recoil moves the entire working head.
    cube("iron throw arm", (0, -.32 + kick, .95), (.16, .96, .13), IRON, .035)
    cyl("bronze sling drum", (0, -.66 + kick, .98), .18, .14, BRONZE,
        (radians(90), 0, 0), 18)
    cyl("throwing disc", (0, -.94 + kick, 1.00), .245, .075, BONE,
        (radians(90), 0, 0), 20)
    cube("dark disc hub", (0, -1.00 + kick, 1.00), (.13, .055, .13), IRON, .018)
    cube("counterweight beam", (0, .32 + kick, .78), (.20, .45, .18), WOOD, .035)
    if stage >= 1:
        for x in (-.30, .30):
            cube("iron stabiliser", (x, .10, .63), (.09, .78, .12), IRON, .018)
    if stage >= 2:
        cyl("reserve disc", (.43, .26, .66), .17, .08, BONE, (radians(90), 0, 0), 18)
        cube("stone ammunition chest", (-.48, .25, .57), (.29, .42, .28), STONE, .030)
    if stage >= 3:
        sphere("amber trajectory lens", (0, -.42 + kick, 1.16), (.095, .085, .095), GLOW)
        for x in (-.48, .48):
            cone("fork crown", (x, -.33 + kick, 1.05), .08, .24, BONE,
                 (0, radians(-20 if x < 0 else 20), 0), 7)
    export("TowerBounce%d%s" % (stage, "_pose" if recoil else ""))

def tower_multi(stage, recoil=0.0):
    """A three-bow volley nest with enough wood/iron construction to belong on grass."""
    clear()
    kick = .085 * recoil
    foundation("rough stone plinth", (0, 0, .13), .74 + stage * .04, .26)
    cube("oak firing deck", (0, 0, .39), (1.34, .92, .23), WOOD, .055)
    cube("iron turntable", (0, .01, .55), (.88, .64, .13), IRON, .035)
    for x in (-.34, 0, .34):
        cube("volley bow stock", (x, -.20 + kick, .78), (.16, 1.06, .14), WOOD, .032)
        cube("volley iron guide", (x, -.51 + kick, .91), (.075, .60, .07), IRON, .018)
        cube("volley bow limb", (x, -.61 + kick, .88), (.42, .12, .12), WOOD, .030)
        cyl("bronze bow pin", (x, -.38 + kick, .83), .055, .13, BRONZE,
            (0, radians(90), 0), 12)
        cyl("iron bolt", (x, -.91 + kick, .91), .032, .72, IRON,
            (radians(90), 0, 0), 12)
        cone("bolt head", (x, -1.29 + kick, .91), .075, .18, BONE,
             (radians(90), 0, 0), 7)
    cube("oak windlass rail", (0, .25 + kick, .68), (.92, .14, .16), WOOD, .035)
    if stage >= 1:
        cube("iron volley shield", (0, .35, .83), (1.04, .10, .40), IRON, .032)
    if stage >= 2:
        for x in (-.48, .48):
            cube("extra bow brace", (x, -.40 + kick, .79), (.10, .82, .12), IRON, .018)
    if stage >= 3:
        sphere("amber command sight", (0, -.69 + kick, 1.08), (.10, .075, .10), GLOW)
        for x in (-.40, .40):
            cyl("apex bolt rack", (x, .22, .91), .07, .42, BRONZE, verts=14)
    export("TowerMulti%d%s" % (stage, "_pose" if recoil else ""))

def tower_corrupt(stage, recoil=0.0):
    """A dark stone hex obelisk held together by visible metal and bone braces."""
    clear()
    pulse = .035 * recoil
    foundation("weathered stone dais", (0, 0, .13), .76 + stage * .045, .26)
    cube("blackstone foot", (0, 0, .39), (.86, .78, .20), STONE, .050)
    # Four tapered blocks build a real faceted obelisk rather than a glowing
    # crystal sprite. The amber runes remain small and local.
    cone("dark obelisk core", (0, 0, .89 + pulse), .34, 1.14 + stage * .07, IRON, verts=6)
    cyl("bronze binding ring", (0, 0, .61 + pulse), .36, .10, BRONZE, verts=18)
    cyl("bronze upper ring", (0, 0, 1.10 + pulse), .23, .08, BRONZE, verts=18)
    for i in range(4):
        a = i * 1.5708 + .35
        x, y = cos(a) * .48, sin(a) * .48
        cube("oak ritual brace", (x, y, .55), (.14, .14, .58), WOOD, .025,
             (radians(16 * sin(a)), radians(16 * cos(a)), a))
        cone("bone ward spike", (x * 1.13, y * 1.13, .77), .075, .34, BONE,
             (radians(12 * sin(a)), radians(12 * cos(a)), a), 7)
    sphere("amber runic heart", (0, -.30, .91 + pulse), (.095, .06, .13), GLOW)
    if stage >= 1:
        for i in range(3):
            a = i * 2.094 + .2
            cyl("iron tether", (cos(a) * .33, sin(a) * .33, .50), .045, .38, IRON, verts=12)
    if stage >= 2:
        for i in range(4):
            a = i * 1.5708
            sphere("bound amber pebble", (cos(a) * .46, sin(a) * .46, .56), (.075, .075, .075), GLOW)
    if stage >= 3:
        cone("apex ward crown", (0, 0, 1.56 + pulse), .13, .32, BONE, verts=6)
    export("TowerCorrupt%d%s" % (stage, "_pose" if recoil else ""))

def tower_air(stage, recoil=0.0):
    """A compact sky lance battery: mast, guide rings, fins and actual darts."""
    clear()
    kick = .07 * recoil
    foundation("cut stone socket", (0, 0, .13), .74 + stage * .04, .26)
    cube("oak sky platform", (0, 0, .39), (1.18, 1.02, .22), WOOD, .055)
    cube("iron mast socket", (0, 0, .56), (.52, .52, .14), IRON, .035)
    cyl("oak launch mast", (0, .02 + kick, .95), .15, .94, WOOD, verts=16)
    for z in (.72, 1.06):
        cyl("bronze guide collar", (0, .02 + kick, z), .21, .09, BRONZE, verts=18)
    for x in (-.31, .31):
        cube("iron sky rail", (x, -.18 + kick, .98), (.09, .78, .11), IRON, .022)
        cone("sky lance", (x, -.65 + kick, 1.10), .075, .74, BONE,
             (radians(90), 0, 0), 8)
        cube("lance fin", (x, -.38 + kick, 1.10), (.22, .08, .07), IRON, .014)
    for x in (-.43, .43):
        cube("oak angled stay", (x, .16, .67), (.12, .18, .64), WOOD, .025,
             (0, radians(-23 if x < 0 else 23), 0))
    if stage >= 1:
        cube("iron wind shroud", (0, .24, 1.02), (.74, .12, .43), IRON, .032)
    if stage >= 2:
        for x in (-.47, .47):
            cyl("reserve lance tube", (x, .29, .67), .08, .42, BONE, verts=12)
    if stage >= 3:
        sphere("amber altitude sight", (0, -.28 + kick, 1.34), (.09, .07, .09), GLOW)
        cone("mast crown", (0, .02 + kick, 1.52), .12, .30, BRONZE, verts=7)
    export("TowerAir%d%s" % (stage, "_pose" if recoil else ""))

def tower_chaos(stage, recoil=0.0):
    """A furnace-spitter built from masonry, bellows and iron rather than a neon laser."""
    clear()
    kick = .10 * recoil
    foundation("black stone furnace base", (0, 0, .14), .78 + stage * .045, .28)
    cube("oak fire cradle", (0, .02, .38), (1.16, .94, .24), WOOD, .055)
    cube("iron furnace jacket", (0, -.04 + kick, .70), (.66, .64, .58), IRON, .055)
    cyl("bronze blast collar", (0, -.43 + kick, .78), .29, .18, BRONZE,
        (radians(90), 0, 0), 22)
    cyl("iron blast barrel", (0, -.66 + kick, .79), .22, .60, IRON,
        (radians(90), 0, 0), 22)
    cyl("black fire mouth", (0, -.99 + kick, .79), .18, .018, HIDE,
        (radians(90), 0, 0), 18)
    for x in (-.36, .36):
        cube("oak bellows arm", (x, .16, .67), (.13, .52, .38), WOOD, .030,
             (0, radians(-15 if x < 0 else 15), 0))
        cone("iron heat fin", (x, -.23 + kick, 1.05), .075, .30, IRON,
             (0, radians(-25 if x < 0 else 25), 0), 7)
    sphere("amber fire heart", (0, -.20 + kick, .89), (.11, .07, .11), GLOW)
    if stage >= 1:
        cube("iron furnace shield", (0, .33, .84), (.90, .10, .43), IRON, .030)
    if stage >= 2:
        for x in (-.48, .48):
            cyl("bronze pressure pipe", (x, .05, .80), .065, .55, BRONZE, verts=14)
    if stage >= 3:
        cone("apex fire chimney", (0, .08, 1.35), .17, .42, BRONZE, verts=8)
        sphere("apex furnace rune", (0, -.46 + kick, 1.07), (.085, .055, .085), GLOW)
    export("TowerChaos%d%s" % (stage, "_pose" if recoil else ""))

def tower_destroy(stage, recoil=0.0):
    """A brutal stone ram cannon with a broad grounded silhouette."""
    clear()
    kick = .11 * recoil
    foundation("rough stone foundation", (0, 0, .14), .80 + stage * .045, .28)
    cube("reinforced oak sled", (0, 0, .38), (1.34, 1.02, .24), WOOD, .060)
    for x in (-.48, .48):
        cyl("heavy wood wheel", (x, .17, .43), .26, .15, WOOD,
            (0, radians(90), 0), 18)
        cyl("iron wheel hub", (x, .17, .43), .10, .18, IRON,
            (0, radians(90), 0), 14)
        cube("iron ram cheek", (x, -.06 + kick, .70), (.14, .82, .42), IRON, .035)
    cube("oak ram bed", (0, -.12 + kick, .68), (.58, 1.12, .20), WOOD, .045)
    cyl("black ram barrel", (0, -.45 + kick, .90), .265, 1.14, IRON,
        (radians(90), 0, 0), 26)
    cone("stone breaker head", (0, -1.08 + kick, .90), .33, .42, BONE,
         (radians(90), 0, 0), 8)
    cyl("bronze ram band", (0, -.62 + kick, .90), .296, .14, BRONZE,
        (radians(90), 0, 0), 24)
    if stage >= 1:
        cube("iron ram mantle", (0, .30, .84), (1.02, .11, .48), IRON, .035)
    if stage >= 2:
        cube("stone counterweight", (0, .48 + kick, .84), (.56, .28, .44), STONE, .040)
    if stage >= 3:
        sphere("amber breach sight", (0, -.42 + kick, 1.17), (.10, .075, .10), GLOW)
        for x in (-.34, .34):
            cone("breaker crown", (x, -.68 + kick, 1.12), .08, .25, BONE, verts=7)
    export("TowerDestroy%d%s" % (stage, "_pose" if recoil else ""))

def tower_aura(stage, recoil=0.0):
    """A carved command beacon: banner, rune frame and small local light."""
    clear()
    pulse = .03 * recoil
    foundation("circular stone dais", (0, 0, .14), .76 + stage * .045, .28)
    cube("carved stone root", (0, 0, .39), (.86, .82, .20), STONE, .050)
    cyl("oak banner mast", (0, 0, .94 + pulse), .12, 1.10, WOOD, verts=16)
    cyl("bronze mast collar", (0, 0, .67 + pulse), .19, .10, BRONZE, verts=18)
    cube("dark command banner", (.18, .02, 1.15 + pulse), (.39, .08, .42), CLOTH, .022)
    cube("bronze banner bar", (.18, .02, 1.37 + pulse), (.52, .08, .07), BRONZE, .014)
    for x in (-.36, .36):
        cube("iron ritual pillar", (x, .06, .70), (.12, .16, .62), IRON, .030)
        sphere("amber pillar rune", (x, -.05, .82), (.07, .04, .08), GLOW)
    if stage >= 1:
        for a in (0.0, 2.094, 4.188):
            cone("bone command ward", (cos(a) * .47, sin(a) * .47, .63), .07, .30, BONE, verts=7)
    if stage >= 2:
        cube("iron signal ring", (0, 0, 1.15 + pulse), (.58, .58, .08), IRON, .022)
    if stage >= 3:
        cone("banner apex", (0, 0, 1.64 + pulse), .13, .30, BONE, verts=7)
        sphere("apex command ember", (0, -.15, 1.34 + pulse), (.09, .06, .09), GLOW)
    export("TowerAura%d%s" % (stage, "_pose" if recoil else ""))

def tower_demon(stage, recoil=0.0):
    """A thorned demon ballista, with bone limbs and dark forge hardware."""
    clear()
    kick = .10 * recoil
    foundation("charred stone footing", (0, 0, .14), .78 + stage * .045, .28)
    cube("dark oak altar bed", (0, 0, .39), (1.22, .94, .24), WOOD, .055)
    cube("black iron pivot", (0, -.02, .57), (.70, .60, .14), IRON, .035)
    cube("demon stock", (0, -.20 + kick, .80), (.22, 1.18, .18), WOOD, .040)
    cube("iron demon guide", (0, -.52 + kick, .93), (.10, .72, .10), IRON, .020)
    for x in (-.58, .58):
        cone("bone bow limb", (x, -.55 + kick, .93), .12, .58, BONE,
             (0, radians(-28 if x < 0 else 28), 0), 7)
        cone("iron limb thorn", (x * .92, -.65 + kick, 1.09), .06, .24, IRON,
             (0, radians(-22 if x < 0 else 22), 0), 7)
    cyl("bronze blood winch", (0, .20 + kick, .70), .11, .82, BRONZE,
        (0, radians(90), 0), 14)
    cone("bone demon bolt", (0, -1.08 + kick, .94), .095, .76, BONE,
         (radians(90), 0, 0), 8)
    sphere("amber cursed sight", (0, -.48 + kick, 1.10), (.085, .055, .085), GLOW)
    if stage >= 1:
        cube("iron horn mantle", (0, .30, .84), (.90, .11, .43), IRON, .030)
    if stage >= 2:
        for x in (-.38, .38):
            cone("altar horn", (x, .23, .98), .09, .36, BONE,
                 (0, radians(-18 if x < 0 else 18), 0), 7)
    if stage >= 3:
        sphere("apex demon ember", (0, -.70 + kick, 1.18), (.10, .06, .10), GLOW)
    export("TowerDemon%d%s" % (stage, "_pose" if recoil else ""))

def tower_king(stage, recoil=0.0):
    """A royal multi-bow with a crown silhouette, not a flat gold recolour."""
    clear()
    kick = .08 * recoil
    foundation("polished stone socket", (0, 0, .14), .78 + stage * .045, .28)
    cube("royal oak deck", (0, 0, .39), (1.34, .96, .24), WOOD, .055)
    cube("iron royal turntable", (0, .01, .56), (.90, .66, .14), IRON, .035)
    for x in (-.36, 0, .36):
        cube("royal bow stock", (x, -.22 + kick, .82), (.16, 1.08, .14), WOOD, .032)
        cube("royal iron guide", (x, -.53 + kick, .96), (.075, .65, .075), IRON, .018)
        cube("royal bow limb", (x, -.65 + kick, .93), (.42, .12, .12), BONE, .028)
        cyl("royal bronze pin", (x, -.39 + kick, .88), .06, .14, BRONZE,
            (0, radians(90), 0), 12)
        cone("royal bolt", (x, -1.16 + kick, .96), .070, .56, BONE,
             (radians(90), 0, 0), 7)
    cube("bronze crown rail", (0, .27, .76), (1.02, .14, .16), BRONZE, .035)
    for x in (-.38, 0, .38):
        cone("royal crown point", (x, .26, 1.06), .09, .30, BRONZE, verts=7)
    if stage >= 1:
        cube("iron royal mantle", (0, .36, .88), (1.05, .10, .43), IRON, .030)
    if stage >= 2:
        for x in (-.48, .48):
            sphere("royal amber seal", (x, .20, .93), (.07, .05, .07), GLOW)
    if stage >= 3:
        sphere("apex crown jewel", (0, -.70 + kick, 1.16), (.10, .065, .10), GLOW)
    export("TowerKing%d%s" % (stage, "_pose" if recoil else ""))

def humanoid(pose=0.0):
    clear()
    # Opening infantry must look like constructed fantasy soldiers, not a
    # collection of polished spheres.  Flat armour planes, a tapered torso,
    # a real visor and a solid kite shield establish the blue-black silhouette
    # first; seams and fittings are secondary.  Both poses retain exactly this
    # object order so `bake_models.py` can interpolate a genuine gait.
    for x in (-.18, .18):
        stride = (.11 if x < 0 else -.11) * pose
        frustum("square sabaton", (x, stride - .035, .10), (.27,.36), (.23,.31), .20, HIDE, bevel=.025)
        frustum("segmented greave", (x, stride * .64, .35), (.22,.22), (.18,.19), .32, WARPLATE, bevel=.018)
        kite_plate("knee plate", (x, stride * .44 - .105, .55), .18, .20, .060, WARPLATE, bevel=.012)
        frustum("blue thigh", (x, stride * .20, .72), (.23,.23), (.19,.20), .34, CLOTH, bevel=.016)

    frustum("split tabard", (0, .035, .88), (.58,.37), (.47,.32), .43, CLOTH, bevel=.024)
    for x in (-.15, .15):
        kite_plate("folded cloak", (x, .198, 1.02), .23, .55, .055, CLOTH, bevel=.010)
    frustum("forged cuirass", (0, -.01, 1.13), (.62,.42), (.72,.37), .48, WARPLATE, bevel=.035)
    kite_plate("blue breast tabard", (0, -.235, 1.10), .38, .39, .040, CLOTH, bevel=.009)
    cube("bronze belt", (0, -.035, .93), (.62,.40,.075), BRONZE, .018)
    for x in (-.20, .20):
        sphere("breast rivet", (x, -.262, 1.19), (.035,.022,.035), BRONZE)

    # Helmet: angular crown, exposed shadowed face and a separate visor instead
    # of the old glossy ball. The crest stays small so it reads as faction
    # identity rather than a toy cone.
    ico("shadowed face", (0, -.105, 1.48), (.17,.13,.20), KNIGHT_SKIN, 211, subdivisions=2)
    # A rounded war helm has a real curved crown and a separately constructed
    # brow/neck guard.  The former beveled frustum was mathematically detailed
    # but still read as a single square head at game scale.
    ico("rounded war helm", (0, -.015, 1.61), (.255,.215,.235), WARPLATE, 131, subdivisions=2)
    cube("visor brow", (0, -.235, 1.58), (.34,.050,.080), WARPLATE, .010)
    frustum("neck guard", (0, .145, 1.50), (.34,.13), (.26,.10), .18, WARPLATE, bevel=.012)
    cube("nose guard", (0, -.258, 1.48), (.050,.035,.17), BONE, .008)
    cube("blue crest", (0, .030, 1.80), (.12,.12,.26), CLOTH, .020)

    for x in (-.38, .38):
        arm = (-.08 if x < 0 else .08) * pose
        ico("rounded pauldron", (x, arm, 1.36), (.17,.15,.13), WARPLATE, 171 + int((x + .4) * 40), subdivisions=2)
        cyl("blue sleeve", (x, arm * .58, 1.09), .095, .30, CLOTH,
            (radians(arm * 23), radians(24 if x < 0 else -24), 0), 12)
        frustum("dark gauntlet", (x * 1.10, arm * .82 - .060, .91), (.18,.15), (.14,.13), .18, BONE, bevel=.014)

    shield_y = -.13 - .055 * pose
    kite_plate("dark kite shield", (-.47, shield_y, 1.10), .46, .70, .090, WARPLATE, bevel=.016)
    kite_plate("shield face", (-.47, shield_y - .052, 1.10), .36, .54, .020, CLOTH, bevel=.006)
    cyl("shield boss", (-.47, shield_y - .105, 1.12), .070, .11, BRONZE,
        (0, radians(90), 0), 12)
    # A short broad sword remains visibly attached to the gauntlet without
    # becoming a vertical strip through neighbouring ranks.
    cube("short sword blade", (.47, -.105 + .085 * pose, 1.14), (.075,.060,.56), WARPLATE, .010)
    cone("sword point", (.47, -.105 + .085 * pose, 1.48), .074, .17, WARPLATE, verts=4)
    cube("sword fuller", (.47, -.146 + .085 * pose, 1.16), (.024,.014,.40), IRON, .004)
    cube("sword guard", (.47, -.105 + .085 * pose, .86), (.27,.068,.055), BRONZE, .009)
    cyl("sword grip", (.47, -.105 + .085 * pose, .72), .038, .22, HIDE, verts=10)
    sphere("sword pommel", (.47, -.105 + .085 * pose, .59), (.060,.060,.060), BRONZE)
    export("Warrior" + ("_pose" if pose else ""))

def beetle(pose=0.0):
    clear()
    # The opening heavy needs a different *construction language* from the
    # knight: a hunched shell, trophy plates and split horns rather than simply
    # a larger round humanoid. This keeps the first mixed formation readable at
    # whole-board scale and supplies an actual counter silhouette for armour.
    # The opening shellbeast must visibly lumber through its two authored
    # contact poses at full-board scale.  A tiny bob survived the export but
    # looked frozen once hundreds marched together, so the heavy now has a
    # deliberate stomp, shoulder sway and weapon follow-through.
    bob = .065 * pose
    for x in (-.25, .25):
        stride = (.30 if x < 0 else -.30) * pose
        frustum("heavy boot", (x, stride - .030, .12), (.35,.40), (.29,.34), .23, HIDE, bevel=.028)
        frustum("chitin greave", (x, stride * .68, .40 + bob), (.29,.28), (.23,.24), .39, CHITIN, bevel=.018)
        kite_plate("trophy knee", (x, stride * .42 - .13, .64 + bob), .23, .22, .070, RAIDPLATE, bevel=.012)
        frustum("heavy thigh", (x, stride * .19, .87 + bob), (.32,.31), (.25,.27), .41, CHITIN, bevel=.022)

    frustum("broad pelvis", (0, .025, .90 + bob), (.74,.51), (.64,.46), .31, HIDE, bevel=.028)
    frustum("hunched shell base", (0, .030, 1.28 + bob), (.82,.60), (.66,.51), .61, CHITIN, bevel=.040)
    # Overlapping carapace lobes create a genuinely rounded, weight-bearing
    # shell instead of a single large low-poly box. They remain separate from
    # the trophy metal so the runner reads as chitin plus scavenged armour.
    ico("left raised carapace", (-.22, .075, 1.40 + bob), (.31,.26,.27), CHITIN, 301, subdivisions=2)
    ico("right raised carapace", (.22, .075, 1.40 + bob), (.31,.26,.27), CHITIN, 302, subdivisions=2)
    # Split trophy plates catch independently and give the dark rust shell a
    # hard silhouette, rather than a single bright rectangular chest.
    for x in (-.21, .21):
        kite_plate("split breast plate", (x, -.327, 1.28 + bob), .34, .48, .075, RAIDPLATE, bevel=.016)
    cube("broad iron belt", (0, -.08, 1.03 + bob), (.71,.46,.10), IRON, .020)
    for i, z in enumerate((1.56, 1.36, 1.16)):
        frustum("overlap shell", (0, .235 + i * .012, z + bob),
                (.70 - i * .055,.14), (.56 - i * .045,.12), .18, CHITIN, bevel=.014)
    for x in (-.28, 0, .28):
        cone("shell ridge", (x, .06, 1.71 + bob), .085, .24, RAIDPLATE,
             (0, radians(-13 if x < 0 else 13), 0), 6)

    frustum("thick neck", (0, -.01, 1.59 + bob), (.37,.31), (.30,.25), .19, HIDE, bevel=.018)
    ico("brute face", (0, -.155, 1.73 + bob), (.26,.20,.25), BRUTE_SKIN, 311, subdivisions=2)
    ico("brow helm", (0, -.015, 1.84 + bob), (.28,.23,.16), RAIDPLATE, 371, subdivisions=2)
    cube("broken nose guard", (0, -.278, 1.77 + bob), (.070,.040,.19), BONE, .008)
    for x in (-.20, .20):
        cone("swept horn", (x, -.04, 1.80 + bob), .085, .31, BONE,
             (radians(-20), radians(-35 if x < 0 else 35), 0), 7)
        sphere("amber eye", (x * .72, -.305, 1.63 + bob), (.045, .035, .045), GLOW)

    for x in (-.48, .48):
        arm = (-.22 if x < 0 else .22) * pose
        ico("brute pauldron", (x, arm, 1.48 + bob), (.22,.19,.15), RAIDPLATE, 391 + int((x + .5) * 20), subdivisions=2)
        cyl("brute upper arm", (x, arm * .55, 1.19 + bob), .13, .38, CHITIN,
            (radians(arm * 20), radians(18 if x < 0 else -18), 0), 12)
        frustum("brute fist", (x * 1.08, arm * .82 - .05, .99 + bob), (.22,.18), (.17,.15), .19, IRON, bevel=.016)

    # A compact cleaver is connected to the right fist and never stretches
    # above the head, so the contact pose reads as a weapon swing rather than
    # a hovering black object in a dense column.
    weapon_y = -.10 + .30 * pose
    frustum("cleaver blade", (.62, weapon_y, 1.15 + bob), (.28,.14), (.21,.10), .54, IRON, bevel=.016)
    kite_plate("cleaver edge", (.62, weapon_y - .093, 1.30 + bob), .31, .35, .032, RAIDPLATE, bevel=.006)
    cube("cleaver notch", (.62, weapon_y - .147, 1.42 + bob), (.12, .026, .10), RAIDPLATE, .006)
    cube("cleaver grip", (.62, weapon_y, .81 + bob), (.09, .10, .24), HIDE, .018)
    sphere("cleaver pommel", (.62, weapon_y, .66 + bob), (.082, .082, .082), BRONZE)
    export("Brute" + ("_pose" if pose else ""))


def gnoll(pose=0.0):
    """A low, lean packrunner for Campaign's actual Swarm trait.

    This is intentionally not a reskinned upright warrior and not the old
    pale imported Wolf.  The long ribcage, lifted haunches, four articulated
    legs, muzzle, ears and loose harness make a fast scavenger recognisable in
    a dense tide before the player reads a tooltip. Both fixed-contact poses
    contain the same source objects in the same order so the bake carries a
    genuine run cycle all the way into the browser vertex-delta stream.
    """
    clear()
    bob = .030 * pose
    # Four legs run in diagonal pairs.  The staged knee/foot points create a
    # real bound instead of making an animated source object slide as one
    # rigid wolf-shaped mesh.
    leg_data = [(-.245, -.29, 0.0), (.245, -.29, 3.14159),
                (-.245, .31, 3.14159), (.245, .31, 0.0)]
    for i, (x, y, phase) in enumerate(leg_data):
        stride = .19 * pose * (1.0 if phase == 0.0 else -1.0)
        hip = (x, y, .70 + bob)
        knee = (x * .92, y + stride * .36, .36 + (0.025 if i < 2 else 0.0))
        paw = (x * 1.06, y + stride, .105)
        limb("packrunner upper leg", hip, knee, .108, .078, FUR, 12)
        limb("packrunner lower leg", knee, paw, .078, .048, FUR_DARK, 10)
        # A small, low paw casts a contact shadow without turning this into a
        # row of tall black posts on the road.
        ico("packrunner paw", paw, (.105, .145, .055), HIDE, 460 + i, subdivisions=1)

    # Ribcage and haunches deliberately overlap as separate organic masses.
    # The body reads from the fixed overview as a running animal, while the
    # close zoom can still see a shoulder/hip break and the bristled mane.
    ico("lean brindled ribcage", (0, -.045, .80 + bob), (.38,.58,.29), FUR, 481, subdivisions=2)
    ico("raised packrunner haunch", (0, .37, .84 + bob), (.40,.36,.31), FUR_DARK, 482, subdivisions=2)
    limb("packrunner neck", (0, -.34, .95 + bob), (0, -.58, 1.09 + bob), .165, .125, FUR_DARK, 12)
    ico("long gnoll head", (0, -.68, 1.12 + bob), (.255,.31,.215), FUR, 483, subdivisions=2)
    frustum("dark muzzle", (0, -.93, 1.045 + bob), (.28,.20), (.20,.15), .22, HIDE, bevel=.015)
    ico("black nose", (0, -1.045, 1.07 + bob), (.105,.060,.070), FUR_DARK, 484, subdivisions=1)
    for x in (-.135, .135):
        cone("alert packrunner ear", (x, -.63, 1.34 + bob), .085, .25, FUR_DARK,
             (0, radians(-12 if x < 0 else 12), 0), 6)
        sphere("amber packrunner eye", (x * .86, -.915, 1.17 + bob), (.036,.024,.036), GLOW)

    # A short ridge keeps the silhouette rough and natural without a giant
    # fantasy mohawk that would turn a hundred bodies into another fence.
    for i in range(5):
        y = -.12 + i * .18
        cone("bristled back mane", (0, y, 1.085 + bob + (i % 2) * .025), .052, .16,
             FUR_DARK, (radians(8), 0, 0), 5)

    # A worn two-piece harness makes this a raider rather than generic fauna.
    # The plates are small and separate from fur, so their physical response
    # stays localized in a mass horde.
    cube("packrunner chest strap", (0, -.36, .79 + bob), (.58,.10,.12), HIDE, .014)
    for x in (-.18, .18):
        kite_plate("packrunner harness plate", (x, -.43, .84 + bob), .17, .20, .040,
                   RAIDPLATE, bevel=.008)
    cyl("packrunner bronze tag", (0, -.47, .76 + bob), .042, .050, BRONZE,
        (radians(90), 0, 0), 10)
    # Tail provides a clear rear cue through crowded packs and moves slightly
    # with the authored run pose rather than remaining a static rod.
    limb("packrunner tail", (0, .62, .88 + bob), (.04, .98 + .12 * pose, .77 + bob),
         .075, .020, FUR_DARK, 10)
    export("Gnoll" + ("_pose" if pose else ""))

def crown_lump(name, p, scale, mat, seed):
    """A full, irregular clump of leaves with real volume and cast shadow.

    This intentionally uses a varied set of compact volumes rather than one
    smooth crown or a stack of flat discs.  At overview distance the overlap
    becomes a woodland silhouette; at a zoom it still resolves as separate
    branches and leaf masses, all from baked mesh geometry.
    """
    return ico(name, p, scale, mat, seed, subdivisions=2, smooth_surface=True)


def leaf_spray(name, centre, sx, sy, sz, count, materials, seed, needle=False):
    """A deterministic cluster of individual solid leaves or needles.

    Foliage volumes are useful deep inside a tree, but large rounded volumes
    at every branch tip are precisely what made the prior live map look like a
    field of lollipops.  This is intentionally mesh-heavy in the visible
    silhouette: each piece has a top, underside and edge, so it catches light
    and casts a real shadow instead of being a textured cloud.
    """
    for i in range(count):
        if needle:
            a = seed * .733 + i * 2.3999632
            radial = .18 + .76 * (.5 + .5 * sin(seed * 2.17 + i * 1.91))
            x = centre[0] + cos(a) * sx * radial
            y = centre[1] + sin(a) * sy * radial
            z = centre[2] + sin(seed * .91 + i * 2.63) * sz * (.32 + radial * .68)
        else:
            # Fill an irregular leaf *volume*, not only a thin shell around
            # it. The previous scatter was technically many solid leaves but
            # left a bare branch rack in the live browser. Golden-angle
            # placement keeps the clusters dense without radial spoke seams
            # or a smooth proxy sphere.
            a = seed * .733 + i * 2.3999632
            radial = (((i * .61803398875 + seed * .173) % 1.0) ** .5) * .97
            vertical = ((i * .41421356237 + seed * .119) % 1.0) - .5
            x = centre[0] + cos(a) * sx * radial
            y = centre[1] + sin(a) * sy * radial
            z = centre[2] + vertical * sz * (.92 - radial * .20)
        # Broadleaf tips must read as short, overlapping leaf bodies rather
        # than the long diamond lattice that the live camera turned into bare
        # twigs. Needles retain their slender construction; broad leaves get a
        # near-oval proportion and therefore make their own canopy mass.
        length = (.17 if needle else .13) + (.028 if needle else .022) * (i % 4)
        width = (.020 if needle else .084) + (.005 if needle else .014) * (i % 3)
        leaf(
            name,
            (x, y, z),
            length,
            width,
            materials[(i * 3 + int(seed * 7.0)) % len(materials)],
            a + .42 * sin(i * 1.31 + seed),
            (-28 if i % 4 == 0 else 18 if i % 4 == 1 else 43 if i % 4 == 2 else -8),
        )


def oak(name, needle=False):
    clear()
    # Each source name gets a different phase *and* a different crown layout.
    # Runtime rotation alone cannot stop cloned trees reading like a map icon
    # when a wide camera shows dozens of them at once.
    phase = {
        "NatureOak": .17,
        "NatureBroadleaf": .91,
        "NaturePineA": 1.47,
        "NaturePineB": 2.29,
    }.get(name, .17)

    # Curved, tapering trunk and roots: a live tree starts as a branching
    # structure, not a green mass balanced on a cylindrical pole.  The trunk
    # deliberately forks *inside* the lower third of the crown.  The previous
    # asset kept its trunk bare almost to the top, which made even a dense leaf
    # treatment read as a green hat balanced on a post in the live browser.
    trunk = [
        ((0.00, 0.00, .03), (.035, -.020, .48), .245, .196),
        ((.035, -.020, .48), (-.045, .035, .93), .196, .130),
        ((-.045, .035, .93), (.035, -.015, 1.55), .130, .060),
    ]
    for i, (a, b, ra, rb) in enumerate(trunk):
        limb("weathered trunk %d" % i, a, b, ra, rb, BARK, 16)
    for i in range(6):
        a = phase + i * 1.0472
        root_a = (cos(a) * .055, sin(a) * .055, .095)
        root_b = (cos(a) * (.34 + .045 * (i % 2)), sin(a) * (.34 + .045 * (i % 2)), .035)
        limb("root buttress", root_a, root_b, .082, .018, BARK, 9)

    if needle:
        # A conifer is a trunk supporting irregular drooping boughs.  The
        # former nested green cones and the first replacement's round bough
        # volumes both read as toy Christmas trees.  These five uneven whorls
        # carry visible timber and *individual solid needle sprays* instead.
        needles = (PINE_DEEP, PINE_LEAF, PINE_SUN)
        levels = [
            (1.02, .88, 6), (1.32, .75, 5), (1.64, .63, 5),
            (1.94, .48, 4), (2.22, .31, 3),
        ]
        for level, (z, reach, arms) in enumerate(levels):
            base_phase = phase + level * .69
            for arm in range(arms):
                a = base_phase + arm * 6.2831853 / arms
                # The point and needle pads droop differently on every arm.
                reach_arm = reach * (.78 + .11 * sin(arm * 2.17 + phase))
                start = (.006 * sin(a), .006 * cos(a), z + .015)
                tip = (cos(a) * reach_arm, sin(a) * reach_arm, z - .12 - .035 * (arm % 2))
                limb("pine bough", start, tip, .048 - level * .004, .014, WOOD, 9)
                mid = (cos(a) * reach_arm * .54, sin(a) * reach_arm * .54, z - .030)
                leaf_spray(
                    "pine needle", mid,
                    reach_arm * .56, reach_arm * .31, .14 + .018 * (arm % 3),
                    # Pine is a peripheral asset, not a 700-body character.
                    # Five-to-six solid needle sprays preserve its bough
                    # silhouette without leaving a 21k-triangle mesh in the
                    # universal runtime library.
                    5 + (arm + level) % 2, needles, 610 + level * 19 + arm + phase, True,
                )
                # The tip points irregularly beyond the branch rather than
                # forming a clean cone perimeter.
                outer = (cos(a) * reach_arm * .92, sin(a) * reach_arm * .92, z - .11)
                leaf_spray(
                    "pine tip needle", outer,
                    reach_arm * .24, reach_arm * .16, .10,
                    2 + arm % 2, needles, 730 + level * 23 + arm + phase, True,
                )
        leaf_spray("pine crown needle", (.01, -.02, 2.38), .22, .18, .24, 34, needles, 811 + phase, True)
    else:
        # Broadleaf canopy: a mature tree reads as a *volume of many joined
        # branch-end masses*, not a thin rack of cards and not one green ball.
        # Each lobe below is a solid, irregular leaf mesh with a top, underside
        # and edge; the smaller true leaves only break its outline.  This keeps
        # real geometry and shadows at whole-board distance while making the
        # trunk visibly disappear into a broad, grounded crown.
        leaves = (LEAF_DEEP, LEAF, LEAF_SUN)
        boughs = [
            ((-.035, .025, .78), (-.88, -.26, 1.20), .120, .032),
            ((-.035, .025, .86), ( .78, -.35, 1.30), .112, .031),
            ((-.020, .015, 1.00), (-.58,  .52, 1.48), .092, .026),
            (( .000, .005, 1.14), ( .54,  .48, 1.62), .080, .023),
            (( .020,-.010, 1.31), (-.28, -.08, 1.98), .062, .018),
        ]
        for i, (a, b, ra, rb) in enumerate(boughs):
            limb("forked crown bough %d" % i, a, b, ra, rb, BARK, 12)

        if name == "NatureOak":
            lumps = [
                (-.82, -.22, 1.22, .48, .42, .30),
                (-.46, -.43, 1.34, .52, .42, .34),
                ( .03, -.45, 1.37, .54, .44, .35),
                ( .53, -.25, 1.42, .50, .41, .34),
                ( .86,  .08, 1.49, .42, .37, .31),
                (-.73,  .22, 1.49, .49, .41, .34),
                (-.31,  .36, 1.63, .56, .45, .38),
                ( .22,  .34, 1.69, .55, .45, .38),
                ( .63,  .28, 1.75, .43, .37, .33),
                (-.45, -.03, 1.88, .47, .39, .35),
                ( .03, -.08, 1.98, .55, .43, .40),
                ( .43,  .04, 1.97, .44, .37, .35),
                (-.18,  .18, 2.20, .42, .35, .34),
                ( .18,  .13, 2.25, .36, .31, .31),
            ]
        else:
            # The second broadleaf has a lower, wind-bent crown so a grove
            # remains a woodland rather than several rotated copies of one
            # hero tree.
            lumps = [
                (-.86, -.10, 1.16, .45, .39, .30),
                (-.54, -.39, 1.27, .50, .42, .33),
                (-.04, -.47, 1.34, .56, .45, .36),
                ( .48, -.32, 1.43, .55, .44, .35),
                ( .83, -.01, 1.53, .43, .37, .32),
                (-.69,  .27, 1.43, .52, .42, .34),
                (-.24,  .40, 1.55, .57, .46, .38),
                ( .28,  .36, 1.67, .53, .44, .38),
                ( .67,  .25, 1.76, .42, .36, .34),
                (-.39,  .03, 1.82, .50, .40, .37),
                ( .10, -.06, 1.94, .57, .45, .41),
                ( .50,  .04, 1.96, .41, .35, .34),
                (-.11,  .18, 2.17, .45, .37, .36),
                ( .26,  .16, 2.26, .36, .30, .32),
            ]
        # A wide-screen woodland can show many full trees at once. Keep the
        # smoothed physical crown within the real 8k per-instance ceiling by
        # omitting three redundant middle lobes rather than degrading every
        # remaining lobe back into a faceted primitive. The retained positions
        # cover both shoulders, the deep centre and the high crown.
        lumps = [lumps[i] for i in (0, 2, 3, 5, 6, 7, 9, 10, 11, 12, 13)]
        for i, (x, y, z, sx, sy, sz) in enumerate(lumps):
            root = boughs[i % len(boughs)][1]
            tip = (x * .78 + root[0] * .22, y * .78 + root[1] * .22, z - .08)
            limb("leafy twig", root, tip, .031, .010, BARK, 8)
            mat = leaves[(i * 5 + int(phase * 11.0)) % len(leaves)]
            leaf_cluster("solid leaf crown lobe", (x, y, z), sx, sy, sz, mat, 630 + i * 23 + phase)
            # A few solid leaves around each real lobe keep its contour from
            # resolving into a repeated primitive at a close zoom. They are a
            # silhouette detail, not a replacement for the opaque foliage
            # volume above, so they stay well within the wave-safe mesh budget.
            leaf_spray(
                "crown contour leaf", (x, y, z), sx * 1.04, sy * 1.03, sz * .82,
                6 + i % 3, leaves, 730 + i * 19 + phase,
            )
    export(name)

def rock(name):
    clear()
    # Preserve broken faces on fieldstone. Smooth shading is useful on bark
    # and leaf volumes, but it turned this low, irregular rock into a pale
    # egg in the live overview.
    ico("weathered boulder",(0,0,.30),(.72,.56,.44),FIELDSTONE, 71, smooth_surface=False)
    ico("broken flank",(-.22,.18,.26),(.36,.29,.27),FIELDSTONE, 72, smooth_surface=False)
    # Raised, material-separated moss shelves catch their own light and shadow
    # instead of hiding beneath the boulder surface as the earlier small caps
    # did after normalisation.
    for i, (x,y,z,sx,sy) in enumerate([
        (-.24,.16,.72,.29,.22), (.27,-.12,.68,.24,.18), (.04,.31,.71,.23,.18),
    ]):
        ico("moss cap",(x,y,z),(sx,sy,.080),MOSS, 80+i, subdivisions=1)
    export(name)

def bush():
    clear()
    # A bush cannot be seven perfect green balls: that was the second most
    # conspicuous toy prop after the old trees.  Short woody stems and uneven
    # leaf clumps give this low cover a grounded, non-spherical silhouette.
    for i, (x, y, z) in enumerate([
        (-.36, -.10, .27), (-.18, .20, .34), (.12, -.23, .31),
        (.34, .09, .28), (.08, .27, .42), (-.18, -.30, .35),
        (.02, .00, .53),
    ]):
        limb("bush stem", (0.0, 0.0, .04), (x * .72, y * .72, z - .12), .042, .012, WOOD, 8)
        leaf_spray(
            "bush leaf", (x, y, z),
            .20 + .025 * (i % 3), .17 + .020 * ((i + 1) % 3), .13 + .018 * (i % 2),
            18 + i % 5, (LEAF_DEEP, BUSH, LEAF), 101 + i,
        )
    export("NatureBush")

def grass_blade(name, root, angle, length, width, bend, mat):
    """One narrow, bent and genuinely solid grass blade.

    The former cone bundle had mathematically valid volume, but its four-sided
    triangular silhouettes rendered as a line of black thorns along the road.
    A live meadow blade has a rooted base, a broad middle and a soft bent tip;
    this small four-section leaf gives it all three while retaining top,
    underside and edge faces for real shadow and lighting.
    """
    forward = (cos(angle), sin(angle))
    side = (-forward[1], forward[0])
    # A low, leaning blade reads as meadow growth from an oblique tactical
    # camera.  The prior geometry rose almost vertically with almost no
    # horizontal travel, which made every repeated clump render as a black
    # thorn fence along the road.
    sections = ((0.0, .34, .025), (.31, 1.0, .30), (.70, .68, .57), (1.0, .06, .74))
    top = []
    for t, taper, rise in sections:
        # The tip leans outward after the middle instead of continuing as a
        # rigid spear. A tiny sideways sine stops every blade sharing a plane.
        reach = length * (t * .78 + bend * .20 * sin(t * 1.5708))
        sway = width * .22 * sin(t * 3.14159 + angle * 1.7)
        cx = root[0] + forward[0] * reach + side[0] * sway
        cy = root[1] + forward[1] * reach + side[1] * sway
        z = length * rise
        half = width * taper
        top.extend(((cx + side[0] * half, cy + side[1] * half, z),
                    (cx - side[0] * half, cy - side[1] * half, z)))
    thick = .010
    verts = top + [(x, y, z - thick) for x, y, z in top]
    faces = []
    count = len(sections)
    for i in range(count - 1):
        a, b, c, d = i * 2, i * 2 + 1, i * 2 + 3, i * 2 + 2
        faces.extend(((a, b, c, d), (a + count * 2, d + count * 2,
                      c + count * 2, b + count * 2),
                      (a, d, d + count * 2, a + count * 2),
                      (b, b + count * 2, c + count * 2, c)))
    faces.append((0, 1, count * 2 + 1, count * 2))
    tip = (count - 1) * 2
    faces.append((tip, count * 2 + tip, count * 2 + tip + 1, tip + 1))
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces); mesh.update()
    o = bpy.data.objects.new(name, mesh); bpy.context.collection.objects.link(o)
    return assign(o, mat)


def grass():
    clear()
    # An irregular rooted patch, not a radial star or a row of cone spikes.
    # Keep the blade count modest so one visible clump remains meadow detail
    # and never becomes a spiny silhouette fence when repeated beside a route.
    for i in range(18):
        a = i * 2.39996 + (i % 5) * .19
        r = .025 + ((i * 7) % 17) * .021
        h = .115 + ((i * 11) % 8) * .014
        root = (sin(a) * r, cos(a * 1.37) * r)
        grass_blade(
            "curved grass blade", root, a + .31 * sin(i * 1.71), h,
            .019 + (i % 4) * .003,
            .16 + (i % 5) * .030,
            VERGE if i % 4 else LEAF,
        )
    export("NatureGrass")

def fern_leaf(name, angle, length, width, rise, mat=FERN):
    """One solid, tapered fern frond with top, underside and edge faces.

    A crossed billboard cannot survive the fixed, near-top-down camera. This
    shallow prism gives each leaflet a real catching surface and an underside
    in shadow, so a ground-cover cluster reads as plant structure rather than
    a field of coloured spikes.
    """
    forward = (cos(angle), sin(angle))
    side = (-forward[1], forward[0])
    sections = ((0.0, .16, .09), (.52, 1.0, .15 + rise * .48), (1.0, .14, .15 + rise))
    verts = []
    for t, taper, z in sections:
        x, y = forward[0] * length * t, forward[1] * length * t
        half = width * taper
        verts.extend(((x + side[0] * half, y + side[1] * half, z),
                      (x - side[0] * half, y - side[1] * half, z)))
    # A thin solid is still cheap, but it avoids disappearing when a frond is
    # seen from the low side of the tactical light or in its own shadow.
    verts.extend((x, y, z - .018) for x, y, z in verts[:])
    faces = []
    for i in range(2):
        a, b, c, d = i * 2, i * 2 + 1, i * 2 + 3, i * 2 + 2
        faces.extend(((a, b, c, d), (a + 6, d + 6, c + 6, b + 6),
                      (a, d, d + 6, a + 6), (b, b + 6, c + 6, c)))
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces); mesh.update()
    o = bpy.data.objects.new(name, mesh); bpy.context.collection.objects.link(o)
    return assign(o, mat)

def fern():
    clear()
    # A real fern is asymmetric from its root and grows towards a light gap;
    # twelve evenly radiating broad fronds looked like a bright green star in
    # the supplied live capture.  Two uneven arcs retain physical leaf faces
    # and shadows, but their silhouette reads as understory rather than a UI
    # marker stamped around the route.
    angles = (-1.34, -.98, -.61, -.18, .24, .67, 1.08, 1.52)
    for i, offset in enumerate(angles):
        a = offset + .18 * sin(i * 2.71)
        fern_leaf(
            "fern frond", a,
            .30 + ((i * 7) % 6) * .040,
            .046 + (i % 3) * .011,
            .032 + ((i * 5) % 4) * .016,
            LEAF_DEEP if i % 3 == 0 else FERN,
        )
    cyl("fern heart", (0, 0, .10), .075, .12, FERN, verts=10)
    export("NatureFern")

def stump():
    clear()
    cyl("cut oak", (0, 0, .24), .34, .48, WOOD, verts=18)
    # The pale cut face and three low roots stop this from reading as another
    # generic brown cylinder when the fixed camera looks down on it.
    cyl("cut face", (0, 0, .485), .285, .018, BONE, verts=18)
    for a in (0.15, 2.32, 4.48):
        cyl("root", (sin(a)*.29, sin(a)*.29, .13), .065, .46, WOOD,
            (radians(72), 0, a), verts=10)
    export("NatureStump")

for s in range(4):
    tower_seed(s); tower_seed(s, 1.0)
    tower_siege(s); tower_siege(s, 1.0)
    tower_bounce(s); tower_bounce(s, 1.0)
    tower_multi(s); tower_multi(s, 1.0)
    tower_corrupt(s); tower_corrupt(s, 1.0)
    tower_air(s); tower_air(s, 1.0)
    tower_chaos(s); tower_chaos(s, 1.0)
    tower_destroy(s); tower_destroy(s, 1.0)
    tower_aura(s); tower_aura(s, 1.0)
    tower_demon(s); tower_demon(s, 1.0)
    tower_king(s); tower_king(s, 1.0)
humanoid(); humanoid(1.0); beetle(); beetle(1.0); gnoll(); gnoll(1.0)
oak("NatureOak"); oak("NatureBroadleaf"); oak("NaturePineA", True); oak("NaturePineB", True)
rock("NatureRockA"); rock("NatureRockB"); bush(); grass(); fern(); stump()
print("wrote original meshes to", OUT)
