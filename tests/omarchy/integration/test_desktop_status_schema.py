"""Focused published-schema checks for desktop summary operation rules."""

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[3]
SCHEMA = json.loads((ROOT / "schemas/desktop-status.schema.json").read_text())
ACCEPTED = json.loads((ROOT / "tests/fixtures/desktop/accepted_status.json").read_text())
RENDER = json.loads((ROOT / "tests/fixtures/desktop/render_status.json").read_text())
LARGE_RENDER = json.loads((ROOT / "tests/fixtures/desktop/render_status_90_variants.json").read_text())
VALIDATOR = Draft202012Validator(SCHEMA)


class DesktopStatusSchemaTest(unittest.TestCase):
    def test_real_status_fixtures(self):
        Draft202012Validator.check_schema(SCHEMA)
        for value in (ACCEPTED, RENDER, LARGE_RENDER):
            self.assertTrue(VALIDATOR.is_valid(value))
        self.assertEqual(LARGE_RENDER["selected_run"]["summary"]["artifact_count"], 274)
        self.assertEqual(len(LARGE_RENDER["selected_run"]["artifacts"]), 274)

    def test_non_render_disclosure_and_render_outcome_are_rejected(self):
        for field, value in (("preview_symbols", []), ("render_outcome", "verified")):
            changed = copy.deepcopy(ACCEPTED)
            changed["runs"][0][field] = value
            self.assertFalse(VALIDATOR.is_valid(changed), field)

    def test_render_requires_real_outcome_and_accepts_disclosure(self):
        for value in (None, "invented"):
            changed = copy.deepcopy(RENDER)
            changed["runs"][0]["render_outcome"] = value
            self.assertFalse(VALIDATOR.is_valid(changed))
        changed = copy.deepcopy(RENDER)
        del changed["runs"][0]["render_outcome"]
        self.assertFalse(VALIDATOR.is_valid(changed))
        changed = copy.deepcopy(RENDER)
        changed["runs"][0]["preview_symbols"] = []
        self.assertTrue(VALIDATOR.is_valid(changed))

    def test_cross_field_cardinality_is_documented_for_desktop_reader(self):
        comment = SCHEMA["$defs"]["DesktopRunSummary"]["$comment"]
        self.assertIn("preview_symbols length must not exceed variant_count", comment)
        changed = copy.deepcopy(RENDER)
        changed["runs"][0]["variant_count"] = 0
        changed["runs"][0]["preview_symbols"] = [{
            "variant_id": "variant-" + "a" * 64,
            "views": ["top"],
            "notice": "Screen-width massing edges are nonphysical preview symbols; GLB dimensions remain exact.",
        }]
        self.assertTrue(VALIDATOR.is_valid(changed))


if __name__ == "__main__":
    unittest.main()
