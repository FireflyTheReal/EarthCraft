"""Pre-bake one Overture Divisions country/bbox slice for the optional metro layer.

Overture is ODbL 1.0 and is not downloaded by the app. Generate a local GeoJSON file
with DuckDB, then enable "Metropolitan regions" in EarthCraft:

    python3 scripts/prebake_overture_divisions.py --iso3 USA --subtype locality
    python3 scripts/prebake_overture_divisions.py --iso3 FRA --bbox 2.2,48.7,2.5,48.9

The default output is ~/.cache/earthcraft_tiles/overture/<ISO3>.geojson (the app's
cache directory). Requires `pip install duckdb`; the DuckDB spatial/httpfs extensions
are installed on first use.
"""
import argparse
import json
import os
from pathlib import Path
import sys

DEFAULT_RELEASE = "2026-09-23.1"
S3_PREFIX = "s3://overturemaps-us-west-2/release"
ISO2 = {
    "USA": "US",
    "CAN": "CA",
    "FRA": "FR",
    "DEU": "DE",
    "ESP": "ES",
    "MEX": "MX",
    "BRA": "BR",
    "GBR": "GB",
    "ITA": "IT",
    "NLD": "NL",
}


def cache_dir():
    """Keep this in sync with crates/earthcraft-data/src/paths.rs."""
    base = os.environ.get("LOCALAPPDATA") or os.environ.get("XDG_CACHE_HOME")
    if not base:
        base = str(Path.home() / ".cache")
    return Path(base) / "earthcraft_tiles"


def output_path(iso3, path=None):
    if path:
        return Path(path)
    return cache_dir() / "overture" / f"{iso3.upper()}.geojson"


def iso2(iso3):
    code = iso3.upper()
    if code not in ISO2:
        raise ValueError(f"No ISO2 mapping for {code}; pass --iso2 explicitly")
    return ISO2[code]


def parse_bbox(value):
    if not value:
        return None
    parts = [float(part) for part in value.split(",")]
    if len(parts) != 4:
        raise ValueError("--bbox needs min_lon,min_lat,max_lon,max_lat")
    min_lon, min_lat, max_lon, max_lat = parts
    if not (min_lon < max_lon and min_lat < max_lat):
        raise ValueError("--bbox min values must be smaller than max values")
    return parts


def query(release, country, subtype, bbox, simplify):
    source = f"{S3_PREFIX}/{release}/theme=divisions/type=division_area/*.parquet"
    params = [simplify] if simplify > 0 else []
    where = ["country = ?"]
    params.append(country)
    if subtype:
        where.append("subtype = ?")
        params.append(subtype)
    if bbox:
        where.append("bbox.xmin <= ? AND bbox.xmax >= ? AND bbox.ymin <= ? AND bbox.ymax >= ?")
        params.extend([bbox[2], bbox[0], bbox[3], bbox[1]])
    simplify_sql = "ST_SimplifyPreserveTopology(geometry, ?)" if simplify > 0 else "geometry"
    sql = (
        "SELECT names.primary, ST_AsGeoJSON("
        + simplify_sql
        + ") FROM read_parquet('"
        + source
        + "', hive_partitioning = 1) WHERE "
        + " AND ".join(where)
        + " AND names.primary IS NOT NULL"
    )
    return sql, params


def write_slice(connection, sql, params, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    cursor = connection.execute(sql, params)
    with destination.open("w", encoding="utf-8") as output:
        output.write('{"type":"FeatureCollection","features":[')
        first = True
        while True:
            rows = cursor.fetchmany(1000)
            if not rows:
                break
            for name, geometry in rows:
                if not geometry:
                    continue
                if not first:
                    output.write(",")
                first = False
                feature = {
                    "type": "Feature",
                    "properties": {"name": name},
                    "geometry": json.loads(geometry),
                }
                output.write(json.dumps(feature, ensure_ascii=False, separators=(",", ":")))
        output.write("]}\n")
    return not first


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--iso3", required=True, help="Country ISO3, e.g. USA")
    parser.add_argument("--iso2", help="Override Overture's ISO2 country code")
    parser.add_argument("--subtype", default="locality", help="Overture subtype (locality, county, ...)")
    parser.add_argument("--bbox", help="min_lon,min_lat,max_lon,max_lat")
    parser.add_argument("--release", default=DEFAULT_RELEASE)
    parser.add_argument("--output", help="Output GeoJSON path")
    parser.add_argument("--simplify", type=float, default=0.001, help="Degrees; 0 keeps full geometry")
    args = parser.parse_args(argv)

    try:
        country = args.iso2 or iso2(args.iso3)
        bbox = parse_bbox(args.bbox)
        sql, params = query(args.release, country, args.subtype, bbox, args.simplify)
    except ValueError as error:
        parser.error(str(error))

    try:
        import duckdb
    except ImportError:
        print("Missing dependency: pip install duckdb", file=sys.stderr)
        return 2

    connection = duckdb.connect()
    connection.execute("INSTALL httpfs; LOAD httpfs; INSTALL spatial; LOAD spatial;")
    connection.execute("SET s3_region='us-west-2';")
    destination = output_path(args.iso3, args.output)
    if not write_slice(connection, sql, params, destination):
        destination.unlink(missing_ok=True)
        print("No Overture divisions matched the filter.", file=sys.stderr)
        return 1
    print(destination)
    print("Attribution: Overture Maps Foundation, ODbL 1.0 (contains OSM-derived data).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
