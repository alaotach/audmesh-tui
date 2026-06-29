import bpy
import os
workspace_dir = "C:\\Users\\Aryan\\audmesh"
obj_path = os.path.join(workspace_dir, "base_mesh.obj")
mdd_path = os.path.join(workspace_dir, "animation.mdd")
bpy.ops.wm.obj_import(filepath=obj_path)
obj = bpy.context.active_object

if obj:
    mod = obj.modifiers.new(name="Mesh", type='MESH_CACHE')
    mod.cache_format = 'MDD'
    mod.filepath = mdd_path
    mod.forward_axis = 'POS_Y'
    mod.up_axis = 'POS_Z'
    print(f"{obj.name} loaded")