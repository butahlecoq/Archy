"""Exact IFC4 schematic spatial hierarchy and swept profiles."""

import uuid

from .geometry import footprint
from .properties import attach

NAMESPACE = uuid.uuid5(uuid.NAMESPACE_URL, "urn:archy:ifc:v1")


def assign_stable_guids(model, request):
    """Derive each root identity from its semantic role, never entity order."""
    import ifcopenshell

    def endpoint(entity):
        if not entity.Name:
            raise ValueError("unnamed IFC relationship endpoint")
        return f"{entity.is_a()}:{entity.Name}"

    def endpoints(entities):
        return ",".join(sorted(endpoint(entity) for entity in entities))

    seen = set()
    for root in model.by_type("IfcRoot"):
        class_name = root.is_a()
        scope = request["project_id"]
        if class_name == "IfcRelAggregates":
            role = f"{endpoint(root.RelatingObject)}>{endpoints(root.RelatedObjects)}"
            if not root.RelatingObject.is_a("IfcProject"):
                scope += "|" + request["variant_id"]
        elif class_name == "IfcRelContainedInSpatialStructure":
            scope += "|" + request["variant_id"]
            role = f"{endpoint(root.RelatingStructure)}>{endpoints(root.RelatedElements)}"
        elif class_name == "IfcRelDefinesByProperties":
            scope += "|" + request["variant_id"]
            role = (f"{endpoint(root.RelatingPropertyDefinition)}"
                    f">{endpoints(root.RelatedObjects)}")
        elif class_name in ("IfcProject", "IfcSite"):
            role = endpoint(root)
        elif class_name in ("IfcBuilding", "IfcBuildingStorey",
                            "IfcBuildingElementProxy", "IfcPropertySet", "IfcElementQuantity"):
            scope += "|" + request["variant_id"]
            role = endpoint(root)
        else:
            raise ValueError("unsupported IFC root identity role: " + class_name)
        key = f"{scope}|{class_name}|{role}"
        if key in seen:
            raise ValueError("duplicate IFC semantic identity role")
        seen.add(key)
        root.GlobalId = ifcopenshell.guid.compress(uuid.uuid5(NAMESPACE, key).hex)


def build(request):
    import ifcopenshell
    import ifcopenshell.api.aggregate
    import ifcopenshell.api.context
    import ifcopenshell.api.geometry
    import ifcopenshell.api.profile
    import ifcopenshell.api.project
    import ifcopenshell.api.root
    import ifcopenshell.api.spatial
    import ifcopenshell.api.unit
    import numpy as np

    v = request["variant"]
    project_id = request["project_id"]
    variant_id = request["variant_id"]
    height = v["floor_to_floor_m"]
    ox, oy = v["parameters"]["origin_xy_m"]
    local, area = footprint(v)
    model = ifcopenshell.api.project.create_file(version="IFC4")
    project = ifcopenshell.api.root.create_entity(model, ifc_class="IfcProject", name=f"{project_id}:project")
    site = ifcopenshell.api.root.create_entity(model, ifc_class="IfcSite", name=f"{project_id}:site")
    building = ifcopenshell.api.root.create_entity(model, ifc_class="IfcBuilding", name=f"{project_id}:{variant_id}:building")
    ifcopenshell.api.aggregate.assign_object(model, products=[site], relating_object=project)
    ifcopenshell.api.aggregate.assign_object(model, products=[building], relating_object=site)
    units = [ifcopenshell.api.unit.add_si_unit(model, unit_type=kind, prefix=None)
             for kind in ("LENGTHUNIT", "AREAUNIT", "VOLUMEUNIT")]
    ifcopenshell.api.unit.assign_unit(model, units=units)
    context = ifcopenshell.api.context.add_context(model, context_type="Model")
    body = ifcopenshell.api.context.add_context(model, context_type="Model", context_identifier="Body",
                                               target_view="MODEL_VIEW", parent=context)
    profile = ifcopenshell.api.profile.add_arbitrary_profile(model, profile=local,
                                                             name="ArchyCanonicalFootprint")

    def matrix(x=0.0, y=0.0, z=0.0):
        result = np.eye(4)
        result[:3, 3] = (x, y, z)
        return result

    ifcopenshell.api.geometry.edit_object_placement(model, product=site, matrix=matrix())
    ifcopenshell.api.geometry.edit_object_placement(model, product=building, matrix=matrix(ox, oy))
    proxies = []
    for i in range(v["storeys"]):
        storey = ifcopenshell.api.root.create_entity(model, ifc_class="IfcBuildingStorey",
                                                    name=f"{project_id}:{variant_id}:storey:{i:03d}")
        storey.Elevation = i * height
        ifcopenshell.api.aggregate.assign_object(model, products=[storey], relating_object=building)
        proxy = ifcopenshell.api.root.create_entity(model, ifc_class="IfcBuildingElementProxy",
                                                   predefined_type="USERDEFINED",
                                                   name=f"{project_id}:{variant_id}:mass:{i:03d}")
        proxy.ObjectType = "ArchySchematicStoreyMass"
        proxy.Description = "Schematic extruded storey mass; DRAFT; no detailed construction semantics"
        ifcopenshell.api.spatial.assign_container(model, products=[proxy], relating_structure=storey)
        representation = ifcopenshell.api.geometry.add_profile_representation(
            model, context=body, profile=profile, depth=height, cardinal_point=None)
        proxy.Representation = model.create_entity("IfcProductDefinitionShape", Representations=[representation])
        ifcopenshell.api.geometry.edit_object_placement(model, product=storey,
                                                        matrix=matrix(ox, oy, i * height))
        ifcopenshell.api.geometry.edit_object_placement(model, product=proxy,
                                                        matrix=matrix(ox, oy, i * height))
        proxies.append(proxy)
    attach(model, request, building, proxies, area)
    assign_stable_guids(model, request)
    return model
