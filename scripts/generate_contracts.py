"""Generate Rust/TS shape bindings from the project's closed schema vocabulary.

Runtime validation remains the original JSON Schema, including all constraints.
This intentionally fails on unsupported type shapes instead of guessing them.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
NAMES = {"id": "Id", "relativePath": "RelativePath", "sha256": "Sha256", "hashes": "Hashes",
         "source": "Source", "loader": "Loader", "artifact": "Artifact", "instance": "InstanceSpec",
         "lockedFile": "LockedFile", "lock": "InstanceLock", "operation": "Operation", "plan": "InstallPlan",
         "plugin": "PluginManifest", "event": "TaskEvent"}


def pascal(value):
    return "".join(word[:1].upper() + word[1:] for word in re.split(r"[_-]", value))


def generate():
    raw = (ROOT / "contracts/schemas/types.schema.json").read_bytes()
    schema = json.loads(raw)
    digest = hashlib.sha256(raw).hexdigest()
    rust_items = {}

    def rust_type(node, name):
        if "$ref" in node:
            return NAMES[node["$ref"].split("/")[-1]]
        if "anyOf" in node or isinstance(node.get("type"), list):
            choices = node.get("anyOf", [{"type": kind} for kind in node.get("type", [])])
            non_null = [item for item in choices if item.get("type") != "null"]
            if len(non_null) != 1 or len(choices) != 2:
                raise ValueError(f"Unsupported nullable union: {name}")
            return f"Option<{rust_type(non_null[0], name)}>"
        if "enum" in node:
            values = node["enum"]
            if not all(isinstance(value, str) for value in values):
                raise ValueError(f"Unsupported enum: {name}")
            lines = ["#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]", f"pub enum {name} {{"]
            for value in values:
                lines += [f"    #[serde(rename = {json.dumps(value)})]", f"    {pascal(value)},"]
            lines.append("}")
            rust_items[name] = "\n".join(lines)
            return name
        if "oneOf" in node:
            lines = ["#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]", '#[serde(tag = "kind", deny_unknown_fields)]', f"pub enum {name} {{"]
            for variant in node["oneOf"]:
                tag = variant["properties"]["kind"]["const"]
                lines += [f"    #[serde(rename = {json.dumps(tag)})]", f"    {pascal(tag)} {{"]
                for key, field in variant["properties"].items():
                    if key != "kind":
                        lines.append(f"        {key}: {rust_type(field, name + pascal(key))},")
                lines.append("    },")
            lines.append("}")
            rust_items[name] = "\n".join(lines)
            return name
        kind = node.get("type")
        if "const" in node:
            return "String" if isinstance(node["const"], str) else "u64"
        if kind == "object":
            if node.get("additionalProperties") is not False:
                raise ValueError(f"Open object is not supported: {name}")
            lines = ["#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]", "#[serde(deny_unknown_fields)]", f"pub struct {name} {{"]
            for key, field in node["properties"].items():
                value_type = rust_type(field, name + pascal(key))
                if key not in node.get("required", []):
                    lines.append('    #[serde(default, skip_serializing_if = "Option::is_none")]')
                    value_type = f"Option<{value_type}>"
                lines.append(f"    pub {key}: {value_type},")
            lines.append("}")
            rust_items[name] = "\n".join(lines)
            return name
        if kind == "array":
            return f"Vec<{rust_type(node['items'], name + 'Item')}>"
        if kind == "integer":
            return "u64" if node.get("minimum", 0) >= 0 else "i64"
        if kind == "string":
            return "String"
        if kind == "boolean":
            return "bool"
        raise ValueError(f"Unsupported Rust shape: {name}: {node}")

    def ts_type(node):
        if "$ref" in node:
            return NAMES[node["$ref"].split("/")[-1]]
        if "const" in node:
            return json.dumps(node["const"])
        if "enum" in node:
            return " | ".join(json.dumps(value) for value in node["enum"])
        for union in ("anyOf", "oneOf"):
            if union in node:
                return " | ".join(ts_type(item) for item in node[union])
        kind = node.get("type")
        if isinstance(kind, list):
            return " | ".join(ts_type({"type": value}) for value in kind)
        if kind == "object":
            return "{\n" + "\n".join(f"  {key}{'' if key in node.get('required', []) else '?'}: {ts_type(value)};"
                                         for key, value in node["properties"].items()) + "\n}"
        if kind == "array":
            return f"Array<{ts_type(node['items'])}>"
        return {"string": "string", "integer": "number", "boolean": "boolean", "null": "null"}[kind]

    ts_items = []
    for key, node in schema["$defs"].items():
        name = NAMES[key]
        value_type = rust_type(node, name)
        if value_type != name:
            rust_items[name] = f"pub type {name} = {value_type};"
        ts_items.append(f"export type {name} = {ts_type(node)};")
    rust = "// SPDX-License-Identifier: MIT\n// Generated by scripts/generate_contracts.py. Do not edit.\n"
    rust += f"// Source SHA-256: {digest}\nuse serde::{{Deserialize, Serialize}};\n\n"
    rust += "\n\n".join(rust_items.values()) + "\n"
    rust = subprocess.check_output(["rustfmt", "--edition", "2024", "--emit", "stdout"], input=rust, text=True, encoding="utf-8")
    ts = "// SPDX-License-Identifier: MIT\n// Generated by scripts/generate_contracts.py. Do not edit.\n"
    ts += f"// Source SHA-256: {digest}\n\n" + "\n\n".join(ts_items) + "\n"
    return {"crates/domain/src/generated.rs": rust, "contracts/generated/models.ts": ts}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    for relative, content in generate().items():
        path = ROOT / relative
        if args.check:
            if not path.exists() or path.read_text(encoding="utf-8") != content:
                raise SystemExit(f"Generated contract drift: {relative}")
        else:
            path.parent.mkdir(exist_ok=True, parents=True)
            path.write_text(content, encoding="utf-8", newline="\n")
    print("Contract bindings: " + ("no drift" if args.check else "generated"))


if __name__ == "__main__":
    main()
