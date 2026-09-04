"""Emits src/game/greentd_map.rs: the map's own terrain and the player's lane.

Green Circle TD is not a circle. It is a ninety-six by ninety-six field cut
into eight identical arenas by three-tile corridors, with a spawn box in each
corner and on each edge. The corridors are painted in the rock texture
(`Arck`), the buildable ground in grass and dirt, and that texture grid *is*
the level - the pathing map is uniformly walkable, so nothing else describes it.

This reads `war3map.w3e`, bakes the texture grid, and traces the route the Red
player's creeps actually walk out of the move orders in `war3map.j`.
"""

import re
import struct

import mpq

MAP = '../GREEN TD 9.3c PEIN.w3x'

# Warcraft III tiles are 128 units across, and this map's origin is its corner.
TILE = 128.0
ORIGIN = -6144.0

# Ground texture indices, in the order `war3map.w3e` lists them.
DIRT, GRASS, ROCK, DARK = 0, 1, 2, 3


def read_terrain():
    a = mpq.Archive(MAP)
    d = a.read('war3map.w3e')
    o = 4 + 4 + 1 + 4
    ng, = struct.unpack_from('<I', d, o)
    o += 4 + ng * 4
    nc, = struct.unpack_from('<I', d, o)
    o += 4 + nc * 4
    w, h = struct.unpack_from('<II', d, o)
    o += 8
    ox, oy = struct.unpack_from('<ff', d, o)
    o += 8
    tex = bytearray(w * h)
    lvl = bytearray(w * h)
    for i in range(w * h):
        _gh, _wl, t, _det, cl = struct.unpack_from('<HHBBB', d, o + i * 7)
        tex[i] = t & 0x0F
        lvl[i] = cl & 0x0F
    return w, h, ox, oy, tex, lvl


W, H, OX, OY, TEX, LVL = read_terrain()


def road(x, y):
    return 0 <= x < W and 0 <= y < H and TEX[y * W + x] == ROCK


def to_tile(wx, wy):
    """World units to tile indices."""
    return (int(round((wx - OX) / TILE)), int(round((wy - OY) / TILE)))


# ---------------------------------------------------------------- the lane
#
# The Red player's creeps are ordered, region by region, along this route. The
# last two regions are a shuttle: enter the far one and you are sent back to the
# near one, forever. Nothing ever leaves.
#
#   rect01  the spawn box, top-left corner
#   rect02  the foot of the entry corridor
#   rect03  the junction where the entry meets the long east-west run
#   rect04  the far end of the north-south corridor
#
# Those four are read out of `war3map.j` rather than typed here, so a different
# map version moves the lane on its own.
RECTS = {
    'rect01': (-5824.0, 4928.0, -4928.0, 5824.0),
    'rect02': (-5600.0, 3616.0, -5248.0, 3872.0),
    'rect03': (-3968.0, 3744.0, -3744.0, 3936.0),
    'rect04': (-3936.0, -64.0, -3840.0, 64.0),
}


def load_rects():
    import re
    src = mpq.Archive(MAP).read('Scripts' + chr(92) + 'war3map.j').decode('latin-1')
    out = {}
    for m in re.finditer(r'set udg_(rect\d+)=Rect\(([-\d.]+),([-\d.]+),([-\d.]+),([-\d.]+)\)', src):
        out[m.group(1)] = tuple(float(m.group(i)) for i in range(2, 6))
    return out or RECTS


R = load_rects()


def centre(name):
    x0, y0, x1, y1 = R[name]
    return to_tile((x0 + x1) / 2.0, (y0 + y1) / 2.0)


SPAWN = centre('rect01')
ENTRY = centre('rect02')
JUNCTION = centre('rect03')
FAR = centre('rect04')


def corridor_span(x, y, axis):
    """How far the corridor through (x, y) runs along `axis`, as (lo, hi)."""
    lo = hi = (x if axis == 'x' else y)
    while True:
        p = (lo - 1, y) if axis == 'x' else (x, lo - 1)
        if not road(*p):
            break
        lo -= 1
    while True:
        p = (hi + 1, y) if axis == 'x' else (x, hi + 1)
        if not road(*p):
            break
        hi += 1
    return lo, hi


