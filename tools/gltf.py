"""Read a .glb and flatten it to coloured triangles.

The game has no texture path and no skinning, and these models need neither: the
CC0 packs it uses are flat-shaded, so every material is a `baseColorFactor` and
the colour can be baked per vertex. What comes out is one triangle soup per
model, in the game's own axes, which `bake_models.py` writes into a blob the
Rust side reads.

glTF is JSON plus one binary chunk, so there is no dependency to add for this.
"""

import json
import math
import struct

# Accessor component types, and how many bytes each takes.
CTYPE = {5120: ('b', 1), 5121: ('B', 1), 5122: ('h', 2),
         5123: ('H', 2), 5125: ('I', 4), 5126: ('f', 4)}
NCOMP = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4, 'MAT4': 16}


def read_glb(path):
    """The JSON chunk and the binary chunk of a .glb."""
    d = open(path, 'rb').read()
    magic, _ver, _len = struct.unpack('<III', d[:12])
    if magic != 0x46546C67:
        raise ValueError('%s is not a .glb' % path)
    off, js, bin_ = 12, None, b''
    while off < len(d):
        clen, _ = struct.unpack('<II', d[off:off + 8])
        tag = d[off + 4:off + 8]
        chunk = d[off + 8:off + 8 + clen]
        if tag == b'JSON':
            js = json.loads(chunk)
        elif tag[:3] == b'BIN':
            bin_ = chunk
        off += 8 + clen
    if js is None:
        raise ValueError('%s has no JSON chunk' % path)
    return js, bin_


def accessor(js, bin_, i):
    """One accessor, as a list of tuples."""
    a = js['accessors'][i]
    n = NCOMP[a['type']]
    fmt, size = CTYPE[a['componentType']]
    count = a['count']
    if 'bufferView' not in a:
        return [(0.0,) * n] * count
    bv = js['bufferViews'][a['bufferView']]
    start = bv.get('byteOffset', 0) + a.get('byteOffset', 0)
    stride = bv.get('byteStride') or size * n
    out = []
    for k in range(count):
        o = start + k * stride
        out.append(struct.unpack_from('<' + fmt * n, bin_, o))
    return out


def node_matrix(node):
    """A node's local transform, as a 4x4 in column-major glTF order."""
    if 'matrix' in node:
        return list(node['matrix'])
    t = node.get('translation', [0.0, 0.0, 0.0])
    r = node.get('rotation', [0.0, 0.0, 0.0, 1.0])
    s = node.get('scale', [1.0, 1.0, 1.0])
    x, y, z, w = r
    rm = [
        1 - 2 * (y * y + z * z), 2 * (x * y + z * w), 2 * (x * z - y * w), 0.0,
        2 * (x * y - z * w), 1 - 2 * (x * x + z * z), 2 * (y * z + x * w), 0.0,
        2 * (x * z + y * w), 2 * (y * z - x * w), 1 - 2 * (x * x + y * y), 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
    for c in range(3):
        for row in range(3):
            rm[c * 4 + row] *= s[c]
    rm[12], rm[13], rm[14] = t
    return rm


def mat_mul(a, b):
    """a * b, both column-major 4x4."""
    out = [0.0] * 16
    for c in range(4):
        for r in range(4):
            out[c * 4 + r] = sum(a[k * 4 + r] * b[c * 4 + k] for k in range(4))
    return out


def xform(m, p):
    return (
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    )


def xform_dir(m, p):
    return (
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2],
    )


def srgb_to_linear(c):
    """glTF base colours are already linear; this is here for the rare pack
    that stores sRGB and is left unused rather than guessed at."""
    return c


