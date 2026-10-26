// SPDX-License-Identifier: MIT
//! Entry component. Map generation is provided by the bounded native Seed Map service.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Guest, Report};
use launcher::resource_report::resources::{Failure, Input};
struct SeedCompass;
impl Guest for SeedCompass {
    fn analyze(_input: &Input) -> Result<Report, Failure> {
        Err(Failure::Unsupported)
    }
}
export!(SeedCompass);
