// SPDX-License-Identifier: MIT
//! Bounded object-field comparison, following Config Doctor's object leaves and
//! JSON Pointer escaping. Arrays and structural type changes are whole values.
use serde::{
    Deserialize, Serialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
pub const MAX_JSON_MERGE_BYTES: usize = 256 * 1024;
const MAX_FIELDS: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonMergeSide {
    Local,
    Upstream,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JsonFieldValue {
    pub exists: bool,
    pub value: Value,
}
impl JsonFieldValue {
    fn new(value: Option<&Value>) -> Self {
        Self {
            exists: value.is_some(),
            value: value.cloned().unwrap_or(Value::Null),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JsonMergeField {
    pub pointer: String,
    pub base: JsonFieldValue,
    pub local: JsonFieldValue,
    pub upstream: JsonFieldValue,
    pub conflict: bool,
    pub suggested: JsonMergeSide,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JsonConfigPreview {
    pub path: String,
    pub fields: Vec<JsonMergeField>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JsonFieldChoice {
    pub pointer: String,
    pub side: JsonMergeSide,
}
#[derive(Clone, Debug)]
pub struct JsonMergeDocument {
    pub preview: JsonConfigPreview,
    upstream: Value,
    newline: &'static str,
    bom: bool,
}

/// Strict JSON rejects duplicate object keys instead of silently losing a value.
struct Strict(Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Strict;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("unambiguous JSON")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Strict, E> {
                Ok(Strict(Value::Bool(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Strict, E> {
                Ok(Strict(Value::Number(v.into())))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Strict, E> {
                Ok(Strict(Value::Number(v.into())))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Strict, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Strict(Value::Number(n)))
                    .ok_or_else(|| E::custom("Invalid number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Strict, E> {
                Ok(Strict(Value::String(v.into())))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Strict, E> {
                Ok(Strict(Value::String(v)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Strict, A::Error> {
                let mut values = Vec::new();
                while let Some(Strict(v)) = seq.next_element()? {
                    values.push(v);
                }
                Ok(Strict(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Strict, A::Error> {
                let mut values = Map::new();
                while let Some((key, Strict(value))) = map.next_entry::<String, Strict>()? {
                    if values.insert(key, value).is_some() {
                        return Err(de::Error::custom("Duplicate JSON key"));
                    }
                }
                Ok(Strict(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(V)
    }
}
fn parse(bytes: &[u8]) -> Option<Value> {
    if bytes.len() > MAX_JSON_MERGE_BYTES {
        return None;
    }
    let text = std::str::from_utf8(bytes)
        .ok()?
        .trim_start_matches('\u{feff}');
    let Strict(value) = serde_json::from_str::<Strict>(text).ok()?;
    value.is_object().then_some(value)
}
fn pointer_part(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}
fn walk(
    pointer: &str,
    base: Option<&Value>,
    local: Option<&Value>,
    upstream: Option<&Value>,
    fields: &mut Vec<JsonMergeField>,
    depth: usize,
) -> Option<()> {
    if depth > 32 || fields.len() > MAX_FIELDS {
        return None;
    }
    if base == local && base == upstream {
        return Some(());
    }
    let objects = (
        base.and_then(Value::as_object),
        local.and_then(Value::as_object),
        upstream.and_then(Value::as_object),
    );
    if let (b, Some(l), Some(u)) = objects
        && (b.is_some() || base.is_none())
    {
        let keys: BTreeSet<_> = b
            .into_iter()
            .flat_map(|v| v.keys())
            .chain(l.keys())
            .chain(u.keys())
            .collect();
        for key in keys {
            walk(
                &format!("{pointer}/{}", pointer_part(key)),
                b.and_then(|v| v.get(key)),
                l.get(key),
                u.get(key),
                fields,
                depth + 1,
            )?;
        }
        return Some(());
    }
    fields.push(JsonMergeField {
        pointer: pointer.into(),
        base: JsonFieldValue::new(base),
        local: JsonFieldValue::new(local),
        upstream: JsonFieldValue::new(upstream),
        conflict: local != base && upstream != base && local != upstream,
        suggested: if local == base {
            JsonMergeSide::Upstream
        } else {
            JsonMergeSide::Local
        },
    });
    Some(())
}
pub fn preview_json_merge(
    path: &str,
    base: &[u8],
    local: &[u8],
    upstream: &[u8],
) -> Option<JsonMergeDocument> {
    if !path.ends_with(".json") || cobble_store::SafeRelativePath::parse(path).is_err() {
        return None;
    }
    let (b, l, u) = (parse(base)?, parse(local)?, parse(upstream)?);
    let mut fields = Vec::new();
    walk("", Some(&b), Some(&l), Some(&u), &mut fields, 0)?;
    if fields.len() > MAX_FIELDS {
        return None;
    }
    let preview = JsonConfigPreview {
        path: path.into(),
        fields,
    };
    if serde_json::to_vec(&preview).ok()?.len() > MAX_JSON_MERGE_BYTES {
        return None;
    }
    Some(JsonMergeDocument {
        preview,
        upstream: u,
        newline: if local.windows(2).any(|b| b == b"\r\n") {
            "\r\n"
        } else {
            "\n"
        },
        bom: local.starts_with(&[0xef, 0xbb, 0xbf]),
    })
}

/// Caller supplies only displayed pointers and sides, never arbitrary JSON values.
pub fn merge_json_document(
    document: &JsonMergeDocument,
    choices: &[JsonFieldChoice],
) -> Result<Vec<u8>, &'static str> {
    if choices.len() > document.preview.fields.len() {
        return Err("PACK_UPDATE_JSON_SELECTION_INVALID");
    }
    let mut selected = BTreeMap::new();
    for choice in choices {
        if selected
            .insert(choice.pointer.as_str(), choice.side)
            .is_some()
            || !document
                .preview
                .fields
                .iter()
                .any(|f| f.pointer == choice.pointer)
        {
            return Err("PACK_UPDATE_JSON_SELECTION_INVALID");
        }
    }
    let mut result = document.upstream.clone();
    for field in &document.preview.fields {
        let side = selected
            .get(field.pointer.as_str())
            .copied()
            .or_else(|| (!field.conflict).then_some(field.suggested))
            .ok_or("PACK_UPDATE_JSON_CHOICE_REQUIRED")?;
        let value = match side {
            JsonMergeSide::Local => &field.local,
            JsonMergeSide::Upstream => &field.upstream,
        };
        let (parent, key) = field
            .pointer
            .rsplit_once('/')
            .ok_or("PACK_UPDATE_JSON_SELECTION_INVALID")?;
        let key = key.replace("~1", "/").replace("~0", "~");
        let object = result
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .ok_or("PACK_UPDATE_JSON_SELECTION_INVALID")?;
        if value.exists {
            object.insert(key, value.value.clone());
        } else {
            object.remove(&key);
        }
    }
    let mut text =
        serde_json::to_string_pretty(&result).map_err(|_| "PACK_UPDATE_JSON_SELECTION_INVALID")?;
    if document.newline == "\r\n" {
        text = text.replace('\n', "\r\n");
    }
    if document.bom {
        text.insert(0, '\u{feff}');
    }
    text.push_str(document.newline);
    if text.len() > MAX_JSON_MERGE_BYTES {
        return Err("PACK_UPDATE_JSON_SELECTION_INVALID");
    }
    Ok(text.into_bytes())
}

#[cfg(test)]
mod tests;
