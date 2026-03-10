use miette::{IntoDiagnostic, SourceOffset};
use serde::{Serialize, de::DeserializeOwned};

use super::Format;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("malformed contents")]
#[diagnostic(help("try checking the syntax / schema of the configuration file"))]
pub struct DeserializationError {
    cause: String,
    #[source_code]
    input: String,
    #[label("{cause}")]
    location: SourceOffset,
}

impl DeserializationError {
    fn from_serde_json_error(input: impl Into<String>, cause: serde_json::Error) -> Self {
        let input = input.into();
        let location = SourceOffset::from_location(&input, cause.line(), cause.column());
        Self {
            cause: cause.to_string(),
            input,
            location,
        }
    }

    fn from_serde_yaml_error(input: impl Into<String>, cause: serde_yaml_ng::Error) -> Self {
        let input = input.into();
        let (loc_line, loc_col) = match cause.location() {
            Some(loc) => (loc.line(), loc.column()),
            None => (1, 1),
        };
        let location = SourceOffset::from_location(&input, loc_line, loc_col);
        Self {
            cause: cause.to_string(),
            input,
            location,
        }
    }
}

#[allow(unused)]
pub fn serialize_bytes<T: Serialize>(value: &T, format: Format) -> miette::Result<Vec<u8>> {
    match format {
        Format::Yaml => serde_yaml_ng::to_string(value)
            .map(|v| v.as_bytes().to_vec())
            .into_diagnostic(),
        Format::Json => serde_json::to_vec(value).into_diagnostic(),
    }
}

pub fn deserialize_bytes<T: DeserializeOwned>(
    contents: &[u8],
    format: Format,
) -> miette::Result<T> {
    Ok(match format {
        Format::Yaml => serde_yaml_ng::from_slice(contents).map_err(|cause| {
            DeserializationError::from_serde_yaml_error(
                std::str::from_utf8(contents).unwrap(),
                cause,
            )
        }),
        Format::Json => serde_json::from_slice(contents).map_err(|cause| {
            DeserializationError::from_serde_json_error(
                std::str::from_utf8(contents).unwrap(),
                cause,
            )
        }),
    }?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn test_serialize_deserialize() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct TestStruct {
            field1: String,
            field2: u32,
        }

        let original = TestStruct {
            field1: "test".to_string(),
            field2: 42,
        };

        let serialized = serialize_bytes(&original, Format::Yaml).unwrap();
        let deserialized: TestStruct = deserialize_bytes(&serialized, Format::Yaml).unwrap();
        assert_eq!(original, deserialized);

        let serialized = serialize_bytes(&original, Format::Json).unwrap();
        let deserialized: TestStruct = deserialize_bytes(&serialized, Format::Json).unwrap();
        assert_eq!(original, deserialized);
    }
}
