use earthcraft_core::{language::Lang, placemark::Placemark};
use quick_xml::{
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
    Reader, Writer,
};

pub fn save_kml(path: &str, pms: &[Placemark]) -> std::io::Result<()> {
    let mut writer = Writer::new(Vec::new());
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    let mut kml = BytesStart::new("kml");
    kml.push_attribute(("xmlns", "http://www.opengis.net/kml/2.2"));
    writer.write_event(Event::Start(kml))?;
    writer.write_event(Event::Start(BytesStart::new("Document")))?;
    writer
        .create_element("name")
        .write_text_content(BytesText::new("EarthCraft"))?;
    for pm in pms {
        writer.write_event(Event::Start(BytesStart::new("Placemark")))?;
        writer
            .create_element("name")
            .write_text_content(BytesText::new(&pm.name))?;
        writer
            .create_element("description")
            .write_text_content(BytesText::new(&pm.desc))?;
        writer.write_event(Event::Start(BytesStart::new("Point")))?;
        writer
            .create_element("coordinates")
            .write_text_content(BytesText::new(&format!("{},{},0", pm.lon, pm.lat)))?;
        writer.write_event(Event::End(BytesEnd::new("Point")))?;
        writer.write_event(Event::End(BytesEnd::new("Placemark")))?;
    }
    writer.write_event(Event::End(BytesEnd::new("Document")))?;
    writer.write_event(Event::End(BytesEnd::new("kml")))?;
    std::fs::write(path, writer.into_inner())
}
#[derive(Default)]
struct Draft {
    name: Option<String>,
    desc: String,
    coordinates: Option<String>,
}
impl Draft {
    fn finish(self, lg: Lang) -> Option<Placemark> {
        let nums: Vec<f64> = self
            .coordinates?
            .trim()
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        (nums.len() >= 2).then(|| Placemark {
            name: self.name.unwrap_or_else(|| lg.text("kml-unnamed").into()),
            desc: self.desc,
            lat: nums[1],
            lon: nums[0],
        })
    }
}
fn parse_kml(txt: &str, lg: Lang) -> Result<Vec<Placemark>, crate::Error> {
    let mut reader = Reader::from_str(txt);
    // The legacy parser accepted documents whose enclosing tags were incomplete.
    reader.config_mut().check_end_names = false;
    let mut out = Vec::new();
    let mut draft: Option<Draft> = None;
    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.local_name().as_ref() == b"Placemark" => {
                draft = Some(Draft::default())
            }
            Event::Start(tag)
                if draft.is_some()
                    && matches!(
                        tag.local_name().as_ref(),
                        b"name" | b"description" | b"coordinates"
                    ) =>
            {
                let raw = reader.read_text(tag.name())?;
                let raw = raw.trim();
                let raw = raw
                    .strip_prefix("<![CDATA[")
                    .and_then(|s| s.strip_suffix("]]>"))
                    .unwrap_or(raw);
                let text = quick_xml::escape::unescape(raw)
                    .unwrap_or_else(|_| raw.into())
                    .into_owned();
                let d = draft.as_mut().expect("checked placemark state");
                match tag.local_name().as_ref() {
                    b"name" if d.name.is_none() => d.name = Some(text),
                    b"description" if d.desc.is_empty() => d.desc = text,
                    b"coordinates" if d.coordinates.is_none() => d.coordinates = Some(text),
                    _ => {}
                }
            }
            Event::End(tag) if tag.local_name().as_ref() == b"Placemark" => {
                if let Some(pm) = draft.take().and_then(|d| d.finish(lg)) {
                    out.push(pm);
                }
            }
            Event::Eof => {
                if let Some(pm) = draft.take().and_then(|d| d.finish(lg)) {
                    out.push(pm);
                }
                break;
            }
            _ => {}
        }
    }
    Ok(out)
}
pub fn load_kml(path: &str, lg: Lang) -> Result<Vec<Placemark>, crate::Error> {
    parse_kml(&std::fs::read_to_string(path)?, lg)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cdata_entities_fallback_names_and_incomplete_envelope() {
        let marks = parse_kml("<Placemark><description><![CDATA[A & B]]></description><Point><coordinates>-50,-15,0</coordinates></Point>", Lang::PtBr).unwrap();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].name, "Sem nome");
        assert_eq!(marks[0].desc, "A & B");
        let marks = parse_kml(
            "<Placemark><name>A &amp; B</name><coordinates>1,2</coordinates></Placemark>",
            Lang::En,
        )
        .unwrap();
        assert_eq!(marks[0].name, "A & B");
    }

    #[test]
    fn kml_roundtrip_preserves_names_descriptions_and_coordinate_order() {
        let path = std::env::temp_dir().join(format!(
            "earthcraft-kml-{}-{}.kml",
            std::process::id(),
            earthcraft_core::math::now_unix()
        ));
        let marks = [Placemark {
            name: "São Paulo & <home>".into(),
            desc: "A & B > C".into(),
            lat: -23.5,
            lon: -46.6,
        }];
        save_kml(path.to_str().unwrap(), &marks).unwrap();
        let loaded = load_kml(path.to_str().unwrap(), Lang::PtBr).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, marks[0].name);
        assert_eq!(loaded[0].desc, marks[0].desc);
        assert_eq!((loaded[0].lat, loaded[0].lon), (marks[0].lat, marks[0].lon));
    }
}
