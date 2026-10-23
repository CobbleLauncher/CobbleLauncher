// SPDX-License-Identifier: MIT
//! A workflow-scoped Provider instance. Cancellation belongs to the current RPC.
use super::*;
use crate::worker_session::{WorkerSession, frames};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
pub struct ProviderSession {
    worker: WorkerSession,
    started: Instant,
    last_call: Instant,
    calls: u16,
    planned: Option<(ProviderRequest, Vec<String>, Vec<GetRequest>)>,
}
impl ProviderSession {
    pub fn open(
        host: &Path,
        component: &[u8],
        cancel: Arc<AtomicBool>,
    ) -> Result<Self, PluginError> {
        if component.len() > cobble_plugin_api::MAX_COMPONENT_BYTES
            || !component.starts_with(b"\0asm\x0d\0\x01\0")
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PACKAGE",
                "Expected bounded raw Provider Component",
            ));
        }
        let mut worker = WorkerSession::spawn(
            host,
            "--provider-stdio",
            vec![component.to_vec()],
            cobble_plugin_api::MAX_REPLY_BYTES + 1024,
            &cancel,
        )?;
        let ready: Reply = worker.receive(&cancel)?;
        if !matches!(ready, Reply::Ready) {
            worker.close();
            return Err(invalid("Provider did not initialize"));
        }
        Ok(Self {
            worker,
            started: Instant::now(),
            last_call: Instant::now(),
            calls: 0,
            planned: None,
        })
    }
    fn invoke(
        &mut self,
        job: &Job,
        responses: &[GetResponse],
        cancel: Arc<AtomicBool>,
        validate: impl FnOnce(Reply) -> Result<Reply, PluginError>,
    ) -> Result<Reply, PluginError> {
        if self.started.elapsed() >= Duration::from_secs(300)
            || self.last_call.elapsed() >= Duration::from_secs(60)
            || self.calls >= 128
        {
            self.close();
            return Err(PluginError::new(
                "PLUGIN_CLOSED",
                "Provider workflow expired",
            ));
        }
        if cancel.load(Ordering::Acquire) {
            self.close();
            return Err(PluginError::new(
                "CANCEL_REQUESTED",
                "Provider call cancelled",
            ));
        }
        let request =
            serde_json::to_vec(job).map_err(|_| invalid("Invalid Provider invocation"))?;
        if request.len() > 16 * 1024 {
            return Err(invalid("Provider request exceeds budget"));
        }
        let mut parts = vec![request];
        parts.extend(responses.iter().map(|response| response.body.clone()));
        self.worker.send(frames(&parts))?;
        self.calls += 1;
        let result = self.worker.receive_validated(&cancel, validate);
        self.last_call = Instant::now();
        result
    }
    pub fn plan(
        &mut self,
        request: &ProviderRequest,
        origins: &[String],
        cancel: Arc<AtomicBool>,
    ) -> Result<Vec<GetRequest>, PluginError> {
        request.validate()?;
        self.planned = None;
        let reply = self.invoke(
            &Job::Plan {
                request: request.clone(),
            },
            &[],
            cancel,
            |reply| {
                let Reply::Plan(plan) = &reply else {
                    return Err(invalid("Unexpected Provider reply"));
                };
                validate_plan(plan, origins)?;
                Ok(reply)
            },
        )?;
        if let Reply::Plan(plan) = reply {
            self.planned = Some((request.clone(), origins.to_vec(), plan.clone()));
            Ok(plan)
        } else {
            Err(invalid("Unexpected Provider reply"))
        }
    }
    pub fn normalize(
        &mut self,
        request: &ProviderRequest,
        responses: &[GetResponse],
        origins: &[String],
        cancel: Arc<AtomicBool>,
    ) -> Result<ProviderOutput, PluginError> {
        request.validate()?;
        if !self
            .planned
            .as_ref()
            .is_some_and(|(planned, approved, _)| planned == request && approved == origins)
        {
            self.plan(request, origins, cancel.clone())?;
        }
        let (_, _, plan) = self.planned.take().expect("plan established above");
        if responses.len() != plan.len()
            || responses.iter().zip(&plan).any(|(response, get)| {
                response.body.len() > get.maximum_bytes as usize
                    || !(100..=599).contains(&response.status)
            })
        {
            return Err(invalid("Provider response exceeds its approved plan"));
        }
        let job = Job::Normalize {
            request: request.clone(),
            statuses: responses.iter().map(|r| r.status).collect(),
        };
        let reply = self.invoke(&job, responses, cancel, |reply| {
            let Reply::Normalized(output) = &reply else {
                return Err(invalid("Unexpected Provider reply"));
            };
            validate_output(output, request, origins)?;
            Ok(reply)
        })?;
        if let Reply::Normalized(output) = reply {
            Ok(*output)
        } else {
            Err(invalid("Unexpected Provider reply"))
        }
    }
    pub fn close(&mut self) {
        self.worker.close();
    }
    pub fn process_id(&self) -> u32 {
        self.worker.process_id()
    }
    pub fn has_exited(&mut self) -> std::io::Result<bool> {
        self.worker.has_exited()
    }
}
