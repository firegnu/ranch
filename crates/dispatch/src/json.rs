// From Saddle `plugins/dispatch/src/json.rs` at commit `c21674a`, unchanged.
//! Local ordered JSON; never enable serde_json's binary-wide preserve_order feature.
use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeMap};
use std::{collections::HashMap, fmt};

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub(super) enum Json {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Json>),
    Object(Object),
}
#[derive(Clone, Debug)]
pub(super) struct Object(pub Vec<(String, Json)>);
impl Serialize for Object {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}
impl Json {
    pub fn object(entries: impl IntoIterator<Item = (impl Into<String>, Self)>) -> Self {
        let mut pairs: Vec<(String, Self)> = vec![];
        for (key, value) in entries {
            let key = key.into();
            if let Some((_, old)) = pairs.iter_mut().find(|(k, _)| *k == key) {
                *old = value;
            } else {
                pairs.push((key, value));
            }
        }
        Self::Object(Object(pairs))
    }
    pub fn field(&self, key: &str) -> Result<&Self, &'static str> {
        self.entries()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
            .ok_or("missing field")
    }
    pub fn entries(&self) -> Result<&[(String, Self)], &'static str> {
        match self {
            Self::Object(v) => Ok(&v.0),
            _ => Err("expected object"),
        }
    }
    pub fn number(&self) -> Result<f64, &'static str> {
        match self {
            Self::Number(v) => v.as_f64().ok_or("expected finite number"),
            _ => Err("expected number"),
        }
    }
}
impl From<&str> for Json {
    fn from(s: &str) -> Self {
        Self::String(s.into())
    }
}
impl From<f64> for Json {
    fn from(v: f64) -> Self {
        Self::Number(serde_json::Number::from_f64(v).expect("finite JSON number"))
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Json;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON value")
            }
            fn visit_unit<E: de::Error>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Json, E> {
                Ok(Json::Bool(v))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Json, E> {
                Ok(Json::Number(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Json, E> {
                Ok(Json::Number(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Json, E> {
                serde_json::Number::from_f64(v)
                    .map(Json::Number)
                    .ok_or_else(|| E::custom("non-finite number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Json, E> {
                Ok(Json::from(v))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Json, E> {
                Ok(Json::String(v))
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut values = vec![];
                while let Some(v) = a.next_element()? {
                    values.push(v);
                }
                Ok(Json::Array(values))
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut pairs: Vec<(String, Json)> = vec![];
                let mut positions = HashMap::new();
                while let Some((k, v)) = a.next_entry::<String, Json>()? {
                    if let Some(&i) = positions.get(&k) {
                        pairs[i] = (k, v);
                    } else {
                        positions.insert(k.clone(), pairs.len());
                        pairs.push((k, v));
                    }
                }
                Ok(Json::Object(Object(pairs)))
            }
        }
        d.deserialize_any(Visitor)
    }
}
