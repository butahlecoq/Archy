import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from test_protocol import ROOT, request

sys.path.insert(0, str(ROOT / "workers" / "ifc"))

try:
    import ifcopenshell
except ImportError:
    ifcopenshell = None


@unittest.skipUnless(ifcopenshell, "pinned IfcOpenShell runtime required")
class NativeTests(unittest.TestCase):
    def test_active_project_units_reject_missing_wrong_and_duplicate_assignments(self):
        from archy_ifc_worker import hierarchy, verify
        item = request()
        for case in ("missing", "wrong", "duplicate"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                model = hierarchy.build(item)
                project = model.by_type("IfcProject")[0]
                assigned = {unit.UnitType: unit for unit in project.UnitsInContext.Units}
                if case == "missing":
                    project.UnitsInContext = None
                elif case == "wrong":
                    project.UnitsInContext = model.create_entity("IfcUnitAssignment", Units=[
                        assigned["AREAUNIT"], assigned["VOLUMEUNIT"]])
                else:
                    project.UnitsInContext = model.create_entity("IfcUnitAssignment", Units=[
                        assigned["LENGTHUNIT"], assigned["AREAUNIT"],
                        assigned["AREAUNIT"], assigned["VOLUMEUNIT"]])
                path = Path(temp) / "model.ifc"
                model.write(str(path))
                with self.assertRaises(verify.ArtifactError):
                    verify.inspect(path, item)

    def test_site_world_transform_must_match_canonical_origin(self):
        from archy_ifc_worker import hierarchy, verify
        item = request()
        with tempfile.TemporaryDirectory() as temp:
            model = hierarchy.build(item)
            site = model.by_type("IfcSite")[0]
            building = model.by_type("IfcBuilding")[0]
            site.ObjectPlacement.RelativePlacement.Location.Coordinates = (1., 0., 0.)
            building.ObjectPlacement.RelativePlacement.Location.Coordinates = (
                item["variant"]["parameters"]["origin_xy_m"][0] - 1.,
                item["variant"]["parameters"]["origin_xy_m"][1], 0.)
            path = Path(temp) / "model.ifc"
            model.write(str(path))
            with self.assertRaisesRegex(verify.ArtifactError, "site placement"):
                verify.inspect(path, item)

    def test_fixed_module_cli_writes_reopen_verified_artifacts(self):
        item = request()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "runs/run-1").mkdir(parents=True)
            source = root / "request.json"
            source.write_text(json.dumps(item))
            run = subprocess.run([sys.executable, "-I", "-m", "archy_ifc_worker",
                                  "--project-root", str(root), "--request", str(source)],
                                 text=True, capture_output=True)
            self.assertEqual(run.returncode, 0, run.stderr + run.stdout)
            output = root / item["output_dir"]
            response = json.loads((output / "response.json").read_text())
            self.assertEqual(response["status"], "ok")
            self.assertTrue(response["validation"]["fresh_process_reopen"])
            self.assertEqual(response["validation"]["express_validation"]["exit_code"], 0)
            self.assertEqual([a["path"] for a in response["artifacts"]], ["model.ifc", "validation.json"])
            for artifact in response["artifacts"]:
                data = (output / artifact["path"]).read_bytes()
                self.assertEqual(hashlib.sha256(data).hexdigest(), artifact["sha256"])
                self.assertEqual(len(data), artifact["size_bytes"])

    def test_analytic_rectangle_and_translated_l(self):
        from archy_ifc_worker import hierarchy, verify, protocol
        cases = json.loads((ROOT / "tests/fixtures/analytic/courtyard-shapes.json").read_text())["cases"]
        for case in cases:
            with self.subTest(shape=case["shape"]), tempfile.TemporaryDirectory() as temp:
                item = request()
                v = item["variant"]
                p = {"shape": case["shape"], "origin_xy_m": [case["origin_xy_m"][0] + 10,
                     case["origin_xy_m"][1] - 5], "width_m": case["width_m"],
                     "depth_m": case["depth_m"]}
                if case["shape"] == "l_shape":
                    p.update(notch_width_m=case["notch_width_m"], notch_depth_m=case["notch_depth_m"])
                v["parameters"] = p
                v["shape"] = case["shape"]
                v["footprint_xy_m"] = [[x + 10, y - 5] for x, y in case["footprint_xy_m"]]
                v["id"] = item["variant_id"] = protocol.canonical_id(v)
                protocol.validate_request(item)
                path = Path(temp) / "model.ifc"
                hierarchy.build(item).write(str(path))
                report = verify.inspect(path, item)
                self.assertAlmostEqual(report["footprint_area_m2"], case["footprint_area_m2"])
                self.assertAlmostEqual(report["approximate_gfa_m2"], case["gross_floor_area_m2"])
                self.assertAlmostEqual(report["height_m"], 9.6)

    def test_task02_l_shape_hierarchy_and_draft(self):
        from archy_ifc_worker import hierarchy, verify
        item = request()
        model = hierarchy.build(item)
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "model.ifc"
            model.write(str(path))
            report = verify.inspect(path, item)
            self.assertEqual(report["status"], "ok")
            self.assertEqual(report["storeys"], 3)
            self.assertAlmostEqual(report["height_m"], 9.6)
            self.assertAlmostEqual(report["footprint_area_m2"], report["expected_footprint_area_m2"], places=5)
            reopened = ifcopenshell.open(str(path))
            from ifcopenshell.util import element
            props = element.get_psets(reopened.by_type("IfcBuilding")[0])["Archy_SchematicMassing"]
            self.assertEqual(props["ApprovalState"], "DRAFT")
            self.assertIn("ExportedAtUTC", props)
            self.assertEqual(len(reopened.by_type("IfcBuildingElementProxy")), 3)
            self.assertEqual(len(reopened.by_type("IfcRelContainedInSpatialStructure")), 3)
            self.assertEqual(len({e.GlobalId for e in reopened.by_type("IfcRoot")}), len(reopened.by_type("IfcRoot")))
            self.assertEqual([s.Elevation for s in reopened.by_type("IfcBuildingStorey")], [0.0, 3.2, 6.4])

    def test_stable_guids_across_run_ids(self):
        from archy_ifc_worker import hierarchy
        a = request(); b = copy.deepcopy(a); b["run_id"] = "run-2"
        def guids(item):
            model = hierarchy.build(item)
            return sorted((e.is_a(), e.GlobalId) for e in model.by_type("IfcRoot"))
        self.assertEqual(guids(a), guids(b))

    def test_relation_guids_follow_endpoints_and_property_roles_not_creation_order(self):
        from archy_ifc_worker import hierarchy
        item = request()
        model = hierarchy.build(item)
        contained = model.by_type("IfcRelContainedInSpatialStructure")
        properties = model.by_type("IfcRelDefinesByProperties")

        def inventory():
            return {
                ("contains", rel.RelatingStructure.Name, tuple(sorted(x.Name for x in rel.RelatedElements))): rel.GlobalId
                for rel in model.by_type("IfcRelContainedInSpatialStructure")
            } | {
                ("properties", rel.RelatingPropertyDefinition.Name,
                 tuple(sorted(x.Name for x in rel.RelatedObjects))): rel.GlobalId
                for rel in model.by_type("IfcRelDefinesByProperties")
            }

        before = inventory()
        contents = [(rel.RelatingStructure, tuple(rel.RelatedElements)) for rel in contained]
        property_links = [(rel.RelatingPropertyDefinition, tuple(rel.RelatedObjects)) for rel in properties]
        for rel in [*contained, *properties]:
            model.remove(rel)
        for storey, elements in reversed(contents):
            model.create_entity("IfcRelContainedInSpatialStructure",
                                RelatingStructure=storey, RelatedElements=elements)
        for definition, objects in reversed(property_links):
            model.create_entity("IfcRelDefinesByProperties",
                                RelatingPropertyDefinition=definition, RelatedObjects=objects)
        hierarchy.assign_stable_guids(model, item)
        self.assertEqual(inventory(), before)
        changed = model.by_type("IfcRelContainedInSpatialStructure")[0]
        old_id = changed.GlobalId
        changed.RelatingStructure = model.by_type("IfcBuildingStorey")[0]
        hierarchy.assign_stable_guids(model, item)
        self.assertNotEqual(changed.GlobalId, old_id)

    def test_two_repeated_exports_have_same_semantic_inventory(self):
        from archy_ifc_worker import hierarchy, verify
        item = request()
        reports = []
        inventories = []
        with tempfile.TemporaryDirectory() as temp:
            for i in range(2):
                path = Path(temp) / f"model-{i}.ifc"
                hierarchy.build(item).write(str(path))
                reports.append(verify.inspect(path, item))
                reopened = ifcopenshell.open(str(path))
                inventories.append(sorted((root.is_a(), root.Name or "", root.GlobalId)
                                          for root in reopened.by_type("IfcRoot")))
        self.assertEqual(reports[0], reports[1])
        self.assertEqual(inventories[0], inventories[1])

    def test_corrupt_and_wrong_unit_are_rejected(self):
        from archy_ifc_worker import hierarchy, verify
        item = request()
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "model.ifc"
            path.write_text("broken")
            with self.assertRaises(verify.ArtifactError):
                verify.inspect(path, item)

    def test_wrong_placement_and_quantity_are_rejected(self):
        from archy_ifc_worker import hierarchy, verify
        item = request()
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "model.ifc"
            model = hierarchy.build(item)
            model.by_type("IfcBuildingStorey")[1].ObjectPlacement.RelativePlacement.Location.Coordinates = (0., 0., 40.)
            model.write(str(path))
            with self.assertRaises(verify.ArtifactError):
                verify.inspect(path, item)
            model = hierarchy.build(item)
            points = model.by_type("IfcCartesianPointList2D")[0]
            changed = list(points.CoordList)
            changed[3] = (changed[3][0] + 0.5, changed[3][1])
            points.CoordList = changed
            model.write(str(path))
            with self.assertRaises(verify.ArtifactError):
                verify.inspect(path, item)
            model = hierarchy.build(item)
            model.by_type("IfcQuantityArea")[0].AreaValue = 999.
            model.write(str(path))
            with self.assertRaises(verify.ArtifactError):
                verify.inspect(path, item)
            model = hierarchy.build(item)
            model.by_type("IfcQuantityArea")[0].Unit = next(
                u for u in model.by_type("IfcSIUnit") if u.UnitType == "VOLUMEUNIT")
            model.write(str(path))
            with self.assertRaises(verify.ArtifactError):
                verify.inspect(path, item)
            model = hierarchy.build(item)
            model.by_type("IfcSIUnit")[0].Prefix = "MILLI"
            model.write(str(path))
            with self.assertRaises(verify.ArtifactError):
                verify.inspect(path, item)
            model = hierarchy.build(item)
            model.by_type("IfcProject")[0].GlobalId = "invalid"
            model.write(str(path))
            with self.assertRaises(verify.ArtifactError):
                verify.inspect(path, item)


class OutputTests(unittest.TestCase):
    def test_occupied_and_symlink_destination(self):
        from archy_ifc_worker.worker import prepare_output
        from archy_ifc_worker.protocol import ProtocolError
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "runs").mkdir()
            (root / "runs" / "run-1").mkdir()
            (root / "runs" / "run-1" / "ifc").mkdir()
            with self.assertRaises(ProtocolError):
                prepare_output(root, "runs/run-1/ifc")
            (root / "runs" / "run-1" / "ifc").rmdir()
            sentinel = root / "sentinel.txt"
            sentinel.write_text("unchanged")
            (root / "runs" / "run-1" / "ifc").symlink_to(root)
            with self.assertRaises(ProtocolError):
                prepare_output(root, "runs/run-1/ifc")
            self.assertEqual(sentinel.read_text(), "unchanged")


if __name__ == "__main__":
    unittest.main()
