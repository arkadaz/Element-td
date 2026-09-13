"""Download the CC0 ground textures and bake them into one raw blob.

    python bake_textures.py

Writes `assets/textures.bin`: a small array of square RGBA layers, raw, with no
container. Raw because the game would otherwise need a PNG or JPEG decoder in
Rust to read them, and a decoder is a dependency and a failure mode for
something that is decided entirely at bake time. The blob is what the GPU wants
anyway.

Sources are ambientCG, whose whole library is CC0. Their API is public and the
zips are direct downloads, so this is reproducible without an account.
"""

import io
import os
import struct
import subprocess
import zipfile

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
CACHE = os.path.join(HERE, 'tex')
OUT = os.path.abspath(os.path.join(HERE, '..', 'assets'))

# Every layer is this square. 512 is detailed enough for a close-up and right for
# what this is: ground seen from a camera twenty-six tiles up, tiled once per
# tile. 512 keeps bark, masonry, grass blades and worn road aggregate visible
# at the close camera while the shared eight-layer array remains inexpensive
# enough for integrated GPUs and Chromium/WebGPU.
SIZE = 512

# The layer order is the contract with `view::mod`, which picks a layer per
# tile. Adding one means adding it here and using it there; nothing else.
LAYERS = [
    ('grass', 'Grass004'),
    # The route needs the same compact, dark soil and moss aggregate used by
    # the actual map studies—not the old warm orange generic ground tile.
    # It remains a distinct physical earth layer with its own normal response;
    # grass still grows into its irregular edge through real mesh cover.
    ('dirt', 'LOCAL:mosswatch-ground-v1.png'),
    # The HUD's own materials. They live in the same blob as the ground because
    # they are the same thing - a normalised albedo tinted by a colour the game
    # already chose - and one blob is one loader, one failure mode and one set
    # of credits.
    ('wood', 'Planks037A'),
    ('stone', 'Rock051'),
    # Original project asset generated for Mosswatch. `normal_layer` derives a
    # deliberately restrained local normal from its high-frequency detail.
    ('mosswatch', 'LOCAL:mosswatch-grass-albedo-v2.png'),
]

MAGIC = 0x58455447  # "GTEX"


def fetch(asset):
    os.makedirs(CACHE, exist_ok=True)
    zpath = os.path.join(CACHE, '%s.zip' % asset)
    if not os.path.exists(zpath) or os.path.getsize(zpath) == 0:
        url = 'https://ambientcg.com/get?file=%s_1K-JPG.zip' % asset
        subprocess.run(['curl', '-sL', '--max-time', '180', '-o', zpath, url],
                       check=True)
    return zpath


def colour_layer(asset):
    """The albedo map, square, RGBA, normalised to a linear mean of one half.

    Normalising is the whole trick. A photographic albedo carries the material's
    *brightness* as well as its grain, and the game already has a brightness for
    each surface - a palette measured against a Warcraft III screenshot. Sampling
    the raw texture threw that away: the dirt corridors came out pale grey
    because ambientCG's ground albedo averages rgb(153, 147, 88) and the map's
    trodden earth is far darker than that.

    So each layer is rescaled until its linear average is 0.5. The shader then
    multiplies by twice the sample, which averages to exactly one and leaves the
    measured colour intact - the texture supplies variation and nothing else.
    """
    if asset.startswith('LOCAL:'):
        im = Image.open(os.path.join(OUT, asset[6:])).convert('RGB')
    else:
        z = zipfile.ZipFile(fetch(asset))
        name = next(n for n in z.namelist()
                    if n.lower().endswith('_color.jpg'))
        im = Image.open(io.BytesIO(z.read(name))).convert('RGB')
    im = im.resize((SIZE, SIZE), Image.LANCZOS)

    px = list(im.getdata())
    n = float(len(px))

    def to_lin(v):
        v /= 255.0
        return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4

    def to_srgb(v):
        v = max(0.0, min(1.0, v))
        v = v * 12.92 if v <= 0.0031308 else 1.055 * (v ** (1 / 2.4)) - 0.055
        return int(round(v * 255.0))

    lin = [tuple(to_lin(c) for c in p) for p in px]
    # One scale for all three channels, so normalising cannot shift the hue.
    mean = sum((p[0] + p[1] + p[2]) / 3.0 for p in lin) / n
    k = 0.5 / max(mean, 1e-4)
    im.putdata([tuple(to_srgb(c * k) for c in p) for p in lin])
    return im


