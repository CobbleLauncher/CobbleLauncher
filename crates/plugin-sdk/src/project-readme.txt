# Viewer plugin

The generated parser returns `VIEWER_UNSUPPORTED_FORMAT` until you implement it.
The manifest and copied WIT declare the supported API version.

Install build tools once:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-tools --version 1.262.0 --locked
```

Run the SDK executable from this directory:

```sh
cobble-plugin-sdk build . package
cobble-plugin-sdk check package
cobble-plugin-sdk test package /path/to/cobble-plugin-host input.schem
cobble-plugin-sdk test package /path/to/cobble-plugin-host input.schem --json
```

`build` compiles the Rust library, creates and validates a WebAssembly Component,
and packages it with its SHA-256. Choose a new output directory for each build.
Commit the generated Cargo.lock for repeatable builds. Set `COBBLE_WASM_TOOLS`
to use a wasm-tools executable outside PATH.

`check` verifies package metadata, the component header, and its digest. It does
not validate the Viewer ABI or execute the component. `test` runs the supplied
file in the bounded host, reads every advertised chunk, checks block coordinates
and material totals, and closes the host before reporting success. `--json`
returns a versioned report containing the scene and validated chunk/block counts.
The unchanged scaffold is expected to report unsupported format.

The generated manifest contributes one Tools command bound to the existing Viewer
workflow. Contributions are optional; labels support `en` and `zh-CN`. Command IDs
must begin with the plugin ID and a dot. Menu positions are `tools`, `instance`,
or `blueprint`; conditions are `always`, `instance_selected`, `instance_stopped`,
or `blueprint_selected`. These declarations grant no extra permissions and run no
scripts or custom UI. The host renders the command and opens its existing picker.

Settings can declare categories and typed boolean, enum, string or number fields,
each with a `global` or `instance` scope, a default and optional `advanced` flag.
Enum options use typed scalar JSON values and localized labels. String fields may
declare `max_length` (1–1,024 characters); numeric fields may declare `minimum`
and `maximum` (finite values within ±10¹²). Invalid values are rejected without
coercion. Declare only settings your workflow actually consumes. Provider, auth,
arbitrary IPC and Game Bridge execution are not part of these contributions.

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
