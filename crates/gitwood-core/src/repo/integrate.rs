//! Session-scoped integration transport and cancellation.
use super::{invalid, session::Lease};
use crate::{
    git::{askpass::Askpass, GitError},
    ops::{
        integrate::{self, Integration},
        remote::{RemoteError, RemoteOutcome},
    },
};
use std::sync::Arc;
impl Lease {
    pub async fn integrate(
        &self,
        request: &Integration,
        expected: &str,
        progress: tokio::sync::mpsc::UnboundedSender<String>,
        askpass: Option<Arc<Askpass>>,
    ) -> Result<RemoteOutcome, RemoteError> {
        let mut guard = self
            .writes
            .start(&self.options)
            .map_err(|e| invalid(&e.to_string()))?;
        guard.options.progress = Some(progress);
        guard.options.askpass = askpass.clone();
        let result = integrate::run(&self.runner, &guard.options, request, expected).await;
        result.map_err(|mut error| {
            if askpass.as_ref().is_some_and(|a| a.cancelled()) {
                error.key = "remote.authCancelled".into();
                error.guidance = "remote.checkCredentials";
            }
            error
        })
    }
    pub async fn comparison_open(
        &self,
        left: &str,
        working: bool,
    ) -> Result<super::comparison::Summary, GitError> {
        let comparison =
            super::comparison::open(&self.runner, &self.options, left, working).await?;
        let summary = comparison.summary();
        self.ensure_current()?;
        self.data.lock().await.comparison = Some(Arc::new(comparison));
        Ok(summary)
    }
}