def normal_layer(asset):
    """The tangent-space normal map, square, at `SIZE`.

    These zips have been downloaded all along and only the colour was kept,
    which is most of why the ground read as painted card: an albedo tells the
    light what colour a surface is and nothing at all about its shape. The
    normal map is the shape.

    `NormalGL` rather than `NormalDX` - the green channel points up in OpenGL
    convention, which is what the shader below assumes.
    """
    if asset.startswith('LOCAL:'):
        # Generate restrained tangent-space relief from the authored albedo's
        # aggregate/moss variation. It is intentionally low strength: this is
        # material micro-relief, not a fake sculpt or directional shading.
        source = Image.open(os.path.join(OUT, asset[6:])).convert('L').resize((SIZE, SIZE), Image.LANCZOS)
        src = source.load()
        im = Image.new('RGB', (SIZE, SIZE))
        out = im.load()
        for y in range(SIZE):
            for x in range(SIZE):
                dx = (src[(x + 1) % SIZE, y] - src[(x - 1) % SIZE, y]) / 255.0
                dy = (src[x, (y + 1) % SIZE] - src[x, (y - 1) % SIZE]) / 255.0
                # 0.28 keeps a packed-earth surface subtle at tactical scale.
                nx, ny, nz = -dx * .28, -dy * .28, 1.0
                length = max((nx * nx + ny * ny + nz * nz) ** .5, 1e-6)
                out[x, y] = tuple(int(round(max(0, min(1, c / length * .5 + .5)) * 255)) for c in (nx, ny, nz))
        name = '__local_normal__'
    else:
        z = zipfile.ZipFile(fetch(asset))
        name = next((n for n in z.namelist() if n.lower().endswith('_normalgl.jpg')), None)
    if name is None:
        # No normal map in this pack: a flat one, which perturbs nothing.
        im = Image.new('RGB', (SIZE, SIZE), (128, 128, 255))
    elif name != '__local_normal__':
        im = Image.open(io.BytesIO(z.read(name))).convert('RGB')
        im = im.resize((SIZE, SIZE), Image.LANCZOS)

    # The colour and normal layers share one sRGB texture array so they stay one
    # binding on WebGL. Sampling that texture decodes every channel to linear.
    # Raw normal bytes are already linear data, therefore storing 128 directly
    # would sample as 0.216 and bend a flat normal hard towards (-x, -y). Encode
    # each component as sRGB here so the GPU's decode restores the intended
    # linear 0..1 vector. This was the source of the acid-green, shimmering
    # terrain in oblique gameplay views.
    def to_srgb(v):
        v = max(0.0, min(1.0, v))
        v = v * 12.92 if v <= 0.0031308 else 1.055 * (v ** (1 / 2.4)) - 0.055
        return int(round(v * 255.0))

    im.putdata([
        tuple(to_srgb(c / 255.0) for c in p)
        for p in im.getdata()
    ])
    return im


def main():
    os.makedirs(OUT, exist_ok=True)
    body = bytearray()
    names = []
    # A release build can be reproduced from cached ambientCG sources.  During
    # offline development keep the already-baked CC0 layers byte-for-byte and
    # splice in the original Mosswatch layer instead of silently dropping all
    # texture detail because one source mirror is unavailable.
    existing = os.path.join(OUT, 'textures.bin')
    old = open(existing, 'rb').read() if os.path.exists(existing) else b''
    old_size = struct.unpack_from('<I', old, 8)[0] if len(old) >= 16 else 0
    old_layers = struct.unpack_from('<I', old, 12)[0] if len(old) >= 16 else 0
    old_ok = (old[:8] == struct.pack('<II', MAGIC, 2) and old_size == SIZE
              and old_layers == 8 and len(old) >= 16 + SIZE * SIZE * 4 * 8)
    old_layer_bytes = SIZE * SIZE * 4
    if old_ok:
        old_body = old[16:16 + old_layer_bytes * 8]
        # Old layout was [four colours][four normals].  New layout must remain
        # [five colours][five normals] for the shader's layer offset contract.
        body += old_body[:old_layer_bytes * 4]
        moss = colour_layer('LOCAL:mosswatch-grass-albedo-v2.png')
        body += moss.convert('RGBA').tobytes()
        body += old_body[old_layer_bytes * 4:]
        body += normal_layer('LOCAL:mosswatch-grass-albedo-v2.png').convert('RGBA').tobytes()
        names = LAYERS
    else:
        # Colours first, then matching normals: this is the normal online bake.
        for name, asset in LAYERS:
            im = colour_layer(asset)
            body += im.convert('RGBA').tobytes()
            names.append((name, asset))
            px = im.resize((1, 1), Image.LANCZOS).getpixel((0, 0))
            print('  %-9s %-28s average rgb%s' % (name, asset, px))
        for name, asset in LAYERS:
            body += normal_layer(asset).convert('RGBA').tobytes()
            print('  %-9s %-28s normal map' % (name, asset))

    head = struct.pack('<IIII', MAGIC, 2, SIZE, len(LAYERS) * 2)
    dst = os.path.join(OUT, 'textures.bin')
    open(dst, 'wb').write(head + bytes(body))
    print('\nwrote %s  %d layers of %dx%d  %.1f KB'
          % (dst, len(LAYERS) * 2, SIZE, SIZE, (len(head) + len(body)) / 1024.0))

    creds = os.path.join(OUT, 'CREDITS.md')
    with open(creds, 'a', newline='\n') as f:
        f.write('\n## Ground textures\n\n')
        f.write('CC0, from [ambientCG](https://ambientcg.com/), baked by\n')
        f.write('`tools/bake_textures.py`.\n\n')
        f.write('| Layer | Material | Source |\n| --- | --- | --- |\n')
        for name, asset in names:
            source = ('Original project asset, generated with OpenAI image generation on 2026-09-11'
                      if asset.startswith('LOCAL:')
                      else 'https://ambientcg.com/view?id=%s' % asset)
            f.write('| %s | %s | %s |\n' % (name, asset, source))


if __name__ == '__main__':
    main()
