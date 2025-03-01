use json_writer::{JsonWriter, JsonWriterError};
use tedge_api::measurement::MeasurementVisitor;
use time::{format_description, OffsetDateTime};

pub struct C8yJsonSerializer {
    json: JsonWriter,
    is_within_group: bool,
    timestamp_present: bool,
    default_timestamp: OffsetDateTime,
}

#[derive(thiserror::Error, Debug)]
pub enum C8yJsonSerializationError {
    #[error(transparent)]
    MeasurementCollectorError(#[from] MeasurementStreamError),

    #[error(transparent)]
    JsonWriterError(#[from] JsonWriterError),
}

#[allow(clippy::enum_variant_names)]
#[derive(thiserror::Error, Debug, Eq, PartialEq)]
pub enum MeasurementStreamError {
    #[error("Unexpected time stamp within a group")]
    UnexpectedTimestamp,

    #[error("Unexpected end of data")]
    UnexpectedEndOfData,

    #[error("Unexpected end of group")]
    UnexpectedEndOfGroup,

    #[error("Unexpected start of group")]
    UnexpectedStartOfGroup,
}

impl C8yJsonSerializer {
    pub fn new(default_timestamp: OffsetDateTime, maybe_child_id: Option<&str>) -> Self {
        let capa = 1024; // XXX: Choose a capacity based on expected JSON length.
        let mut json = JsonWriter::with_capacity(capa);

        json.write_open_obj();
        let _ = json.write_key("type");
        let _ = json.write_str("ThinEdgeMeasurement");

        if let Some(child_id) = maybe_child_id {
            // In case the measurement is addressed to a child-device use fragment
            // "externalSource" to tell c8Y identity API to use child-device
            // object referenced by "externalId", instead of root device object
            // referenced by MQTT client's Device ID.
            let _ = json.write_key("externalSource");
            json.write_open_obj();
            let _ = json.write_key("externalId");
            let _ = json.write_str(child_id);
            let _ = json.write_key("type");
            let _ = json.write_str("c8y_Serial");
            json.write_close_obj();
        }

        Self {
            json,
            is_within_group: false,
            timestamp_present: false,
            default_timestamp,
        }
    }

    fn end(&mut self) -> Result<(), C8yJsonSerializationError> {
        if self.is_within_group {
            return Err(MeasurementStreamError::UnexpectedEndOfData.into());
        }

        if !self.timestamp_present {
            self.visit_timestamp(self.default_timestamp)?;
        }

        assert!(self.timestamp_present);

        self.json.write_close_obj();
        Ok(())
    }

    fn write_value_obj(&mut self, value: f64) -> Result<(), C8yJsonSerializationError> {
        self.json.write_open_obj();
        self.json.write_key("value")?;
        self.json.write_f64(value)?;
        self.json.write_close_obj();
        Ok(())
    }

    pub fn into_string(mut self) -> Result<String, C8yJsonSerializationError> {
        self.end()?;
        Ok(self.json.clone().into_string()?)
    }
}

impl MeasurementVisitor for C8yJsonSerializer {
    type Error = C8yJsonSerializationError;

    fn visit_timestamp(&mut self, timestamp: OffsetDateTime) -> Result<(), Self::Error> {
        if self.is_within_group {
            return Err(MeasurementStreamError::UnexpectedTimestamp.into());
        }

        self.json.write_key("time")?;
        self.json.write_str(
            timestamp
                .format(&format_description::well_known::Rfc3339)
                .unwrap()
                .as_str(),
        )?;

        self.timestamp_present = true;
        Ok(())
    }

    fn visit_measurement(&mut self, key: &str, value: f64) -> Result<(), Self::Error> {
        self.json.write_key(key)?;

        if self.is_within_group {
            self.write_value_obj(value)?;
        } else {
            self.json.write_open_obj();
            self.json.write_key(key)?;
            self.write_value_obj(value)?;
            self.json.write_close_obj();
        }
        Ok(())
    }

    fn visit_start_group(&mut self, group: &str) -> Result<(), Self::Error> {
        if self.is_within_group {
            return Err(MeasurementStreamError::UnexpectedStartOfGroup.into());
        }

        self.json.write_key(group)?;
        self.json.write_open_obj();
        self.is_within_group = true;
        Ok(())
    }

    fn visit_end_group(&mut self) -> Result<(), Self::Error> {
        if !self.is_within_group {
            return Err(MeasurementStreamError::UnexpectedEndOfGroup.into());
        }

        self.json.write_close_obj();
        self.is_within_group = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ::time::macros::datetime;
    use assert_json_diff::*;
    use assert_matches::*;
    use serde_json::json;

    use super::*;

    #[test]
    fn serialize_single_value_message() -> anyhow::Result<()> {
        let timestamp = datetime!(2021-06-22 17:03:14.123456789 +05:00);

        let mut serializer = C8yJsonSerializer::new(timestamp, None);
        serializer.visit_timestamp(timestamp)?;
        serializer.visit_measurement("temperature", 25.5)?;

        let output = serializer.into_string()?;

        let expected_output = json!({
            "type": "ThinEdgeMeasurement",
            "time": "2021-06-22T17:03:14.123456789+05:00",
            "temperature":{
                "temperature":{
                    "value": 25.5
                }
            }
        });

        assert_json_eq!(
            serde_json::from_str::<serde_json::Value>(&output)?,
            expected_output
        );
        Ok(())
    }
    #[test]
    fn serialize_multi_value_message() -> anyhow::Result<()> {
        let timestamp = datetime!(2021-06-22 17:03:14.123456789 +05:00);

        let mut serializer = C8yJsonSerializer::new(timestamp, None);
        serializer.visit_timestamp(timestamp)?;
        serializer.visit_measurement("temperature", 25.5)?;
        serializer.visit_start_group("location")?;
        serializer.visit_measurement("alti", 2100.4)?;
        serializer.visit_measurement("longi", 2200.4)?;
        serializer.visit_measurement("lati", 2300.4)?;
        serializer.visit_end_group()?;
        serializer.visit_measurement("pressure", 255.2)?;

        let output = serializer.into_string()?;

        let expected_output = json!({
            "type": "ThinEdgeMeasurement",
            "time": "2021-06-22T17:03:14.123456789+05:00",
            "temperature":{
                "temperature":{
                    "value": 25.5
                }
            },
             "location": {
                 "alti": {
                     "value": 2100.4
                 },
                 "longi":{
                     "value": 2200.4
                 },
                 "lati":{
                     "value": 2300.4
                 },
             },
             "pressure":{
                 "pressure":{
                     "value":255.2
                 }
             }

        });

        assert_json_eq!(
            serde_json::from_str::<serde_json::Value>(&output)?,
            expected_output
        );

        Ok(())
    }

    #[test]
    fn serialize_empty_message() -> anyhow::Result<()> {
        let timestamp = datetime!(2021-06-22 17:03:14.123456789 +05:00);

        let serializer = C8yJsonSerializer::new(timestamp, None);

        let expected_output =
            json!({"type": "ThinEdgeMeasurement", "time": "2021-06-22T17:03:14.123456789+05:00"});

        let output = serializer.into_string()?;

        assert_json_eq!(
            serde_json::from_str::<serde_json::Value>(&output)?,
            expected_output
        );

        Ok(())
    }

    #[test]
    fn serialize_timestamp_message() -> anyhow::Result<()> {
        let timestamp = datetime!(2021-06-22 17:03:14.123456789 +05:00);

        let mut serializer = C8yJsonSerializer::new(timestamp, None);
        serializer.visit_timestamp(timestamp)?;

        let expected_output = json!({
            "type": "ThinEdgeMeasurement",
            "time":"2021-06-22T17:03:14.123456789+05:00"
        });

        let output = serializer.into_string()?;

        assert_json_eq!(
            serde_json::from_str::<serde_json::Value>(&output)?,
            expected_output
        );

        Ok(())
    }

    #[test]
    fn serialize_timestamp_within_group() -> anyhow::Result<()> {
        let timestamp = datetime!(2021-06-22 17:03:14.123456789 +05:00);

        let mut serializer = C8yJsonSerializer::new(timestamp, None);
        serializer.visit_start_group("location")?;

        let expected_err = serializer.visit_timestamp(timestamp);

        assert_matches!(
            expected_err,
            Err(C8yJsonSerializationError::MeasurementCollectorError(
                MeasurementStreamError::UnexpectedTimestamp
            ))
        );
        Ok(())
    }

    #[test]
    fn serialize_unexpected_end_of_group() -> anyhow::Result<()> {
        let timestamp = datetime!(2021-06-22 17:03:14.123456789 +05:00);

        let mut serializer = C8yJsonSerializer::new(timestamp, None);
        serializer.visit_measurement("alti", 2100.4)?;
        serializer.visit_measurement("longi", 2200.4)?;

        let expected_err = serializer.visit_end_group();

        assert_matches!(
            expected_err,
            Err(C8yJsonSerializationError::MeasurementCollectorError(
                MeasurementStreamError::UnexpectedEndOfGroup
            ))
        );

        Ok(())
    }

    #[test]
    fn serialize_unexpected_start_of_group() -> anyhow::Result<()> {
        let timestamp = datetime!(2021-06-22 17:03:14.123456789 +05:00);

        let mut serializer = C8yJsonSerializer::new(timestamp, None);
        serializer.visit_start_group("location")?;
        serializer.visit_measurement("alti", 2100.4)?;
        serializer.visit_measurement("longi", 2200.4)?;

        let expected_err = serializer.visit_start_group("location2");

        assert_matches!(
            expected_err,
            Err(C8yJsonSerializationError::MeasurementCollectorError(
                MeasurementStreamError::UnexpectedStartOfGroup
            ))
        );

        Ok(())
    }

    #[test]
    fn serialize_unexpected_end_of_message() -> anyhow::Result<()> {
        let timestamp = datetime!(2021-06-22 17:03:14.123456789 +05:00);

        let mut serializer = C8yJsonSerializer::new(timestamp, None);
        serializer.visit_start_group("location")?;
        serializer.visit_measurement("alti", 2100.4)?;
        serializer.visit_measurement("longi", 2200.4)?;

        let expected_err = serializer.into_string();

        assert_matches!(
            expected_err,
            Err(C8yJsonSerializationError::MeasurementCollectorError(
                MeasurementStreamError::UnexpectedEndOfData
            ))
        );

        Ok(())
    }

    #[test]
    fn serialize_timestamp_child_message() -> anyhow::Result<()> {
        let timestamp = datetime!(2021-06-22 17:03:14.123456789 +05:00);

        let mut serializer = C8yJsonSerializer::new(timestamp, Some("child1"));
        serializer.visit_timestamp(timestamp)?;
        serializer.visit_measurement("temperature", 25.5)?;

        let expected_output = json!({
            "type": "ThinEdgeMeasurement",
            "time": "2021-06-22T17:03:14.123456789+05:00",
            "externalSource": {
                "externalId": "child1",
                "type": "c8y_Serial"
            },
            "temperature": {
                "temperature": {
                    "value": 25.5
                }
            }
        });

        let output = serializer.into_string()?;

        assert_json_eq!(
            serde_json::from_str::<serde_json::Value>(&output)?,
            expected_output
        );

        Ok(())
    }
}
