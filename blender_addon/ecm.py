import bpy
import json
import os
from pathlib import Path

from bpy.props import (StringProperty, BoolProperty)
from bpy_extras.io_utils import (ImportHelper)
from mathutils import Matrix, Quaternion

from .common import create_srgb_node_group, relink_object

from . import trigger_vis


VERTEX_COLOR_LAYER_CANDIDATES = ("Col", "Color")


class EcmLoader(bpy.types.Operator, ImportHelper):
    """Import EuroChef map exports"""
    bl_idname = "eurochefutil.ecm"
    bl_description = "Import EuroChef map files"
    bl_label = "Import EuroChef ECM"

    filename_ext = ".ecm"
    filter_glob: StringProperty(
        default="*.ecm",
        options={'HIDDEN'},
    )

    filepath: StringProperty(subtype="FILE_PATH")
    merge_materials: BoolProperty(
        name="Merge materials (recommended)", default=True)
    lock_objects: BoolProperty(name="Make objects unselectable", default=False)
    autosmooth: BoolProperty(name="Smooth meshes", default=True)

    trigger_visualizations: BoolProperty(
        name="Visualize special triggers", default=False)

    surface_blending: BoolProperty(
        name="Surface blending (WIP)", default=False)
    rewrite_vertex_lighting_materials: BoolProperty(
        name="Rewrite materials for baked vertex lighting", default=True)
    use_material_preview: BoolProperty(
        name="Switch viewport to Material Preview", default=True)

    import_triggers: BoolProperty(name="Import triggers", default=True)
    import_paths: BoolProperty(name="Import paths", default=True)
    import_lights: BoolProperty(name="Import light markers", default=True)
    import_sounds: BoolProperty(name="Import sound markers", default=True)

    def execute(self, context):
        with open(self.filepath, 'r', encoding='utf-8') as file:
            self.data = json.load(file)
        self.directory = os.path.dirname(self.filepath)
        print("Loading data from {}".format(self.directory))
        if (not self.load()):
            return {'CANCELLED'}

        return {'FINISHED'}

    def load(self):
        if not self.data:
            return False

        create_srgb_node_group()

        self.processed_materials = set()
        self.resource_objects = []
        self.resource_cache = {}
        self.child_collections = {}
        self.edb_root = None
        maps_dir = Path(self.directory).resolve()
        self.current_edb_key = (
            maps_dir.parent.name if maps_dir.name.lower() == 'maps' else None)
        self.gltf_by_uid = {}
        self.gltf_by_alias_stem = {}
        self.source_file = None
        self._load_resource_index()

        map_name = Path(self.filepath).stem
        owner = self.current_edb_key or Path(self.directory).name
        self.collection = bpy.data.collections.new(f"{owner} - {map_name}")
        bpy.context.scene.collection.children.link(self.collection)

        scene_data = self._load_gui_scene()
        if scene_data is not None:
            self.source_file = scene_data.get('source_file')
            self._load_scene_items(scene_data)
        else:
            self._load_legacy_geometry()

        if self.import_paths:
            self.load_paths(self.data.get('paths', []))
        if self.import_lights:
            self.load_lights(self.data.get('lights', []))
        if self.import_sounds:
            self.load_sounds(self.data.get('sounds', []))

        if self.merge_materials:
            self.merge_all_materials()

        if self.import_triggers:
            print("Importing triggers")
            self.load_triggers(self.data.get('triggers', []))

        if self.use_material_preview:
            self._set_viewport_material_preview()

        return True

    def _set_viewport_material_preview(self):
        for window in bpy.context.window_manager.windows:
            screen = window.screen
            if screen is None:
                continue
            for area in screen.areas:
                if area.type != 'VIEW_3D':
                    continue
                for space in area.spaces:
                    if space.type == 'VIEW_3D':
                        space.shading.type = 'MATERIAL'
                        space.shading.use_scene_lights = False
                        space.shading.use_scene_world = False

    def child_collection(self, name):
        collection = self.child_collections.get(name)
        if collection is not None:
            return collection
        collection = bpy.data.collections.new(name)
        self.collection.children.link(collection)
        self.child_collections[name] = collection
        return collection

    def _find_vertex_color_layer(self, mesh):
        if mesh is None:
            return None
        if hasattr(mesh, 'color_attributes'):
            for name in VERTEX_COLOR_LAYER_CANDIDATES:
                layer = mesh.color_attributes.get(name)
                if layer is not None:
                    return layer
        if hasattr(mesh, 'vertex_colors'):
            for name in VERTEX_COLOR_LAYER_CANDIDATES:
                layer = mesh.vertex_colors.get(name)
                if layer is not None:
                    return layer
        return None

    def _load_gui_scene(self):
        path = Path(self.filepath).with_suffix('.gui_scene.json')
        if not path.is_file():
            print(f"[ECM] Expanded scene sidecar not found, using legacy placements: {path}")
            return None
        try:
            with path.open('r', encoding='utf-8') as file:
                scene = json.load(file)
        except (OSError, ValueError) as error:
            print(f"[ECM] Failed to read expanded scene sidecar {path}: {error}")
            return None
        print(
            f"[ECM] Using expanded scene: {len(scene.get('items', []))} items, "
            f"{len(scene.get('missing', []))} diagnostics")
        return scene

    def _load_resource_index(self):
        start = Path(self.directory).resolve()
        for root in (start, *start.parents):
            index_path = root / '_shared' / 'gltf_library' / 'GLOBAL_RESOURCE_INDEX.json'
            if not index_path.is_file():
                continue
            try:
                with index_path.open('r', encoding='utf-8') as file:
                    index = json.load(file)
            except (OSError, ValueError) as error:
                print(f"[ECM] Failed to read glTF resource index {index_path}: {error}")
                return

            self.edb_root = root
            maps_dir = Path(self.directory).resolve()
            if maps_dir.name.lower() == 'maps' and maps_dir.parent.parent == root:
                self.current_edb_key = maps_dir.parent.name

            for record in index.get('resources', []):
                if record.get('kind') != 'gltf':
                    continue
                uid = record.get('uid')
                if uid:
                    self.gltf_by_uid.setdefault(uid.lower(), []).append(record)
                alias = str(record.get('alias', '')).split('#', 1)[0]
                stem = Path(alias).stem.lower()
                if stem:
                    self.gltf_by_alias_stem.setdefault(stem, []).append(record)

            print(
                f"[ECM] Loaded glTF library index schema={index.get('schema_version')} "
                f"owner={self.current_edb_key or 'unknown'}")
            return

    def _direct_model_paths(self, stem, kind):
        owner_dir = Path(self.directory).parent
        folders = [Path(self.directory)]
        if kind == 'script_animation_skin':
            folders.append(owner_dir / 'animations')
        else:
            folders.extend((owner_dir / 'entities', owner_dir / 'maps'))

        seen = set()
        for folder in folders:
            path = folder / f"{stem}.gltf"
            key = str(path).lower()
            if key not in seen:
                seen.add(key)
                yield path

    def _resolve_model(self, item, stem=None):
        kind = item.get('kind', 'placement')
        object_hash = int(item.get('object_hash', 0))
        if stem is None:
            stem = f"ref_{object_hash}" if kind == 'mapzone' else f"{object_hash:x}"

        same_file = self.source_file is not None and item.get('file_hash') == self.source_file
        legacy_current_file = self.source_file is None

        # Raw output is only safe for resources owned by this EDB. An external
        # local hash can legally collide with a same-named resource here.
        if same_file or legacy_current_file or kind == 'mapzone':
            for path in self._direct_model_paths(stem, kind):
                if path.is_file():
                    return path, None

        if self.edb_root is None:
            return None, None

        if kind == 'mapzone':
            records = list(self.gltf_by_alias_stem.get(stem.lower(), []))
        else:
            records = list(self.gltf_by_uid.get(f"0x{object_hash:08x}", []))
            if not records:
                records = list(self.gltf_by_alias_stem.get(stem.lower(), []))

        if kind == 'script_animation_skin':
            records = [record for record in records
                       if record.get('category') == 'animations']
        else:
            records = [record for record in records
                       if record.get('category') in ('maps', 'entities')]

        if self.current_edb_key and (same_file or legacy_current_file or kind == 'mapzone'):
            owner_records = [
                record for record in records
                if str(record.get('edb', '')).lower() == self.current_edb_key.lower()
            ]
            if owner_records:
                records = owner_records

        paths = {}
        for record in records:
            canonical = record.get('canonical_path')
            if not canonical:
                continue
            path = self.edb_root / Path(canonical)
            if path.is_file():
                paths.setdefault(str(path).lower(), (path, record))

        if len(paths) == 1:
            path, record = next(iter(paths.values()))
            return path, record.get('resource_label')
        if len(paths) > 1:
            print(
                f"[ECM] Ambiguous resource {stem}: {len(paths)} distinct glTF payloads; "
                "not guessing across EDBs")
        return None, None

    def _resource_collection(self, model_path):
        key = str(model_path.resolve()).lower()
        cached = self.resource_cache.get(key)
        if cached is not None:
            return cached

        before = {obj.as_pointer() for obj in bpy.data.objects}
        print(f"[ECM] Loading {model_path}")
        bpy.ops.import_scene.gltf(filepath=str(model_path))
        imported = [obj for obj in bpy.data.objects if obj.as_pointer() not in before]
        if not imported:
            print(f"[ECM] glTF importer created no objects for {model_path}")
            return None

        cache_name = f"__EuroChefResource_{len(self.resource_cache):04d}_{model_path.stem}"
        collection = bpy.data.collections.new(cache_name)
        for obj in imported:
            relink_object(obj, collection)
            self.resource_objects.append(obj)
            if obj.type == 'MESH':
                if self.autosmooth:
                    for polygon in obj.data.polygons:
                        polygon.use_smooth = True
                self.process_blended_surfaces(obj)

        self.resource_cache[key] = collection
        return collection

    def _new_model_instance(self, item, model_path, resource_label=None):
        resource = self._resource_collection(model_path)
        if resource is None:
            return None

        kind = item.get('kind', 'placement')
        object_hash = int(item.get('object_hash', 0))
        label = resource_label or (
            f"ref_{object_hash}" if kind == 'mapzone' else f"0x{object_hash:08X}")
        obj = bpy.data.objects.new(f"{label} [{kind}]", None)
        obj.instance_type = 'COLLECTION'
        obj.instance_collection = resource

        collection_name = {
            'mapzone': 'MapZones',
            'sky': 'Sky',
            'script_entity': 'Scripts',
            'script_animation_skin': 'Scripts',
            'trigger_visual': 'Trigger Visuals',
        }.get(kind, 'Placements')
        self.child_collection(collection_name).objects.link(obj)

        obj['eurochef_kind'] = kind
        obj['eurochef_source'] = str(item.get('source', ''))
        obj['eurochef_file_hash'] = f"0x{int(item.get('file_hash', 0)):08X}"
        obj['eurochef_object_hash'] = f"0x{object_hash:08X}"
        obj['eurochef_model_path'] = str(model_path)
        obj.hide_select = self.lock_objects
        return obj

    def _load_scene_items(self, scene_data):
        loaded = 0
        unresolved = 0
        for item in scene_data.get('items', []):
            model_path, resource_label = self._resolve_model(item)
            if model_path is None:
                unresolved += 1
                print(
                    f"[ECM] Couldn't resolve {item.get('kind')} "
                    f"0x{int(item.get('object_hash', 0)):08X} from {item.get('source')}")
                continue

            obj = self._new_model_instance(item, model_path, resource_label)
            if obj is None:
                unresolved += 1
                continue
            obj.location = egx_to_blender_pos(tuple(item.get('position', (0, 0, 0))))
            obj.rotation_mode = 'QUATERNION'
            obj.rotation_quaternion = egx_to_blender_quat(
                tuple(item.get('rotation_xyzw', (0, 0, 0, 1))))
            obj.scale = egx_to_blender_scale(tuple(item.get('scale', (1, 1, 1))))
            loaded += 1

        diagnostics = scene_data.get('missing', [])
        print(
            f"[ECM] Expanded scene loaded={loaded} unresolved={unresolved} "
            f"native_diagnostics={len(diagnostics)}")
        for diagnostic in diagnostics[:20]:
            print(
                f"[ECM] native missing: {diagnostic.get('reason')} "
                f"0x{int(diagnostic.get('object_hash', 0)):08X} source={diagnostic.get('source')}")

    def _load_legacy_geometry(self):
        for index, placement in enumerate(self.data.get('placements', [])):
            object_hash = int(placement['object_ref'])
            item = {
                'kind': 'placement',
                'source': f"placement[{index}]",
                'file_hash': 0,
                'object_hash': object_hash,
            }
            model_path, resource_label = self._resolve_model(item)
            if model_path is None:
                print(f"[ECM] Couldn't find legacy placement model 0x{object_hash:08X}")
                continue
            obj = self._new_model_instance(item, model_path, resource_label)
            if obj is None:
                continue
            obj.location = egx_to_blender_pos(tuple(placement['position']))
            obj.rotation_mode = 'ZXY'
            obj.rotation_euler = egx_to_blender_rot(tuple(placement['rotation']))
            obj.scale = egx_to_blender_scale(tuple(placement['scale']))

        for index, mapzone in enumerate(self.data.get('mapzone_entities', [])):
            refptr = int(mapzone['entity_refptr'])
            item = {
                'kind': 'mapzone',
                'source': f"mapzone[{index}]",
                'file_hash': 0,
                'object_hash': refptr,
            }
            model_path, resource_label = self._resolve_model(item, f"ref_{refptr}")
            if model_path is None:
                print(f"[ECM] Couldn't find legacy mapzone model ref_{refptr}")
                continue
            self._new_model_instance(item, model_path, resource_label)

    def load_paths(self, paths):
        if not paths:
            return
        collection = self.child_collection('Paths')
        for index, path in enumerate(paths):
            nodes = path.get('nodes', [])
            if len(nodes) < 2:
                continue
            base = path.get('position', (0, 0, 0))
            world = [
                egx_to_blender_pos((
                    base[0] + node['position'][0],
                    base[1] + node['position'][1],
                    base[2] + node['position'][2],
                ))
                for node in nodes
            ]
            segments = []
            for link in path.get('links', []):
                a = int(link.get('node_a', -1))
                b = int(link.get('node_b', -1))
                if 0 <= a < len(world) and 0 <= b < len(world):
                    segments.append((world[a], world[b]))

            curve = bpy.data.curves.new(f"Path_{index:03d}", 'CURVE')
            curve.dimensions = '3D'
            if segments:
                for start, end in segments:
                    spline = curve.splines.new('POLY')
                    spline.points.add(1)
                    spline.points[0].co = (*start, 1.0)
                    spline.points[1].co = (*end, 1.0)
            else:
                spline = curve.splines.new('POLY')
                spline.points.add(len(world) - 1)
                for point, position in zip(spline.points, world):
                    point.co = (*position, 1.0)

            hashcode = int(path.get('hashcode', 0))
            obj = bpy.data.objects.new(
                f"Path_{index:03d}_0x{hashcode:08X}", curve)
            collection.objects.link(obj)
            obj['hashcode'] = f"0x{hashcode:08X}"
            obj['flags'] = f"0x{int(path.get('flags', 0)):08X}"
            obj['path_type'] = int(path.get('ptype', 0))
            obj.hide_select = self.lock_objects

    def load_lights(self, lights):
        if not lights:
            return
        collection = self.child_collection('Lights')
        for index, light in enumerate(lights):
            hashcode = json_int(light.get('hashcode', 0))
            obj = bpy.data.objects.new(f"Light_{index:03d}_0x{hashcode:08X}", None)
            collection.objects.link(obj)
            obj.empty_display_type = 'SPHERE'
            obj.empty_display_size = max(0.05, abs(json_float(light.get('radius', 1.0), 1.0)))
            obj.location = egx_to_blender_pos(tuple(light.get('position', (0, 0, 0))))
            obj['hashcode'] = f"0x{hashcode:08X}"
            obj['flags'] = f"0x{json_int(light.get('flags', 0)):08X}"
            obj['light_type'] = json_int(light.get('ltype', 0))
            obj['beam_angle'] = json_int(light.get('beam_angle', 0))
            obj['beam'] = json.dumps(light.get('beam', (0, 0, 0)))
            obj['colour'] = json.dumps(light.get('colour', (255, 255, 255, 255)))
            obj['radius'] = json_float(light.get('radius', 0.0))
            obj['max_effect_fraction'] = json_float(light.get('max_effect_fraction', 0.0))
            obj.hide_select = self.lock_objects

    def load_sounds(self, sounds):
        if not sounds:
            return
        collection = self.child_collection('Sounds')
        for index, sound in enumerate(sounds):
            hashcode = json_int(sound.get('hashcode', 0))
            obj = bpy.data.objects.new(f"Sound_{index:03d}_0x{hashcode:08X}", None)
            collection.objects.link(obj)
            obj.empty_display_type = 'SPHERE'
            obj.empty_display_size = max(0.05, abs(json_float(sound.get('outer_radius', 1.0), 1.0)))
            obj.location = egx_to_blender_pos(tuple(sound.get('position', (0, 0, 0))))
            obj['hashcode'] = f"0x{hashcode:08X}"
            obj['flags'] = f"0x{json_int(sound.get('flags', 0)):08X}"
            obj['sound_ref'] = f"0x{json_int(sound.get('sound_ref', 0)):08X}"
            obj['volume'] = json_int(sound.get('volume', 0))
            obj['fade_in'] = json_int(sound.get('fade_in', 0))
            obj['fade_out'] = json_int(sound.get('fade_out', 0))
            obj['tracking_type'] = json_int(sound.get('tracking_type', 0))
            obj['inner_radius'] = json_float(sound.get('inner_radius', 0.0))
            obj['outer_radius'] = json_float(sound.get('outer_radius', 0.0))
            obj['base_map_on'] = f"0x{json_int(sound.get('base_map_on', 0)):08X}"
            obj.hide_select = self.lock_objects

    def process_blended_surfaces(self, obj: bpy.types.Object):
        if obj.type != 'MESH' or obj.data is None:
            return

        for slot in obj.material_slots:
            material = slot.material
            if material is None or material.name in self.processed_materials:
                continue
            self.processed_materials.add(material.name)
            if self.rewrite_vertex_lighting_materials:
                self.rewrite_material_vertex_lighting(material)

        if not self.surface_blending:
            return

        mesh: bpy.types.Mesh = obj.data
        color_layer = self._find_vertex_color_layer(mesh)
        if color_layer is None:
            return

        seen_materials = []
        for poly in mesh.polygons:
            if poly.material_index >= len(obj.material_slots):
                continue
            material = obj.material_slots[poly.material_index].material
            if material is None or material.name in seen_materials:
                continue
            for idx in poly.loop_indices:
                rgb = color_layer.data[idx].color
                if rgb[3] < 1.0:
                    print(
                        f"Surface has transparency {rgb[0]} {rgb[1]} {rgb[2]} {rgb[3]}")
                    seen_materials.append(material.name)
                    self.modify_material_for_blending(material)
                    break

    # Rewrite the material to use vertex colors as baked lighting.
    def rewrite_material_vertex_lighting(self, material: bpy.types.Material):
        if material is None or not material.use_nodes or material.node_tree is None:
            return

        transparency = getattr(material, 'blend_method', 'OPAQUE')
        source_texture = next((
            node for node in material.node_tree.nodes
            if node.type == 'TEX_IMAGE' and node.image is not None
        ), None)
        if source_texture is None:
            return
        image = source_texture.image

        material.node_tree.nodes.clear()
        texture_node = material.node_tree.nodes.new("ShaderNodeTexImage")
        texture_node.image = image
        try:
            texture_node.image.colorspace_settings.name = "Linear"
        except TypeError:
            pass

        try:
            vertex_color_node = material.node_tree.nodes.new("ShaderNodeVertexColor")
            vertex_color_node.layer_name = "Color"
        except RuntimeError:
            vertex_color_node = material.node_tree.nodes.new("ShaderNodeAttribute")
            vertex_color_node.attribute_name = "Color"

        output_node = material.node_tree.nodes.new("ShaderNodeOutputMaterial")
        multiply_texture_node = material.node_tree.nodes.new("ShaderNodeVectorMath")
        multiply_texture_node.operation = 'MULTIPLY'
        double_vertex_colors_node = material.node_tree.nodes.new("ShaderNodeVectorMath")
        double_vertex_colors_node.operation = 'MULTIPLY'
        double_vertex_colors_node.inputs[1].default_value = (2.0, 2.0, 2.0)
        srgb_node = material.node_tree.nodes.new("ShaderNodeGroup")
        srgb_node.node_tree = bpy.data.node_groups['srgbApprox']

        material.node_tree.links.new(
            double_vertex_colors_node.inputs[0], vertex_color_node.outputs[0])
        material.node_tree.links.new(
            multiply_texture_node.inputs[0], texture_node.outputs[0])
        material.node_tree.links.new(
            multiply_texture_node.inputs[1], double_vertex_colors_node.outputs[0])
        material.node_tree.links.new(
            srgb_node.inputs[0], multiply_texture_node.outputs[0])

        shader_node = material.node_tree.nodes.new("ShaderNodeEmission")
        material.node_tree.links.new(shader_node.inputs['Color'], srgb_node.outputs[0])

        if transparency != 'OPAQUE':
            transparency_node = material.node_tree.nodes.new("ShaderNodeBsdfTransparent")
            transparency_mix_node = material.node_tree.nodes.new("ShaderNodeMixShader")
            material.node_tree.links.new(
                transparency_mix_node.inputs[0], texture_node.outputs[1])
            material.node_tree.links.new(
                transparency_mix_node.inputs[1], transparency_node.outputs[0])
            material.node_tree.links.new(
                transparency_mix_node.inputs[2], shader_node.outputs[0])
            material.node_tree.links.new(
                output_node.inputs[0], transparency_mix_node.outputs[0])
        else:
            material.node_tree.links.new(output_node.inputs[0], shader_node.outputs[0])

    def modify_material_for_blending(self, material: bpy.types.Material):
        if hasattr(material, 'blend_method'):
            material.blend_method = 'HASHED'

        vertex_color_node = next((
            node for node in material.node_tree.nodes
            if node.type in ('VERTEX_COLOR', 'ATTRIBUTE')
        ), None)
        output_node = next((
            node for node in material.node_tree.nodes
            if node.type == 'OUTPUT_MATERIAL'
        ), None)
        if vertex_color_node is None or output_node is None or not output_node.inputs[0].links:
            return

        mix_node = material.node_tree.nodes.new("ShaderNodeMixShader")
        transparency_node = material.node_tree.nodes.new("ShaderNodeBsdfTransparent")
        output_node_shader = output_node.inputs[0].links[0].from_node
        material.node_tree.links.new(
            mix_node.inputs[2], output_node_shader.outputs[0])
        material.node_tree.links.new(
            output_node.inputs["Surface"], mix_node.outputs[0])
        material.node_tree.links.new(
            mix_node.inputs[1], transparency_node.outputs["BSDF"])
        material.node_tree.links.new(
            mix_node.inputs["Fac"], vertex_color_node.outputs["Alpha"])

    # Merge duplicate materials produced by repeated glTF imports.
    def merge_all_materials(self):
        all_base_materials = {}
        duplicates = 0
        materials = {
            slot.material.name: slot.material
            for obj in self.resource_objects if obj.type == 'MESH'
            for slot in obj.material_slots if slot.material is not None
        }

        for material in materials.values():
            basename = blender_duplicate_basename(material.name)
            if basename == material.name:
                all_base_materials[material.name] = material
            else:
                duplicates += 1

        print(
            f"Merging {duplicates} duplicate materials ({len(all_base_materials)} base materials in total)")

        for obj in self.resource_objects:
            if obj.type != 'MESH':
                continue
            for index, slot in enumerate(obj.material_slots):
                if slot.material is None:
                    continue
                basename = blender_duplicate_basename(slot.material.name)
                if basename == slot.material.name:
                    continue
                base_material = all_base_materials.get(basename)
                if base_material is None:
                    continue
                if getattr(base_material, 'blend_method', 'OPAQUE') != getattr(
                        slot.material, 'blend_method', 'OPAQUE'):
                    continue
                obj.material_slots[index].material = base_material

    def load_triggers(self, triggers):
        self.trigger_collection = self.child_collection("Triggers")
        forensics = {
            int(item.get('index', -1)): item
            for item in self.data.get('trigger_forensics', [])
        }
        trigger_scripts = {
            int(item.get('index', -1)): item
            for item in self.data.get('trigger_scripts', [])
        }

        for index, trigger in enumerate(triggers):
            obj = bpy.data.objects.new(
                f"{index:03d}#{trigger.get('ttype', 'UnknownTrigger')}", None)
            self.trigger_collection.objects.link(obj)
            obj.empty_display_type = 'PLAIN_AXES'
            obj.location = egx_to_blender_pos(
                tuple(trigger.get('position', (0, 0, 0))))
            obj.rotation_mode = 'ZXY'
            obj.rotation_euler = egx_to_blender_rot(
                tuple(trigger.get('rotation', (0, 0, 0))))
            obj.scale = egx_to_blender_scale(
                tuple(trigger.get('scale', (1, 1, 1))))

            obj.show_name = True
            obj.hide_select = self.lock_objects
            obj['index'] = index
            obj['link_ref'] = int(trigger.get('link_ref', -1))
            obj['debug'] = int(trigger.get('debug', 0))
            obj['game_flags'] = f"0x{int(trigger.get('game_flags', 0)):08X}"
            obj['trig_flags'] = f"0x{int(trigger.get('trig_flags', 0)):08X}"

            if trigger.get('tsubtype'):
                obj['subtype'] = trigger['tsubtype']
            for data_index, value in enumerate(trigger.get('data', [])):
                if value is not None:
                    obj[f'data[0x{data_index:X}]'] = f"0x{value:X}"
            for link_index, value in enumerate(trigger.get('links', [])):
                if value != -1:
                    obj[f'links[{link_index}]'] = str(value)
            for extra_index, value in enumerate(trigger.get('extra_data', [])):
                if value != 0xffffffff:
                    obj[f'extra_data[{extra_index}]'] = f"0x{value:X}"

            forensic = forensics.get(index)
            if forensic:
                obj['trigger_file_offset'] = int(forensic.get('trigger_file_offset', 0))
                obj['link_ref'] = int(forensic.get('link_ref', -1))
                obj['type_index'] = int(forensic.get('type_index', 0))
                obj['serialized_type_id'] = int(forensic.get('trig_type', 0))
                obj['serialized_subtype_id'] = int(forensic.get('trig_subtype', 0))
                obj['engine_options'] = json.dumps(
                    forensic.get('engine_options', {}), sort_keys=True)
                obj['script_create_flags'] = json.dumps(
                    forensic.get('script_create_flags'), sort_keys=True)
                obj['incoming_links'] = json.dumps(forensic.get('incoming_links', []))

            trigger_script = trigger_scripts.get(index)
            if trigger_script:
                obj['script_file_offset'] = int(trigger_script.get('script_file_offset', 0))
                obj['script_aux'] = f"0x{int(trigger_script.get('aux', 0)):08X}"

            if self.trigger_visualizations:
                trigger_vis.process_triggers(
                    trigger.get('data', []), trigger.get('links', []), obj,
                    index, trigger.get('ttype', ''))


