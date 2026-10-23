# Text report plugin

This standalone Rust project implements `launcher:resource-report@0.1.0`.
It reads one borrowed input and reports UTF-8 byte, line, and word counts.
It accepts `.txt` files up to 1 MiB. It does not diagnose game failures.

Install the Rust target and pinned component tools:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-tools --version 1.262.0 --locked
```

Use an installed `cobble-plugin-sdk` and matching `cobble-plugin-host`:

```sh
cargo test
cobble-plugin-sdk build . package
cobble-plugin-sdk check package
cobble-plugin-sdk test-report package /path/to/cobble-plugin-host sample.txt --json
```

Create `sample.txt` with your own UTF-8 text. On Windows use the host's `.exe`
path. Set `COBBLE_WASM_TOOLS` if wasm-tools is outside PATH. The first build
creates Cargo.lock; retain it for subsequent locked builds. Choose a new package
directory for each build.

Edit `src/lib.rs` and `plugin.json` for your report. Keep the copied WIT version
compatible with the host. Inputs are read through scoped handles; report output
contains plain text and tables. The host controls file selection, permissions,
redaction, execution limits, and cancellation. No launcher checkout is needed to
compile this project.

The generated manifest contributes one Tools command bound to this report
workflow. It declares no unused settings. Optional `contributions` schema version
1 supports host-rendered commands and setting categories with localized `en` and
`zh-CN` labels. Commands use namespaced IDs and existing `report`/`viewer` workflow
bindings; they cannot name scripts, IPC methods or custom DOM.

The default scaffold retains report API 0.1.0. Reports that consume settings or
locale must explicitly use API 0.2.0 and the `report-plugin-context` world. Copy
`context.wit` and `deps/context/context.wit` from the matching SDK contracts along
with the existing report WIT. The new context import returns one immutable locale
and JSON settings snapshot. Only declared, type-checked settings are supplied;
credentials and filesystem access are not included. Existing 0.1.0 reports keep
their original imports and behavior.

For a context-aware package, test overrides with:

```sh
cobble-plugin-sdk test-report package /path/to/cobble-plugin-host sample.txt --locale zh-CN --setting enabled=false --json
```

Use a setting ID actually declared and consumed by your package. `--setting`
accepts `KEY=JSON` without type coercion. Settings support boolean, enum, string
and bounded number fields, with global/instance scopes and an advanced flag.
The SDK validates declarations and effective values before launching the worker.

## Interface dependencies

The manifest explicitly declares the WIT interface this package implements. Use
`cobble-plugin-sdk interfaces package --registry PATH --json` to inspect a built
package against a local installed-plugin registry. Use `interfaces --installed ID
--registry PATH --json` for an installed package. This reads local metadata only.

Declare required or optional peers in `interfaces.requires` or `interfaces.optional`
with an interface ID and SemVer range. Required peers affect readiness; missing
optional peers permit a fallback. Multiple matches require application selection.
Declarations neither install peers nor grant permissions. Peer use is available only
through host-supported typed workflows; it does not expose arbitrary plugin RPC.

## Permission declarations

The generated manifest explicitly lists its selected-file extensions or HTTPS
origins in `permissions`. Keep these nodes aligned with the workflow declarations;
adding a scope requires a fresh user review. A declaration does not grant access
to arbitrary files, instances or native commands.

Use the host settings form for declared options rather than building a separate
settings UI. Context-aware reports use the versioned report-context world and
`launcher:settings/read` keys for declared scopes; this starter has no settings.
For selected YAML configuration input, reuse a maintained permissive YAML parser
in the guest. Treat YAML as data; host selection and size limits still apply.
