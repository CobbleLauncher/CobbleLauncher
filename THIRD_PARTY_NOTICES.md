# Third-party notices

Cobble Launcher's own source code is MIT-licensed. Dependencies keep their respective licenses; Cargo.lock and pnpm-lock.yaml pin the exact package versions and archive integrity values. This source repository does not include Minecraft game files, a JDK, mods, account credentials or a signed release package.

The application uses unmodified general-purpose libraries including Vue, Reka UI, Tauri, Tokio, Hyper, native-tls, SQLite/rusqlite, cap-std, zip, flate2, Unicode normalization/case folding and RustCrypto digest crates. No other launcher's implementation is copied into the project. Package sources and license declarations are available through the registries identified in the lockfiles.

## Notices requiring particular attention

- **Reka UI 2.11.0** is MIT-licensed. Source: https://github.com/unovue/reka-ui. It supplies accessible Vue controls.
- The unmodified Rust transitive dependencies **cssparser 0.37.0**, **cssparser-macros 0.7.1**, **dtoa-short 0.3.5**, **option-ext 0.2.0** and **selectors 0.38.0** retain **MPL-2.0**. Original license/notice files are retained in `third-party/licenses/rust/`. Versioned source archives are available at `https://crates.io/api/v1/crates/{name}/{version}/download`; Cargo.lock records their checksums. MPL is not globally permitted by the project's dependency policy.
- **lightningcss 1.33.0** and its platform packages are unmodified build-time tools under **MPL-2.0**, not packages in the pnpm production graph. Source: https://github.com/parcel-bundler/lightningcss. Redistribution of tooling or modifications requires a separate license review.
- **spdx-exceptions 2.5.0** contains SPDX exception-name data by **The Linux Foundation**, with contributions from **Kyle E. Mitchell**, under **CC-BY-3.0**. It is used unchanged by development license checks. Source: https://github.com/kemitchell/spdx-exceptions.json. License: https://creativecommons.org/licenses/by/3.0/.
- Build tooling also includes packages under **BlueOak-1.0.0** and **MIT-0**, alongside MIT/Apache/BSD/ISC packages. The declared policy is in `scripts/license-policy.json`.
- `native-tls` uses the platform TLS implementation (Schannel on Windows, Security Framework on macOS, OpenSSL on applicable Unix platforms). System WebViews and platform libraries retain their own licenses; they are not covered by this project's MIT license.
- Unicode dependency licenses include **Unicode-3.0**. `cap-std` and `cap-fs-ext` offer Apache-2.0 WITH LLVM-exception, Apache-2.0 or MIT alternatives. Dependency licenses must be retained as required when packaging releases.

The Linux GTK3 build graph includes unmaintained **proc-macro-error 1.0.4** (RUSTSEC-2024-0370). `deny.toml` records a maintenance exception, not a vulnerability exemption. It must be reassessed before release.

Original hostile archive samples and the publicly known synthetic TLS test identity are test data created for this project. The TLS identity is trusted only in test builds and is never used for production authentication or signing.

This is a source-repository notice, not a complete license inventory for a future distributed binary release.

## libcurl artifact transport

The unmodified Rust wrappers **curl 0.4.51** and **curl-sys 0.4.91+curl-8.22.0** are MIT-licensed. Source: https://github.com/sagebind/curl-rust. Their packaged VCS revision is `b4f5eeebb6e4746224fb73078aa4af47bc70dd84`; Cargo.lock pins the registry archives and their checksums. This revision was read from registry metadata, not independently fetched.

The native **libcurl** sources bundled by curl-sys retain the **curl license**, which is distinct from the wrappers' MIT license. The unmodified copyright and permission notice is retained in `third-party/licenses/libcurl-8.22.0/COPYING`; source: https://curl.se/download.html. The tested Windows runtime reports `libcurl 8.22.0-DEV / Schannel` from the vendored sources; the version label is not a claim of an upstream stable 8.22 release; other platforms may select a system libcurl, so the vendored version is not a promise about every runtime. TLS certificate checks remain enabled.

The unmodified **libz-sys 1.1.30** wrapper is MIT OR Apache-2.0, and bundled **zlib** retains its own zlib license in `third-party/licenses/zlib/LICENSE`. Source: https://github.com/rust-lang/libz-sys. **openssl-probe 0.1.6** declares MIT/Apache-2.0 and is used by the Rust curl wrapper on applicable Unix platforms; source: https://github.com/alexcrichton/openssl-probe. Platform TLS libraries retain their own licenses.

aria2 was evaluated as a locally installed development tool. Its binary is not bundled with this application or committed to this repository.
