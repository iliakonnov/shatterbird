use std::str::FromStr;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub fn serialize<S: Serializer>(id: &gix_hash::ObjectId, s: S) -> Result<S::Ok, S::Error> {
    id.to_string().serialize(s)
}

pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<gix_hash::ObjectId, D::Error> {
    let s = String::deserialize(d)?;
    let parsed = gix_hash::ObjectId::from_str(&s).map_err(serde::de::Error::custom)?;
    Ok(parsed)
}