def blender_duplicate_basename(name):
    base, separator, suffix = name.rpartition('.')
    if separator and len(suffix) == 3 and suffix.isdigit():
        return base
    return name


def json_float(value, default=0.0):
    try:
        if value is None:
            raise TypeError
        return float(value)
    except (TypeError, ValueError):
        return default


def json_int(value, default=0):
    try:
        if value is None:
            raise TypeError
        return int(value)
    except (TypeError, ValueError):
        return default


def json_vec3(value, default=(0.0, 0.0, 0.0)):
    if not isinstance(value, (list, tuple)):
        value = default
    return (
        json_float(value[0] if len(value) > 0 else default[0], default[0]),
        json_float(value[1] if len(value) > 1 else default[1], default[1]),
        json_float(value[2] if len(value) > 2 else default[2], default[2]),
    )


def egx_to_blender_quat(rotation_xyzw: tuple):
    if len(rotation_xyzw) != 4:
        return Quaternion((1.0, 0.0, 0.0, 0.0))

    quat_len_sq = sum(float(component) * float(component) for component in rotation_xyzw)
    if quat_len_sq <= 1.0e-16:
        return Quaternion((1.0, 0.0, 0.0, 0.0))

    quat = Quaternion((
        rotation_xyzw[3],
        rotation_xyzw[0],
        rotation_xyzw[1],
        rotation_xyzw[2],
    ))
    quat.normalize()

    basis = Matrix((
        (-1.0, 0.0, 0.0),
        (0.0, 0.0, -1.0),
        (0.0, 1.0, 0.0),
    ))
    return (basis @ quat.to_matrix() @ basis.transposed()).to_quaternion()


def egx_to_blender_pos(pos: tuple):
    pos = json_vec3(pos)
    return (
        -pos[0],
        -pos[2],
        pos[1],
    )


def egx_to_blender_rot(pos: tuple):
    pos = json_vec3(pos)
    return (
        pos[0],
        pos[2],
        -pos[1],
    )


def egx_to_blender_scale(pos: tuple):
    pos = json_vec3(pos, (1.0, 1.0, 1.0))
    return (
        pos[0],
        pos[2],
        pos[1],
    )


def menu_import(self, context):
    self.layout.operator(EcmLoader.bl_idname, text='EuroChef ECM (.ecm)')


def register():
    bpy.utils.register_class(EcmLoader)
    bpy.types.TOPBAR_MT_file_import.append(menu_import)


def unregister():
    bpy.types.TOPBAR_MT_file_import.remove(menu_import)
    bpy.utils.unregister_class(EcmLoader)
