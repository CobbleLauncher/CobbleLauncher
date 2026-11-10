// SPDX-License-Identifier: MIT
//! Platform routing for trusted launcher adapters; plugin networking stays isolated.
use crate::{NetError, TimeoutPhase};
pub(crate) fn request_error(error: reqwest::Error) -> NetError {
    if error.is_timeout() {
        NetError::Timeout(TimeoutPhase::Total)
    } else if error.is_connect() {
        NetError::Connect
    } else {
        NetError::Protocol
    }
}