def inverse(m):
    """Inverse of a column-major 4x4 built from rotation, scale and position.

    General enough for glTF node matrices, which are affine - the last row is
    always (0, 0, 0, 1), so this inverts the 3x3 and re-derives the translation
    rather than running a full 4x4 elimination.
    """
    a = [m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]]
    det = (a[0] * (a[4] * a[8] - a[5] * a[7])
           - a[3] * (a[1] * a[8] - a[2] * a[7])
           + a[6] * (a[1] * a[5] - a[2] * a[4]))
    if abs(det) < 1e-12:
        return [1.0, 0, 0, 0, 0, 1.0, 0, 0, 0, 0, 1.0, 0, 0, 0, 0, 1.0]
    inv = [
        (a[4] * a[8] - a[5] * a[7]) / det,
        (a[2] * a[7] - a[1] * a[8]) / det,
        (a[1] * a[5] - a[2] * a[4]) / det,
        (a[5] * a[6] - a[3] * a[8]) / det,
        (a[0] * a[8] - a[2] * a[6]) / det,
        (a[2] * a[3] - a[0] * a[5]) / det,
        (a[3] * a[7] - a[4] * a[6]) / det,
        (a[1] * a[6] - a[0] * a[7]) / det,
        (a[0] * a[4] - a[1] * a[3]) / det,
    ]
    t = (m[12], m[13], m[14])
    tx = -(inv[0] * t[0] + inv[3] * t[1] + inv[6] * t[2])
    ty = -(inv[1] * t[0] + inv[4] * t[1] + inv[7] * t[2])
    tz = -(inv[2] * t[0] + inv[5] * t[1] + inv[8] * t[2])
    return [inv[0], inv[1], inv[2], 0.0,
            inv[3], inv[4], inv[5], 0.0,
            inv[6], inv[7], inv[8], 0.0,
            tx, ty, tz, 1.0]


def slerp(a, b, t):
    d = sum(x * y for x, y in zip(a, b))
    if d < 0.0:
        b = [-x for x in b]
        d = -d
    if d > 0.9995:
        out = [x + (y - x) * t for x, y in zip(a, b)]
    else:
        th = math.acos(max(-1.0, min(1.0, d)))
        st = math.sin(th)
        wa, wb = math.sin((1 - t) * th) / st, math.sin(t * th) / st
        out = [x * wa + y * wb for x, y in zip(a, b)]
    n = math.sqrt(sum(x * x for x in out)) or 1.0
    return [x / n for x in out]


def pick_animation(js, prefer=('Walk', 'Run', 'Idle')):
    """The index of the best clip to freeze, by name.

    These rigs carry a dozen clips - Death, Punch, Dance, Jump - and clip zero
    is whichever happened to sort first, which is why the first bake caught an
    orc mid-Duck and a skeleton mid-HitReact. Names are prefixed with the
    armature ("CharacterArmature|Walk"), so the match is on the tail.
    """
    anims = js.get('animations', [])
    for want in prefer:
        for i, a in enumerate(anims):
            tail = a.get('name', '').split('|')[-1].strip().lower()
            if tail == want.lower():
                return i
    for want in prefer:
        for i, a in enumerate(anims):
            if want.lower() in a.get('name', '').lower():
                return i
    return 0 if anims else None


def sample_animation(js, bin_, anim_index, time_frac):
    """Node TRS overrides for one animation, a fraction of the way through it.

    A bind pose is a T-pose, and a T-pose is not a thing you can put on a
    battlefield. Every one of these packs ships walk and idle clips, so the bake
    samples one and freezes it: the game gets a figure standing or striding
    rather than one holding its arms straight out.
    """
    anims = js.get('animations', [])
    if not anims or anim_index >= len(anims):
        return {}
    anim = anims[anim_index]
    # The clip's own length, so `time_frac` means the same thing in every file.
    end = 0.0
    for smp in anim['samplers']:
        ts = accessor(js, bin_, smp['input'])
        if ts:
            end = max(end, ts[-1][0])
    t = end * time_frac

    out = {}
    for ch in anim['channels']:
        node = ch['target'].get('node')
        path = ch['target']['path']
        if node is None or path == 'weights':
            continue
        smp = anim['samplers'][ch['sampler']]
        ts = [v[0] for v in accessor(js, bin_, smp['input'])]
        vs = accessor(js, bin_, smp['output'])
        if not ts:
            continue
        if t <= ts[0]:
            val, i, u = vs[0], 0, 0.0
        elif t >= ts[-1]:
            val, i, u = vs[-1], len(ts) - 1, 0.0
        else:
            i = max(k for k in range(len(ts)) if ts[k] <= t)
            span = ts[i + 1] - ts[i]
            u = (t - ts[i]) / span if span > 0 else 0.0
            if path == 'rotation':
                val = slerp(list(vs[i]), list(vs[i + 1]), u)
            else:
                val = [a + (b - a) * u for a, b in zip(vs[i], vs[i + 1])]
        out.setdefault(node, {})[path] = list(val)
    return out


