# Localization regression fixtures

Captured from the inline English/Portuguese pairs at commit `4303123`, before
introducing Fluent. `original-ui-strings.json` preserves all 82 static labels,
including whitespace, punctuation, emoji and help-text line breaks.
`original-ui-messages.json` preserves the output of the original Rust format
strings for the 17 messages now using Fluent arguments. Counts are passed as
strings to retain the original ungrouped output; coordinates retain their
original precision. Expected output was assembled from the original prefixes
and format strings, independently of the new FTL catalogs.

Lookup IDs were subsequently renamed to feature/role names. Only fixture keys
changed; the captured translations, arguments and expected output are unchanged.

Update expected text only when deliberately changing UI wording, with both languages
reviewed. Adding a catalog entry also requires matching keys and variables in
both locales; the catalog tests check syntax and resolution independently.
