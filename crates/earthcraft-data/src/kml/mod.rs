//! Read and write EarthCraft placemarks through GeoRust KML.
use ::kml::{
    types::{Coord, Element, Geometry, KmlDocument, KmlVersion, Placemark as KmlPlacemark, Point},
    Kml, KmlReader, KmlWriter,
};
use earthcraft_core::{language::Lang, placemark::Placemark};
use quick_xml::{
    events::{BytesDecl, Event},
    Writer,
};

mod guard;

pub fn save_kml(path: &str, pms: &[Placemark]) -> Result<(), crate::Error> {
    let marks = pms.iter().map(|pm| {
        Kml::Placemark(KmlPlacemark {
            name: Some(pm.name.clone()),
            description: Some(pm.desc.clone()),
            geometry: Some(Geometry::Point(Point::new(pm.lon, pm.lat, Some(0.0)))),
            ..Default::default()
        })
    });
    let name = Kml::Element(Element {
        name: "name".into(),
        content: Some("EarthCraft".into()),
        ..Default::default()
    });
    let document = Kml::KmlDocument(KmlDocument {
        version: KmlVersion::V22,
        attrs: [("xmlns".into(), "http://www.opengis.net/kml/2.2".into())].into(),
        elements: vec![Kml::Document {
            attrs: Default::default(),
            elements: std::iter::once(name).chain(marks).collect(),
        }],
    });
    // Build the full output before touching the destination, as the old writer did.
    let mut bytes = Vec::new();
    let mut xml = Writer::new(&mut bytes);
    xml.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    KmlWriter::new(xml).write(&document)?;
    Ok(std::fs::write(path, bytes)?)
}

fn collect(document: Kml<f64>, lg: Lang, out: &mut Vec<Placemark>) {
    match document {
        Kml::KmlDocument(doc) => {
            for element in doc.elements {
                collect(element, lg, out);
            }
        }
        Kml::Document { elements, .. } | Kml::Folder(::kml::types::Folder { elements, .. }) => {
            for element in elements {
                collect(element, lg, out);
            }
        }
        Kml::Placemark(pm) => {
            if let Some(coord) = pm.geometry.as_ref().and_then(first_coordinate) {
                out.push(Placemark {
                    name: pm
                        .name
                        .map(|s| s.trim().to_owned())
                        .unwrap_or_else(|| lg.text("placemarks-fallback-name").into()),
                    desc: pm.description.unwrap_or_default().trim().to_owned(),
                    lat: coord.y,
                    lon: coord.x,
                });
            }
        }
        _ => {}
    }
}

// EarthCraft imports one marker per Placemark, at its first coordinate. Decoding
// more KML geometry types does not add polygon/line rendering to the application.
fn first_coordinate(geometry: &Geometry<f64>) -> Option<Coord<f64>> {
    match geometry {
        Geometry::Point(point) => Some(point.coord),
        Geometry::LineString(line) => line.coords.first().copied(),
        Geometry::LinearRing(ring) => ring.coords.first().copied(),
        Geometry::Polygon(polygon) => polygon.outer.coords.first().copied(),
        Geometry::MultiGeometry(collection) => {
            collection.geometries.iter().find_map(first_coordinate)
        }
        _ => None,
    }
}

fn parse_kml(txt: &str, lg: Lang) -> Result<Vec<Placemark>, crate::Error> {
    let xml = guard::prepare(txt)?;
    let document = match KmlReader::<_, f64>::from_reader(xml.as_slice()).read() {
        Ok(document) => document,
        Err(::kml::Error::NoElements) => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut out = Vec::new();
    collect(document, lg, &mut out);
    Ok(out)
}

pub fn load_kml(path: &str, lg: Lang) -> Result<Vec<Placemark>, crate::Error> {
    parse_kml(&std::fs::read_to_string(path)?, lg)
}

#[cfg(test)]
mod tests;
