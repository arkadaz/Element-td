"""Emits src/game/greentd_map.rs: the map's own terrain and the player's lane.

Green Circle TD is not a circle. It is a ninety-six by ninety-six field cut
into eight identical arenas by three-tile corridors, with a spawn box in each
corner and on each edge. The corridors are painted in the rock texture
(`Arck`), the buildable ground in grass and dirt, and that texture grid *is*
the level - the pathing map is uniformly walkable, so nothing else describes it.

This reads `war3map.w3e`, bakes the texture grid, and traces the route the Red
player's creeps actually walk out of the move orders in `war3map.j`.
"""

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
# Read the corridor widths where nothing crosses them, so a junction does not
# make a three-tile passage look sixty tiles wide, and take the turning points
# from the map's own regions.
RUN_Y = centre_of_corridor(JUNCTION[0] - 8, JUNCTION[1], 'y')
COL_X = centre_of_corridor(FAR[0], FAR[1] + 12, 'x')
WEST_X = float(ENTRY[0])
FAR_Y = float(FAR[1])

# Half the corridor's width, so the two directions do not overlap. Creeps pass
# each other in a three-tile passage, which is exactly what this looks like in
# Warcraft III.
LANE = 0.75

LAP = [
    (WEST_X, RUN_Y - LANE),
    (COL_X - LANE, RUN_Y - LANE),
    (COL_X - LANE, FAR_Y),
    (COL_X + LANE, FAR_Y),
    (COL_X + LANE, RUN_Y + LANE),
    (WEST_X, RUN_Y + LANE),
]

# What the camera frames: the lane, plus the ground within a tower's reach of
# it. Everything the player will ever look at, and nothing else.
LANE_X0 = min(x for x, _ in LAP)
LANE_X1 = max(x for x, _ in LAP)
LANE_Y0 = min(y for _, y in LAP)
LANE_Y1 = max(y for _, y in LAP)
VIEW = (
    LANE_X0 - 7.0,
    LANE_Y0 - 7.0,
    LANE_X1 + 8.0,
    LANE_Y1 + 8.0,
)

# ---------------------------------------------------------------- the arena
#
# The buildable field is exactly what the camera frames. A plot the player
# cannot see is a plot they cannot click, and this map is played from one fixed
# view rather than by scrolling around it.
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
