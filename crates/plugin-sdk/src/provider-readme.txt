# Blueprint Provider

This standalone example uses `launcher:blueprint-provider@0.2.0` and a public JSON catalog with at most 50 original `.schem` resources. Replace the example catalog mapping with your service's API. Optional catalog `license` values are preserved; absent licenses remain unknown. No network or filesystem imports are available to the guest.

The approved HTTPS origin is copied into `plugin.json` and `src/lib.rs`; update both together. Only serve files you have permission to distribute. The example does not claim Java-version compatibility, support pagination, or authenticate users.

The project pins Rust 1.99.0 and the Wasm target. Install `wasm-tools 1.262.0`, then use the matching SDK and host:

```sh
cobble-plugin-sdk build . package
cobble-plugin-sdk check package
cobble-plugin-sdk test-provider package /path/to/cobble-plugin-host fixtures/search.json fixtures/catalog.json --json
cobble-plugin-sdk test-provider package /path/to/cobble-plugin-host fixtures/detail.json fixtures/catalog.json --json
cobble-plugin-sdk test-provider package /path/to/cobble-plugin-host fixtures/download.json fixtures/catalog.json --json
```

On Windows use the host's `.exe` path. These fixtures execute the actual Component without network requests; the sample byte count is synthetic and no blueprint file is included. The native launcher fetches proposed original files only after checking origin permission, size and identity. Retain the generated Cargo.lock for subsequent locked builds. Build into a new package directory each time.

The manifest explicitly provides `launcher:blueprint-provider` version `0.2.0`. Optional peers can be declared under `interfaces.optional` with `{ "id": "launcher:blueprint-provider", "version": "^0.2.0" }`; required dependencies use `interfaces.requires`. These are discovery declarations, not permissions. Missing optional peers do not block local features. SemVer ranges do not authorize an unsupported WIT ABI or install any package.

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

The generated Provider declares `launcher:viewer ^1.0.0` as optional: catalog search works without a Viewer, and the application can recommend a compatible one for opening results.

## Permission declarations

The generated manifest explicitly lists its selected-file extensions or HTTPS
origins in `permissions`. Keep these nodes aligned with the workflow declarations;
adding a scope requires a fresh user review. A declaration does not grant access
to arbitrary files, instances or native commands.
