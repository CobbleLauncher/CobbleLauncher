// SPDX-License-Identifier: MIT
//! Local, standard TUF trust tooling. No host runtime or implicit trust anchors.
mod keys;
mod package;
mod repository;
pub use keys::{create_key, public_key_id};
pub use package::{BoundFile, PROOF_DIR, copy_package_proof, verify_bound_files};
pub use repository::{
    VerifiedPackage, create_root, sign_package, verify_package, verify_package_blocking,
};
pub type TrustError = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, TrustError>;
fn fail(message: &'static str) -> TrustError {
    message.into()
}
