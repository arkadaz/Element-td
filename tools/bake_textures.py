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
    ('dirt', 'Ground054'),
    # The HUD's own materials. They live in the same blob as the ground because
    # they are the same thing - a normalised albedo tinted by a colour the game
    # already chose - and one blob is one loader, one failure mode and one set
    # of credits.
    ('wood', 'Planks037A'),
    ('stone', 'Rock051'),
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
    z = zipfile.ZipFile(fetch(asset))
    name = next((n for n in z.namelist() if n.lower().endswith('_normalgl.jpg')), None)
    if name is None:
        # No normal map in this pack: a flat one, which perturbs nothing.
        im = Image.new('RGB', (SIZE, SIZE), (128, 128, 255))
    else:
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
    # Colours first, then the matching normals in the same order, so a layer's
    # normal is always at `layer + len(LAYERS)`. `solid.wgsl` relies on that.
    for name, asset in LAYERS:
        im = colour_layer(asset)
        body += im.convert('RGBA').tobytes()
        names.append((name, asset))
        px = im.resize((1, 1), Image.LANCZOS).getpixel((0, 0))
        print('  %-7s %-12s average rgb%s' % (name, asset, px))
    for name, asset in LAYERS:
        body += normal_layer(asset).convert('RGBA').tobytes()
        print('  %-7s %-12s normal map' % (name, asset))

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
            f.write('| %s | %s | https://ambientcg.com/view?id=%s |\n'
                    % (name, asset, asset))


if __name__ == '__main__':
    main()
