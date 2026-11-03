//! Credential prompts for remote operations, scoped to the window that started them.
use crate::project_windows::WindowSession;
use gitwood_core::git::{askpass::Askpass, PromptKind};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{AppHandle, Emitter};

type Pending = tauri::async_runtime::Sender<Option<String>>;
type Answers = tauri::async_runtime::Receiver<Option<String>>;

/// One prompt: the repository generation, the remote operation within it, and the request
/// ordinal within that operation. Ordinals restart for every operation, so the operation
/// keeps a late answer from one operation away from another's prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
struct Key {
    generation: u64,
    operation: u64,
    request_id: u64,
}

/// Pending prompts of this window. A request exists once: answering it, cancelling the
/// write, or the operation ending removes it, and the window is told to close its dialog.
#[derive(Default)]
pub struct Credentials {
    pending: Mutex<HashMap<Key, Pending>>,
    operations: AtomicU64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    #[serde(flatten)]
    key: Key,
    kind: PromptKind,
    prompt: String,
}

/// Lives as long as the prompt is waiting. Dropping it, whether the prompt was answered or
/// its operation ended first, removes the request and closes the dialog in the window.
struct Waiting {
    credentials: Arc<Credentials>,
    app: AppHandle,
    label: String,
    key: Key,
}

impl Drop for Waiting {
    fn drop(&mut self) {
        self.credentials.close(&self.key);
        if let Err(error) = self
            .app
            .emit_to(self.label.as_str(), "credential_closed", self.key)
        {
            eprintln!("Unable to close the credential prompt: {error}");
        }
    }
}

impl Credentials {
    /// Only remote transports call this; local writes never prompt. Starting an operation
    /// leaves other operations' prompts alone: a write rejected by the serialization lock
    /// must not cancel the one that holds it.
    pub fn start(
        self: &Arc<Self>,
        app: &AppHandle,
        sessions: &WindowSession,
        generation: u64,
    ) -> Option<Arc<Askpass>> {
        let operation = self.operations.fetch_add(1, Ordering::SeqCst) + 1;
        let credentials = Arc::clone(self);
        let app = app.clone();
        let label = sessions.label.clone();
        let handler: gitwood_core::git::askpass::Handler = Arc::new(move |request_id, prompt| {
            let key = Key {
                generation,
                operation,
                request_id,
            };
            let mut answers = credentials.register(key);
            let waiting = Waiting {
                credentials: Arc::clone(&credentials),
                app: app.clone(),
                label: label.clone(),
                key,
            };
            let request = Request {
                key,
                kind: prompt.kind,
                prompt: prompt.text,
            };
            if let Err(error) = app.emit_to(label.as_str(), "credential_request", request) {
                eprintln!("Unable to request credentials: {error}");
                drop(waiting);
                return Box::pin(std::future::ready(None::<String>));
            }
            Box::pin(async move {
                let _waiting = waiting;
                answers.recv().await.flatten()
            })
        });
        Askpass::start(handler)
            .map(Arc::new)
            .map_err(|error| eprintln!("Unable to listen for credential prompts: {error}"))
            .ok()
    }

    fn register(&self, key: Key) -> Answers {
        let (sender, answers) = tauri::async_runtime::channel(1);
        self.pending.lock().unwrap().insert(key, sender);
        answers
    }

    fn close(&self, key: &Key) {
        self.pending.lock().unwrap().remove(key);
    }

    /// Each request is answered at most once; an ended or stale request cannot be answered.
    fn answer(&self, key: Key, answer: Option<String>) -> Result<(), String> {
        let sender = self
            .pending
            .lock()
            .unwrap()
            .remove(&key)
            .ok_or_else(|| "no credential request is waiting for an answer".to_string())?;
        sender
            .try_send(answer)
            .map_err(|_| "the credential request already ended".to_string())
    }

    /// The user cancelled the running write: its waiting prompts end as declined.
    pub fn clear(&self) {
        self.pending.lock().unwrap().clear();
    }
}

#[tauri::command]
pub fn answer_credential(
    sessions: WindowSession,
    generation: u64,
    operation: u64,
    request_id: u64,
    answer: Option<String>,
) -> Result<(), String> {
    if sessions.generation() != generation {
        return Err("the credential request belongs to a closed repository session".into());
    }
    sessions.state.credentials.answer(
        Key {
            generation,
            operation,
            request_id,
        },
        answer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(operation: u64, request_id: u64) -> Key {
        Key {
            generation: 7,
            operation,
            request_id,
        }
    }

    #[test]
    fn each_request_is_answered_once_and_cancelled_when_the_write_is() {
        let credentials = Credentials::default();
        let mut answers = credentials.register(key(1, 1));
        credentials
            .answer(key(1, 1), Some("secret".into()))
            .unwrap();
        assert_eq!(
            tauri::async_runtime::block_on(answers.recv()),
            Some(Some("secret".into()))
        );
        assert!(credentials.answer(key(1, 1), Some("again".into())).is_err());

        let mut answers = credentials.register(key(1, 2));
        credentials.clear();
        assert_eq!(tauri::async_runtime::block_on(answers.recv()), None);
    }

    #[test]
    fn an_ended_operation_cannot_answer_the_next_operations_prompt() {
        let credentials = Credentials::default();
        let _abandoned = credentials.register(key(1, 1));
        credentials.close(&key(1, 1));
        let mut answers = credentials.register(key(2, 1));
        // The stale dialog of operation 1 answers with the same generation and ordinal.
        assert!(credentials.answer(key(1, 1), Some("alice".into())).is_err());
        credentials
            .answer(key(2, 1), Some("secret".into()))
            .unwrap();
        assert_eq!(
            tauri::async_runtime::block_on(answers.recv()),
            Some(Some("secret".into()))
        );
    }
}
