"""Render one authored OBJ in Blender for asset-pipeline inspection.

This is deliberately a small diagnostic, not a replacement renderer.  It lets
the game team inspect the exact source assembly that the browser bake consumes
when a live tower silhouette looks wrong.  Usage:

  blender.exe -b --python tools/render_asset_preview.py -- input.obj output.png
"""

from __future__ import annotations

import math
import sys

import bpy
from mathutils import Vector


def look_at(obj: bpy.types.Object, point: Vector) -> None:
    obj.rotation_euler = (point - obj.location).to_track_quat("-Z", "Y").to_euler()


def main() -> None:
    args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    if len(args) != 2:
        raise SystemExit("expected input OBJ and output PNG after --")
    source, output = args

    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    # The production baker accepts both our authored OBJ meshes and cached
    # CC0 glTF sources. Let the visual inspector open either format so an
    # asset-quality decision is made from geometry, not filenames or a model
    # list. This remains a local Blender preview; it does not alter a bake.
    if source.lower().endswith((".glb", ".gltf")):
        bpy.ops.import_scene.gltf(filepath=source)
    else:
        bpy.ops.wm.obj_import(filepath=source)
    meshes = [obj for obj in bpy.context.scene.objects if obj.type == "MESH"]
    if not meshes:
        raise SystemExit(f"no mesh imported from {source}")

    points = [obj.matrix_world @ Vector(corner) for obj in meshes for corner in obj.bound_box]
    low = Vector((min(p.x for p in points), min(p.y for p in points), min(p.z for p in points)))
    high = Vector((max(p.x for p in points), max(p.y for p in points), max(p.z for p in points)))
    centre = (low + high) * 0.5
    extent = max((high - low).length, 0.1)

    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE_NEXT"
    scene.render.resolution_x = 900
    scene.render.resolution_y = 900
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.filepath = output
    scene.world.color = (0.018, 0.026, 0.018)

    camera_data = bpy.data.cameras.new("PreviewCamera")
    camera = bpy.data.objects.new("PreviewCamera", camera_data)
    scene.collection.objects.link(camera)
    scene.camera = camera
    camera.location = centre + Vector((extent * 1.30, -extent * 1.70, extent * 0.95))
    camera_data.lens = 54
    look_at(camera, centre)

    for name, offset, energy, size, colour in [
        ("Key", (1.3, -1.6, 2.1), 1200.0, extent * 1.2, (1.0, 0.80, 0.58)),
        ("Fill", (-1.5, -0.7, 1.2), 700.0, extent, (0.45, 0.66, 1.0)),
        ("Rim", (0.4, 1.6, 2.4), 950.0, extent * 0.9, (0.88, 0.96, 1.0)),
    ]:
        light_data = bpy.data.lights.new(name, "AREA")
        light_data.energy = energy
        light_data.shape = "DISK"
        light_data.size = size
        light_data.color = colour
        light = bpy.data.objects.new(name, light_data)
        scene.collection.objects.link(light)
        light.location = centre + Vector(offset) * extent
        look_at(light, centre)

    bpy.ops.mesh.primitive_plane_add(size=extent * 6.0, location=(centre.x, centre.y, low.z - extent * 0.015))
    plane = bpy.context.object
    material = bpy.data.materials.new("PreviewGround")
    material.diffuse_color = (0.055, 0.075, 0.045, 1.0)
    material.roughness = 0.92
    plane.data.materials.append(material)

    bpy.ops.render.render(write_still=True)


if __name__ == "__main__":
    main()
