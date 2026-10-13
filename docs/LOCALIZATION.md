# Localization / Localização

EarthCraft embeds [English](../crates/earthcraft-core/locales/en.ftl) and
[Brazilian Portuguese](../crates/earthcraft-core/locales/pt-BR.ftl) Fluent catalogs.
Editing them requires rebuilding; no translation files need to be installed next
to the executable. The existing `lang.txt` preference still chooses the language.

Add the same semantic ID and named variables to both files:

```ftl
# en.ftl
search-going = Going to { $destination }

# pt-BR.ftl
search-going = Indo para { $destination }
```

Rust calls the language API in core:

```rust
let label = lang.text("sidebar-layers");
let status = lang.format("search-going", &[("destination", name.as_str().into())]);
```

Use `text` for parameterless labels and `format` for whole messages. Keep
punctuation and word order in Fluent rather than joining translated fragments.
Static labels are cached after first use; the shared concurrent bundles can also
be used by background KML imports. There is no runtime file I/O.

Pass counts and preformatted coordinates as strings when preserving the existing
ungrouped numbers and decimal precision. Numeric Fluent values and plural/select
expressions are available for deliberate wording improvements; review those as
UI changes. Fluent isolation is disabled for the current English/Portuguese
catalogs to retain existing output. Review bidirectional text handling before
introducing an RTL language.

Missing or unresolvable translations fall back to English. If neither locale can
resolve the message, the ID is displayed. Embedded syntax/duplicate-key mistakes
fail initialization; automated tests catch these before release. Catalog parity
is required even though runtime fallback exists. Native language names and
provider-supplied geographic names remain metadata. Existing service diagnostic
logs and source-error details retain their original text.

Run `cargo test -p earthcraft-core --locked`. Tests verify syntax, matching IDs
and variables, resolution, fallback, application/data references and exact
original labels and dynamic-message output. Regression fixtures are explained in
[their README](../crates/earthcraft-core/tests/fixtures/README.md). Do not update
expected output just to make a mechanical migration pass. Also switch languages
in the desktop app and inspect settings, help, search, file status and provider
attribution when those areas change.

## Português

Adicione as mesmas chaves e variáveis nos dois arquivos FTL. Use `Lang::text`
para rótulos estáticos e `Lang::format` para mensagens completas com argumentos.
Pontuação e ordem das palavras pertencem ao catálogo. Os arquivos ficam embutidos
no executável e exigem uma recompilação depois de editar.

Mantenha contagens e coordenadas como strings quando precisar conservar a
formatação original. Mensagens ausentes usam inglês como fallback; falhas nas duas
línguas mostram a chave. Os testes verificam sintaxe, chaves, variáveis, resolução
e os textos originais. Execute os testes de core e confira a troca de idioma na
interface. Nomes geográficos e diagnósticos existentes continuam vindos dos serviços.
