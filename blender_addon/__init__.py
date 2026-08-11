from . import ecm

bl_info = {
    "name": "EuroChef Utility",
    "author": "cohaereo",
    "description": "Import EuroChef maps, expanded Robots scene data and deduplicated glTF resources",
    "blender": (2, 80, 0),
    "version": (0, 2, 0),
    "location": "File -> Import",
    "category": "Import-Export"
}


def register():
    ecm.register()


def unregister():
    ecm.unregister()