def centre_of_corridor(x, y, axis):
    lo, hi = corridor_span(x, y, axis)
    return (lo + hi) / 2.0


# The lap.
#
# This used to be three regions joined into a U by hand, and it was wrong in a
# way that made the whole game look strange: the board was a straight corridor
# across the top of an empty field, because a U through one corner of the map is
# what it is.
#
# The map is a *circle*, which is what it is called. `script.j` registers a
# trigger on every waypoint region and each one orders the creep to the next, so
# the route is in the map and can be read rather than guessed. Converted to
# tiles, the waypoints are two concentric square rings:
#
#   outer   corners (18, 78) (79, 78) (78, 18) (18, 18)
#           mid-edges Top (48, 79)  Right (79, 48)  Bottom (48, 18)  Left (18, 48)
#   inner   corners (26, 71) (71, 71) (71, 26) (26, 26)
#           mid-edges Top (48, 70)  Right (70, 48)  Bottom (48, 27)  Left (27, 48)
#
# Eight players sit around them - four at the map's corners feeding the outer
# ring, four in the middle feeding the inner one - and the mid-edges are where
# the two rings meet, which is how a creep crosses from one to the other.
#
# Single player takes the outer ring, as a closed loop. It is the largest of the
# two, it is the one the map's own Red player walks, and a loop is what makes
# this game what it is: nothing ever reaches an exit, so what you are defending
# is a *rate*.
WAYPOINT_RE = re.compile(r'udg_rect(\d+)=Rect\(([-0-9.]+),([-0-9.]+),([-0-9.]+),([-0-9.]+)\)')


def waypoints():
    """Every waypoint region's centre, in tiles, by its rect number."""
    # The script lives at Scripts\war3map.j inside the archive; there is a
    # stub at the root that reads back as nothing.
    a = mpq.Archive(MAP)
    raw = a.read('Scripts\war3map.j') or a.read('war3map.j')
    if not raw:
        raise SystemExit('the archive has no trigger script - the route cannot be read')
    text = raw.decode('latin-1', 'replace')
    out = {}
    for m in WAYPOINT_RE.finditer(text):
        n = int(m.group(1))
        x0, y0, x1, y1 = (float(g) for g in m.groups()[1:])
        # Same origin and tile size the terrain is read with, so a
        # waypoint and a tile mean the same thing.
        out[n] = (
            ((x0 + x1) * 0.5 - ORIGIN) / TILE,
            ((y0 + y1) * 0.5 - ORIGIN) / TILE,
        )
    return out


def outer_ring(wp):
    """The outer circuit, in order, as a closed loop of tile positions.

    The rect numbers are the map's, taken from which trigger is registered on
    which region: rect03 is OutsideTL, rect17 OutsideTop, and so on round.
    Pinned by number rather than found by geometry because the map is fixed and
    a wrong guess here is a route through a wall.
    """
    order = [3, 17, 16, 28, 27, 7, 6, 4]
    missing = [n for n in order if n not in wp]
    if missing:
        raise SystemExit('script.j has no rect %s - the route cannot be read' % missing)
    return [wp[n] for n in order]


WP = waypoints()
RING = outer_ring(WP)

# Creeps travel on one side of a three-tile corridor so the two directions do
# not overlap, exactly as they do in Warcraft III. The ring is walked
# anticlockwise, so pushing each leg towards the middle of the board puts the
# traffic on the inside kerb.
LANE = 0.75
CX = sum(x for x, _ in RING) / len(RING)
CY = sum(y for _, y in RING) / len(RING)


def kerb(p):
    x, y = p
    dx = LANE if x < CX else -LANE
    dy = LANE if y < CY else -LANE
    # Only the axis the corner turns on gets the offset on a mid-edge, so a
    # straight run stays straight.
    return (x + dx * 0.5, y + dy * 0.5)


LAP = [kerb(p) for p in RING]

