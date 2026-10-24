//! Hosted-only request decoding with bounds before allocating an image array.
use crate::models::SearchQuery;
use serde::{
    de::{self, IgnoredAny, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use std::fmt;

#[derive(Default, Deserialize)]
#[serde(default)]
struct Input {
    title: String,
    #[serde(rename = "type")]
    qtype: String,
    question: String,
    options: String,
    #[serde(deserialize_with = "images")]
    images: Vec<String>,
    suggestion_title: String,
    suggestion_options: String,
}

fn images<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    struct Images;
    impl<'de> Visitor<'de> for Images {
        type Value = Vec<String>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("at most eight bounded image data URLs")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
            let mut values = Vec::with_capacity(8);
            while values.len() < 8 {
                let Some(value) = sequence.next_element::<String>()? else {
                    return Ok(values);
                };
                if value.len() > 2 * 1024 * 1024 + 128 {
                    return Err(de::Error::custom("image is too large"));
                }
                values.push(value);
            }
            if sequence.next_element::<IgnoredAny>()?.is_some() {
                return Err(de::Error::custom("too many images"));
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Images)
}

pub(super) fn parse(bytes: &[u8]) -> Result<SearchQuery, serde_json::Error> {
    let value: Input = serde_json::from_slice(bytes)?;
    Ok(SearchQuery {
        title: value.title,
        qtype: value.qtype,
        question: value.question,
        options: value.options,
        images: value.images,
        suggestion_title: value.suggestion_title,
        suggestion_options: value.suggestion_options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_arrays_are_bounded_during_decode() {
        assert!(parse(br#"{"title":"test","images":[]}"#)
            .unwrap()
            .images
            .is_empty());
        let eight = serde_json::json!({"images":vec!["data:image/png;base64,AA==";8]});
        assert_eq!(
            parse(&serde_json::to_vec(&eight).unwrap())
                .unwrap()
                .images
                .len(),
            8
        );
        let excess = serde_json::json!({"images":vec!["";50_000]});
        assert!(parse(&serde_json::to_vec(&excess).unwrap()).is_err());
        let huge = serde_json::json!({"images":["x".repeat(2*1024*1024+129)]});
        assert!(parse(&serde_json::to_vec(&huge).unwrap()).is_err());
    }
}
