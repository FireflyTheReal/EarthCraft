//! Embedded catalogs shared by the UI and background importers.
use crate::language::Lang;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue, concurrent::FluentBundle};
use fluent_syntax::ast::Entry;
use std::{collections::HashMap, sync::OnceLock};

const EN: &str = include_str!("../locales/en.ftl");
const PT: &str = include_str!("../locales/pt-BR.ftl");
struct Catalog {
    bundle: FluentBundle<FluentResource>,
    labels: HashMap<String, String>,
}
impl Catalog {
    fn new(code: &str, source: &str) -> Self {
        let resource = FluentResource::try_new(source.to_owned()).expect("valid embedded Fluent");
        let ids: Vec<String> = resource
            .entries()
            .filter_map(|entry| match entry {
                Entry::Message(message) => Some(message.id.name.to_owned()),
                _ => None,
            })
            .collect();
        let mut bundle = FluentBundle::new_concurrent(vec![code.parse().expect("valid locale")]);
        // Format numeric strings and paths without inserting bidirectional isolation characters.
        bundle.set_use_isolating(false);
        bundle
            .add_resource(resource)
            .expect("unique embedded Fluent keys");
        let mut catalog = Self {
            bundle,
            labels: HashMap::new(),
        };
        for id in ids {
            // Parameterless messages are formatted once, rather than on every egui frame.
            if let Some(label) = catalog.format(&id, None) {
                catalog.labels.insert(id, label);
            }
        }
        catalog
    }
    fn format_or_fallback(
        &self,
        english: &Self,
        id: &str,
        args: Option<&FluentArgs<'_>>,
    ) -> Option<String> {
        self.format(id, args).or_else(|| english.format(id, args))
    }
    fn format(&self, id: &str, args: Option<&FluentArgs<'_>>) -> Option<String> {
        let pattern = self.bundle.get_message(id)?.value()?;
        let mut errors = Vec::new();
        let text = self.bundle.format_pattern(pattern, args, &mut errors);
        errors.is_empty().then(|| text.into_owned())
    }
}
fn catalogs() -> &'static [Catalog; 2] {
    static CATALOGS: OnceLock<[Catalog; 2]> = OnceLock::new();
    CATALOGS.get_or_init(|| [Catalog::new("en", EN), Catalog::new("pt-BR", PT)])
}
fn index(lang: Lang) -> usize {
    match lang {
        Lang::En => 0,
        Lang::PtBr => 1,
    }
}
pub(super) fn text(lang: Lang, id: &str) -> &str {
    let catalogs = catalogs();
    catalogs[index(lang)]
        .labels
        .get(id)
        .or_else(|| catalogs[0].labels.get(id))
        .map(String::as_str)
        .unwrap_or(id)
}
pub(super) fn format(lang: Lang, id: &str, values: &[(&str, FluentValue<'_>)]) -> String {
    let args: FluentArgs = values
        .iter()
        .map(|(key, value)| (*key, value.clone()))
        .collect();
    let catalogs = catalogs();
    catalogs[index(lang)]
        .format_or_fallback(&catalogs[0], id, Some(&args))
        .unwrap_or_else(|| id.to_owned())
}
#[cfg(test)]
mod tests;
