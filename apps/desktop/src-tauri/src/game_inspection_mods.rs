// SPDX-License-Identifier: MIT
//! Fabric/Quilt declarations are descriptive metadata, never compatibility proof.
use super::*;
pub(super) fn declarations(root: &Path, name: &str) -> (Vec<String>, Vec<Value>) {
    let path = root.join("mods").join(name);
    let Ok(file) = fs::File::open(path) else {
        return (vec![], vec![]);
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return (vec![], vec![]);
    };
    if archive.len() > 10_000 {
        return (vec![], vec![]);
    }
    for (file, quilt) in [("fabric.mod.json", false), ("quilt.mod.json", true)] {
        let Ok(mut entry) = archive.by_name(file) else {
            continue;
        };
        if entry.size() > BYTE_LIMIT {
            continue;
        }
        let mut bytes = vec![];
        if entry
            .by_ref()
            .take(BYTE_LIMIT + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() as u64 > BYTE_LIMIT
        {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        return decode(&value, quilt);
    }
    (vec![], vec![])
}
fn decode(value: &Value, quilt: bool) -> (Vec<String>, Vec<Value>) {
    let null = Value::Null;
    let node = if quilt {
        value.get("quilt_loader").unwrap_or(&null)
    } else {
        value
    };
    let mut ids = node
        .get("id")
        .and_then(Value::as_str)
        .map(|id| vec![clean(id)])
        .unwrap_or_default();
    if let Some(provides) = node.get("provides").and_then(Value::as_array) {
        for provided in provides.iter().take(64) {
            let alias = provided
                .as_str()
                .or_else(|| provided.get("id").and_then(Value::as_str));
            if let Some(alias) = alias {
                let alias = clean(alias);
                if !ids.contains(&alias) {
                    ids.push(alias);
                }
            }
        }
    }
    let dependencies = if quilt {
        node.get("depends").and_then(Value::as_array).map(|list|list.iter().take(64).filter_map(|dep|{
            let id=dep.get("id")?.as_str()?;Some(json!({"id":clean(id),"version":dep.get("versions").and_then(Value::as_str).map(clean),"optional":dep.get("optional").and_then(Value::as_bool).unwrap_or(false)}))
        }).collect()).unwrap_or_default()
    } else {
        node.get("depends").and_then(Value::as_object).map(|deps|deps.iter().take(64).map(|(id,version)|json!({"id":clean(id),"version":version.as_str().map(clean),"optional":false})).collect()).unwrap_or_default()
    };
    (ids, dependencies)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fabric_ids_and_declared_dependency_ranges() {
        let (ids, deps) = decode(
            &json!({"id":"demo","depends":{"fabricloader":">=0.16","helper":"*"}}),
            false,
        );
        assert_eq!(ids, vec!["demo"]);
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0]["version"], ">=0.16");
    }
    #[test]
    fn quilt_ids_are_not_guessed_from_filename() {
        let (ids, deps) = decode(
            &json!({"quilt_loader":{"id":"qdemo","depends":[{"id":"helper","versions":"*"}]}}),
            true,
        );
        assert_eq!(ids, vec!["qdemo"]);
        assert_eq!(deps[0]["id"], "helper");
    }
}