# What the camera may look at, and what may be built on: the ring plus a tower's
# reach outside it, and the whole field inside. The player scrolls this; it is
# far larger than one screen, which is the point of a circuit.
LANE_X0 = min(x for x, _ in LAP)
LANE_X1 = max(x for x, _ in LAP)
LANE_Y0 = min(y for _, y in LAP)
LANE_Y1 = max(y for _, y in LAP)
VIEW = (
    LANE_X0 - 8.0,
    LANE_Y0 - 8.0,
    LANE_X1 + 8.0,
    LANE_Y1 + 8.0,
)

# ---------------------------------------------------------------- the arena
#
# Everything inside the framed rectangle that is not corridor. On this map that
# is the whole field the ring encloses plus the margin outside it.
ARENA = VIEW


def emit():
    out = []
    w = out.append
    w('//! The Green Circle TD terrain, and the lane one player defends.')
    w('//!')
    w('//! **Generated from `GREEN TD 9.3c PEIN.w3x` by `tools/emit_map.py` - do')
    w('//! not hand-edit.**')
    w('//!')
    w('//! `TEXTURE` is the map\'s own ground-texture grid, one byte a tile, and')
    w('//! it is the level: the corridors are painted in rock and everything else')
    w('//! is grass or dirt. The pathing map says the whole field is walkable, so')
    w('//! the texture is the only thing that describes the maze.')
    w('')
    w('/// Tiles across and down. The map is 96x96 tiles of 128 world units.')
    w('pub const MAP_W: usize = %d;' % W)
    w('pub const MAP_H: usize = %d;' % H)
    w('')
    w('/// Ground texture per tile: 0 dirt, 1 grass, **2 the corridors**, 3 dark')
    w('/// grass. Row-major, row 0 at the south edge, exactly as the map stores it.')
    w('pub static TEXTURE: &[u8; MAP_W * MAP_H] = &[')
    for y in range(H):
        row = ''.join('%d,' % TEX[y * W + x] for x in range(W))
        w('    ' + row)
    w('];')
    w('')
    w('/// Cliff level per tile. Flat everywhere but two corners in this map, and')
    w('/// kept so a raised arena would come through without a code change.')
    w('pub static LEVEL: &[u8; MAP_W * MAP_H] = &[')
    for y in range(H):
        w('    ' + ''.join('%d,' % LVL[y * W + x] for x in range(W)))
    w('];')
    w('')
    w('/// Where the Red player\'s creeps come from, in tiles.')
    w('pub const SPAWN_TILE: [f32; 2] = [%.1f, %.1f];' % SPAWN)
    w('')
    w('/// The lap, in tiles.')
    w('///')
    w('/// The map orders Red\'s creeps along four regions and then shuttles them')
    w('/// between the last two forever, so the "lap" is a corridor walked down')
    w('/// and back. Written as a closed loop down one half of the corridor and')
    w('/// up the other, which is how two streams pass in a three-tile passage.')
    w('pub static LAP: &[[f32; 2]] = &[')
    for x, y in LAP:
        w('    [%.2f, %.2f],' % (x, y))
    w('];')
    w('')
    w('/// The quarter of the map this player defends: min x, min y, max x, max y,')
    w('/// in tiles. The rest of the field belongs to the other seven players and')
    w('/// is drawn but never built on.')
    w('pub const ARENA: [f32; 4] = [%.1f, %.1f, %.1f, %.1f];' % ARENA)
    w('')
    w('/// What the camera frames: the lane and the ground a tower can reach from')
    w('/// it. Tighter than the arena, because Warcraft III sits about twenty-five')
    w('/// tiles from edge to edge and the whole arena is sixty.')
    w('pub const VIEW: [f32; 4] = [%.1f, %.1f, %.1f, %.1f];' % VIEW)
    open('../src/game/greentd_map.rs', 'w', newline='\n').write('\n'.join(out) + '\n')
    print('map: %dx%d tiles, %d corridor tiles' % (W, H, sum(1 for t in TEX if t == ROCK)))
    print('spawn %s  entry %s  junction %s  far %s' % (SPAWN, ENTRY, JUNCTION, FAR))
    print('lap %s' % (LAP,))
    print('arena %s' % (ARENA,))


if __name__ == '__main__':
    emit()
