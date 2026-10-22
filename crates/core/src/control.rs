// SPDX-License-Identifier: MIT
use crate::InstallProgress;
use cobble_net::TransferCancellation;
use std::{fmt, sync::Arc};

#[derive(Debug)]
pub struct OperationCancelled;
impl fmt::Display for OperationCancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("操作已取消")
    }
}
impl std::error::Error for OperationCancelled {}

/// Cooperative control. The owner must await the operation, never abort its task.
#[derive(Clone)]
pub struct OperationControl {
    cancellation: TransferCancellation,
    progress: Arc<dyn Fn(InstallProgress) + Send + Sync>,
    changed: Arc<tokio::sync::Notify>,
}
impl OperationControl {
    pub fn new(progress: Arc<dyn Fn(InstallProgress) + Send + Sync>) -> Self {
        Self {
            cancellation: TransferCancellation::default(),
            progress,
            changed: Arc::new(tokio::sync::Notify::new()),
        }
    }
    pub fn cancel(&self) {
        self.cancellation.cancel();
        self.changed.notify_waiters();
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
    pub fn check_cancelled(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_cancelled() {
            Err(OperationCancelled.into())
        } else {
            Ok(())
        }
    }
    pub fn cancellation(&self) -> TransferCancellation {
        self.cancellation.clone()
    }
    pub(crate) fn progress(&self) -> Arc<dyn Fn(InstallProgress) + Send + Sync> {
        self.progress.clone()
    }
    pub(crate) fn with_progress(
        &self,
        progress: Arc<dyn Fn(InstallProgress) + Send + Sync>,
    ) -> Self {
        Self {
            cancellation: self.cancellation.clone(),
            progress,
            changed: self.changed.clone(),
        }
    }
    pub(crate) async fn cancelled(&self) {
        let changed = self.changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if !self.is_cancelled() {
            changed.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobble_minecraft::DownloadSource;
    use std::path::Path;

    fn cancelled<T>(result: Result<T, Box<dyn std::error::Error + Send + Sync>>) {
        match result {
            Err(error) => assert!(error.is::<OperationCancelled>(), "{error}"),
            Ok(_) => panic!("pre-cancelled operation succeeded"),
        }
    }

    #[test]
    fn precancelled_installers_do_not_resolve_network_metadata_or_create_targets() {
        let control =
            OperationControl::new(Arc::new(|_| panic!("no work before cancellation check")));
        control.cancel();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let absent = std::env::temp_dir().join(format!(
            "cobble-precancelled-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        runtime.block_on(async {
            cancelled(
                crate::install_game_controlled(
                    "invalid",
                    &absent,
                    DownloadSource::Official,
                    control.clone(),
                )
                .await,
            );
            cancelled(
                crate::install_resources_controlled(
                    "invalid",
                    &absent,
                    false,
                    DownloadSource::Official,
                    control.clone(),
                )
                .await,
            );
            cancelled(
                crate::install_fabric_controlled(
                    "invalid",
                    "invalid",
                    &absent,
                    DownloadSource::Official,
                    control.clone(),
                )
                .await,
            );
            cancelled(
                crate::install_forge_controlled(
                    "invalid",
                    "invalid",
                    crate::ForgeKind::Forge,
                    &absent,
                    DownloadSource::Official,
                    control.clone(),
                )
                .await,
            );
            cancelled(
                crate::install_mrpack_controlled(
                    Path::new("missing.mrpack"),
                    &absent,
                    DownloadSource::Official,
                    &[],
                    control.clone(),
                )
                .await,
            );
            cancelled(
                crate::resume_launcher_import_controlled(
                    &absent,
                    DownloadSource::Official,
                    control.clone(),
                )
                .await,
            );
            cancelled(
                crate::install_modrinth_content_pack_controlled(
                    &absent,
                    cobble_content::LocalPackKind::ResourcePack,
                    "invalid",
                    "invalid!",
                    "invalid!",
                    control.clone(),
                )
                .await,
            );
            cancelled(
                crate::download_modrinth_pack_controlled(
                    "invalid!",
                    "invalid!",
                    &absent,
                    control.clone(),
                )
                .await,
            );
            let plan = cobble_content::ModrinthPlan {
                project_id: "Root".into(),
                version_id: "Version".into(),
                mods: Vec::new(),
            };
            cancelled(
                crate::install_modrinth_plan_controlled(&plan, &absent, control.clone()).await,
            );
        });
        assert!(!absent.exists());
    }
}