def triangles(path, anim='auto', at=0.0):
    """Every triangle in the file, at rest pose, as (pos, nrm, rgb) vertices.

    Skinned meshes are taken at their bind pose. That is not the same as the
    animated pose and it is deliberate: the game has no skinning, and a bind
    pose standing still reads correctly while a half-applied skin does not.
    """
    js, bin_ = read_glb(path)
    nodes = js.get('nodes', [])
    if anim == 'auto':
        anim = pick_animation(js)
    pose = sample_animation(js, bin_, anim, at) if anim is not None else {}
    world = {}

    def walk(i, parent):
        node = dict(nodes[i])
        if i in pose:
            # An animated channel replaces that component of the node's TRS; a
            # `matrix` node cannot be animated per the spec, so dropping it here
            # is correct rather than lossy.
            node.pop('matrix', None)
            node.update(pose[i])
        m = mat_mul(parent, node_matrix(node))
        world[i] = m
        for c in nodes[i].get('children', []):
            walk(c, m)

    ident = [1.0, 0, 0, 0, 0, 1.0, 0, 0, 0, 0, 1.0, 0, 0, 0, 0, 1.0]
    scene = js.get('scenes', [{}])[js.get('scene', 0)]
    for root in scene.get('nodes', range(len(nodes))):
        walk(root, ident)

    mats = js.get('materials', [])
    verts = []
    for ni, node in enumerate(nodes):
        if 'mesh' not in node or ni not in world:
            continue
        m = world[ni]
        # A skinned mesh ignores its own node transform: its vertices are placed
        # entirely by the joint matrices below.
        skin = None
        if 'skin' in node:
            sk = js['skins'][node['skin']]
            joints = sk['joints']
            ibm = (accessor(js, bin_, sk['inverseBindMatrices'])
                   if 'inverseBindMatrices' in sk else
                   [(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1)] * len(joints))
            root_inv = inverse(m)
            skin = [mat_mul(root_inv, mat_mul(world.get(j, m), list(ibm[k])))
                    for k, j in enumerate(joints)]
        for prim in js['meshes'][node['mesh']]['primitives']:
            if prim.get('mode', 4) != 4:
                continue
            pos = accessor(js, bin_, prim['attributes']['POSITION'])
            nrm = (accessor(js, bin_, prim['attributes']['NORMAL'])
                   if 'NORMAL' in prim['attributes'] else None)
            jts = (accessor(js, bin_, prim['attributes']['JOINTS_0'])
                   if skin and 'JOINTS_0' in prim['attributes'] else None)
            wts = (accessor(js, bin_, prim['attributes']['WEIGHTS_0'])
                   if skin and 'WEIGHTS_0' in prim['attributes'] else None)
            if 'indices' in prim:
                idx = [i[0] for i in accessor(js, bin_, prim['indices'])]
            else:
                idx = list(range(len(pos)))
            col = (1.0, 1.0, 1.0)
            mi = prim.get('material')
            if mi is not None and mi < len(mats):
                pbr = mats[mi].get('pbrMetallicRoughness', {})
                bc = pbr.get('baseColorFactor', [1, 1, 1, 1])
                col = tuple(srgb_to_linear(c) for c in bc[:3])
            for k in idx:
                if jts and wts:
                    # Linear blend skinning, the same four-weight sum the GPU
                    # would do - done here once, at bake time, because the game
                    # draws seven hundred units and cannot afford to do it per
                    # frame.
                    acc = [0.0] * 16
                    total = 0.0
                    for c in range(4):
                        w = wts[k][c]
                        if w <= 0.0:
                            continue
                        jm = skin[jts[k][c]]
                        for e in range(16):
                            acc[e] += jm[e] * w
                        total += w
                    if total > 1e-6:
                        sm = mat_mul(m, [e / total for e in acc])
                    else:
                        sm = m
                else:
                    sm = m
                p = xform(sm, pos[k])
                if nrm:
                    n = xform_dir(sm, nrm[k])
                    ln = math.sqrt(sum(c * c for c in n)) or 1.0
                    n = tuple(c / ln for c in n)
                else:
                    n = (0.0, 0.0, 1.0)
                verts.append((p, n, col))
    return verts


if __name__ == '__main__':
    import sys
    for p in sys.argv[1:]:
        v = triangles(p, anim='auto', at=0.30)
        xs = [q[0][0] for q in v]
        ys = [q[0][1] for q in v]
        zs = [q[0][2] for q in v]
        cols = set(q[2] for q in v)
        print('%-28s %6d tris  bounds x[%.2f %.2f] y[%.2f %.2f] z[%.2f %.2f]  %d colours'
              % (p.rsplit('/', 1)[-1], len(v) // 3,
                 min(xs), max(xs), min(ys), max(ys), min(zs), max(zs), len(cols)))
