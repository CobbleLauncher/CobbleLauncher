// SPDX-License-Identifier: MIT
//! Validate untrusted JSON using the canonical schemas before constructing models.
//! Structural validity is not filesystem authorization or an install approval.

use std::sync::LazyLock;

use jsonschema::{Draft, Validator};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::generated::{InstallPlan, InstanceLock, InstanceSpec, PluginManifest, TaskEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractError {
    InputTooLarge,
    InvalidJson,
    InvalidSchema,
    ModelMismatch,
}

mod sealed {
    pub trait Sealed {}
}

/// Only the five canonical contract roots can select a schema validator.
pub trait ContractModel: DeserializeOwned + sealed::Sealed {
    const INDEX: usize;
}

macro_rules! model {
    ($name:ty, $index:expr) => {
        impl sealed::Sealed for $name {}
        impl ContractModel for $name {
            const INDEX: usize = $index;
        }
    };
}
model!(InstanceSpec, 0);
model!(InstanceLock, 1);
model!(InstallPlan, 2);
model!(PluginManifest, 3);
model!(TaskEvent, 4);

static VALIDATORS: LazyLock<Vec<Validator>> = LazyLock::new(|| {
    let template: Value =
        serde_json::from_str(include_str!("../../../contracts/schemas/types.schema.json"))
            .expect("embedded contract schema must be valid JSON");
    ["instance", "lock", "plan", "plugin", "event"]
        .into_iter()
        .map(|definition| {
            let mut schema = template.clone();
            schema["$ref"] = Value::String(format!("#/$defs/{definition}"));
            jsonschema::options()
                .with_draft(Draft::Draft202012)
                .should_validate_formats(true)
                .build(&schema)
                .expect("embedded contract schema must compile")
        })
        .collect()
});

/// Enforces a caller-selected byte budget and every structural schema constraint.
/// Errors never contain input values (which could contain credentials).
pub fn parse_contract<T: ContractModel>(input: &str, max_bytes: usize) -> Result<T, ContractError> {
    if input.len() > max_bytes {
        return Err(ContractError::InputTooLarge);
    }
    let mut value: Value = serde_json::from_str(input).map_err(|_| ContractError::InvalidJson)?;
    if !VALIDATORS[T::INDEX].is_valid(&value) {
        return Err(ContractError::InvalidSchema);
    }
    normalize_integers(&mut value);
    serde_json::from_value(value).map_err(|_| ContractError::ModelMismatch)
}

// JSON Schema's integer type includes e.g. 512.0; normalize validated bounded
// integral JSON numbers so serde's integer fields preserve that same semantics.
fn normalize_integers(value: &mut Value) {
    match value {
        Value::Number(number) if number.is_f64() => {
            if let Some(number) = number.as_f64()
                && number.fract() == 0.0
                && (0.0..=9_007_199_254_740_991.0).contains(&number)
            {
                *value = Value::from(number as u64);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(normalize_integers),
        Value::Object(fields) => fields.values_mut().for_each(normalize_integers),
        _ => {}
    }
}
