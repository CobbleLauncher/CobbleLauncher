// SPDX-License-Identifier: MIT
pub(super) struct Activity {
    operation: &'static str,
    game_id: String,
    config_id: String,
    started: std::time::Instant,
    success: bool,
}
impl Activity {
    pub(super) fn new(operation: &'static str, game_id: &str, config_id: &str) -> Self {
        fn safe(value: &str) -> String {
            if value.len() <= 128
                && !value.is_empty()
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            {
                value.into()
            } else {
                "invalid".into()
            }
        }
        Self {
            operation,
            game_id: safe(game_id),
            config_id: safe(config_id),
            started: std::time::Instant::now(),
            success: false,
        }
    }
    pub(super) fn finish<T>(mut self, result: Result<T, String>) -> Result<T, String> {
        self.success = result.is_ok();
        result
    }
}
impl Drop for Activity {
    fn drop(&mut self) {
        tracing::info!(
            event = "game_config_operation_finished",
            operation = self.operation,
            game_id = self.game_id.as_str(),
            config_id = self.config_id.as_str(),
            outcome = if self.success { "completed" } else { "failed" },
            duration_ms = self.started.elapsed().as_millis() as u64
        );
    }
}
