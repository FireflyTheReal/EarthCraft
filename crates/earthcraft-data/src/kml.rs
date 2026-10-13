use earthcraft_core::{language::Lang, placemark::Placemark};
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn xml_unescape(s: &str) -> String {
    let s = s.trim();
    let s = s
        .strip_prefix("<![CDATA[")
        .and_then(|x| x.strip_suffix("]]>"))
        .unwrap_or(s);
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}
fn between<'a>(s: &'a str, a: &str, b: &str) -> Option<&'a str> {
    let i = s.find(a)? + a.len();
    let j = s[i..].find(b)? + i;
    Some(&s[i..j])
}
pub fn save_kml(path: &str, pms: &[Placemark]) -> std::io::Result<()> {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n<Document>\n<name>EarthCraft</name>\n",
    );
    for p in pms {
        s += &format!(
            "<Placemark><name>{}</name><description>{}</description><Point><coordinates>{},{},0</coordinates></Point></Placemark>\n",
            xml_escape(&p.name),
            xml_escape(&p.desc),
            p.lon,
            p.lat
        );
    }
    s += "</Document>\n</kml>\n";
    std::fs::write(path, s)
}
pub fn load_kml(path: &str, lg: Lang) -> Result<Vec<Placemark>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut rest = txt.as_str();
    while let Some(i) = rest.find("<Placemark") {
        let after = &rest[i..];
        let end = after
            .find("</Placemark>")
            .map(|e| e + 12)
            .unwrap_or(after.len());
        let block = &after[..end];
        if let Some(c) = between(block, "<coordinates>", "</coordinates>") {
            let nums: Vec<f64> = c
                .trim()
                .split(',')
                .filter_map(|x| x.trim().parse().ok())
                .collect();
            if nums.len() >= 2 {
                out.push(Placemark {
                    name: between(block, "<name>", "</name>")
                        .map(xml_unescape)
                        .unwrap_or_else(|| lg.pick("Unnamed", "Sem nome").into()),
                    desc: between(block, "<description>", "</description>")
                        .map(xml_unescape)
                        .unwrap_or_default(),
                    lat: nums[1],
                    lon: nums[0],
                });
            }
        }
        rest = &after[end..];
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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
