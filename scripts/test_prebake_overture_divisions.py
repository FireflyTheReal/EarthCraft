"""Offline checks for the Overture slice pre-bake helper."""
from pathlib import Path
import tempfile
import unittest
import prebake_overture_divisions as overture


class OverturePrebakeTests(unittest.TestCase):
    def test_country_codes_and_output_path(self):
        self.assertEqual(overture.iso2("usa"), "US")
        self.assertEqual(overture.output_path("usa").name, "USA.geojson")
        self.assertEqual(overture.output_path("FRA", "/tmp/fr.geojson"), Path("/tmp/fr.geojson"))
        with self.assertRaises(ValueError):
            overture.iso2("ZZZ")

    def test_bbox_validation_and_query_filters(self):
        self.assertEqual(overture.parse_bbox("1,2,3,4"), [1.0, 2.0, 3.0, 4.0])
        with self.assertRaises(ValueError):
            overture.parse_bbox("4,2,3,1")
        sql, params = overture.query("release", "US", "locality", [1, 2, 3, 4], 0.001)
        self.assertIn("country = ?", sql)
        self.assertIn("subtype = ?", sql)
        self.assertIn("bbox.xmin <= ?", sql)
        self.assertIn("ST_SimplifyPreserveTopology", sql)
        self.assertEqual(params, [0.001, "US", "locality", 3, 1, 4, 2])

    def test_feature_collection_is_written_incrementally(self):
        class Cursor:
            def __init__(self):
                self.rows = [("A", '{"type":"Point","coordinates":[0,0]}'), ("B", None)]

            def fetchmany(self, _):
                rows, self.rows = self.rows, []
                return rows

        class Connection:
            def execute(self, _sql, _params):
                return Cursor()

        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "slice.geojson"
            self.assertTrue(overture.write_slice(Connection(), "sql", [], target))
            data = target.read_text()
            self.assertIn('"name":"A"', data)
            self.assertNotIn('"name":"B"', data)
            self.assertTrue(data.startswith('{"type":"FeatureCollection"'))


if __name__ == "__main__":
    unittest.main()
