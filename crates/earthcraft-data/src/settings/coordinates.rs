//! Tolerant persistence for display preferences, separate from stored geography.
use earthcraft_core::coordinates::{CoordinateDisplay, CoordinateFormat};
use serde::{Serializer, ser::SerializeStruct};
pub(super) fn serialize<S: Serializer>(
    display: &CoordinateDisplay,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut state = serializer.serialize_struct("CoordinateDisplay", 2)?;
    state.serialize_field("format", display.format.code())?;
    state.serialize_field("precision", &display.precision)?;
    state.end()
}
pub(super) fn apply(display: &mut CoordinateDisplay, value: &serde_json::Value) {
    if let Some(format) = value
        .get("format")
        .and_then(|v| v.as_str())
        .and_then(CoordinateFormat::from_code)
    {
        display.select_format(format);
    }
    if let Some(precision) = value.get("precision").and_then(|v| v.as_u64()) {
        let range = display.format.precision_range();
        display.precision =
            precision.clamp(u64::from(*range.start()), u64::from(*range.end())) as u8;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    #[test]
    fn coordinate_display_roundtrips_and_old_or_invalid_fields_keep_valid_preferences() {
        for format in CoordinateFormat::ALL {
            let display = CoordinateDisplay {
                format,
                precision: format.default_precision(),
            };
            let settings = Settings {
                coordinate_display: display,
                ..Settings::default()
            };
            let mut restored = Settings::default();
            restored
                .apply_json(&serde_json::to_vec(&settings).unwrap())
                .unwrap();
            assert_eq!(restored.coordinate_display, display);
            for bytes in [
                br#"{}"#.as_slice(),
                br#"{"coordinate_display":42}"#,
                br#"{"coordinate_display":{"format":"future","precision":"many"}}"#,
            ] {
                restored.apply_json(bytes).unwrap();
                assert_eq!(restored.coordinate_display, display);
            }
        }
        let mut restored = Settings::default();
        assert_eq!(restored.coordinate_display, CoordinateDisplay::default());
        restored
            .apply_json(br#"{"coordinate_display":{"format":"mgrs","precision":100}}"#)
            .unwrap();
        assert_eq!(restored.coordinate_display.precision, 5);
        restored
            .apply_json(br#"{"coordinate_display":{"precision":0}}"#)
            .unwrap();
        assert_eq!(restored.coordinate_display.precision, 1);
    }
}
