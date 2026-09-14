// This code is part of Qiskit.
//
// (C) Copyright IBM, Qoro Quantum 2026
//
// This code is licensed under the Apache License, Version 2.0. You may
// obtain a copy of this license in the LICENSE.txt file in the root directory
// of this source tree or at http://www.apache.org/licenses/LICENSE-2.0.
//
// Any modifications or derivative works of this code must retain this
// copyright notice, and modified files need to carry a notice indicating
// that they have been altered from the originals.

use crate::models::{Payload, ResourceType, TaskResult, TaskStatus};
use crate::{QrmiError, QuantumResource, Result};
use anyhow::anyhow;
use async_trait::async_trait;
use maestro_local_api::maestro::session::{Session, Task, TaskType};
use std::collections::HashMap;
use std::env;

/// QRMI implementation for Maestro Local.
///
/// Session configuration is read when the resource is constructed. An acquired
/// session is retained by this instance; exporting its token is only necessary
/// when another process or resource instance needs to use the same session.
pub struct MaestroLocal {
    backend_name: String,
    session_id: Option<u32>,
    // Maestro removes tasks when results are retrieved or cancellation succeeds.
    // Remember terminal states we have actually observed, scoped to the session.
    terminal_tasks: HashMap<(u32, u32), TaskStatus>,
}

impl MaestroLocal {
    /// Constructs a Maestro Local instance.
    ///
    /// Environment variables used:
    /// * `<backend_name>_QRMI_JOB_ACQUISITION_TOKEN`: optional existing session ID.
    /// * `QRMI_MAESTRO_SOCKET`: optional socket path (default `/run/maestro.sock`).
    pub fn new(backend_name: &str) -> Result<Self> {
        let name = format!("{backend_name}_QRMI_JOB_ACQUISITION_TOKEN");
        let session_id = match env::var(&name) {
            Ok(value) => Some(
                value
                    .parse::<u32>()
                    .map_err(|source| QrmiError::ParseError {
                        name,
                        value,
                        source: Box::new(source),
                    })?,
            ),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(QrmiError::InvalidConfig(format!(
                    "{name} is not valid UTF-8"
                )));
            }
        };
        Ok(Self {
            backend_name: backend_name.to_string(),
            session_id,
            terminal_tasks: HashMap::new(),
        })
    }

    fn get_session_id(&self) -> Result<u32> {
        self.session_id.ok_or_else(|| QrmiError::InvalidConfig(format!(
            "No session for {}. Call acquire() or set {}_QRMI_JOB_ACQUISITION_TOKEN before constructing the resource",
            self.backend_name, self.backend_name
        )))
    }

    fn task(&self, task_id: &str) -> Result<Task> {
        let id = task_id
            .parse::<u32>()
            .map_err(|_| QrmiError::InvalidInput(format!("Invalid task ID: {task_id}")))?;
        Ok(Task {
            id,
            session_id: self.get_session_id()?,
        })
    }

    async fn ensure_task_exists(task: &Task) -> Result<()> {
        if task
            .exists()
            .await
            .map_err(|e| anyhow!("Failed to check task {}: {e}", task.id))?
        {
            Ok(())
        } else {
            Err(QrmiError::TaskNotFound(task.id.to_string()))
        }
    }

    /// Preserve the original failure unless the server confirms that the task
    /// disappeared between the existence check and the requested operation.
    async fn task_response<T>(
        task: &Task,
        operation: &str,
        response: std::result::Result<T, String>,
    ) -> Result<T> {
        match response {
            Ok(value) => Ok(value),
            Err(error) => {
                if matches!(task.exists().await, Ok(false)) {
                    return Err(QrmiError::TaskNotFound(task.id.to_string()));
                }
                Err(anyhow!("{operation} for task {}: {error}", task.id).into())
            }
        }
    }

    fn accepted(operation: &str, response: std::result::Result<bool, String>) -> Result<()> {
        match response {
            Ok(true) => Ok(()),
            Ok(false) => Err(QrmiError::InvalidInput(format!(
                "Maestro rejected {operation}"
            ))),
            Err(error) => Err(anyhow!("Failed to {operation}: {error}").into()),
        }
    }
}

#[async_trait]
impl QuantumResource for MaestroLocal {
    async fn resource_id(&mut self) -> Result<String> {
        Ok(self.backend_name.clone())
    }

    async fn resource_type(&mut self) -> Result<ResourceType> {
        Ok(ResourceType::MaestroLocal)
    }

    async fn is_accessible(&mut self) -> Result<bool> {
        use maestro_local_api::maestro_lib::Response;
        match maestro_local_api::maestro_lib::ping().await {
            Response::OK(accessible) => Ok(accessible),
            Response::ERROR(error) => Err(anyhow!("Error pinging Maestro Local: {error}").into()),
            response => Err(anyhow!("Unexpected Maestro Local ping response: {response:?}").into()),
        }
    }

    async fn acquire(&mut self) -> Result<String> {
        if let Some(id) = self.session_id {
            if Session::session_exists(id)
                .await
                .map_err(|e| anyhow!("Failed to check session {id}: {e}"))?
            {
                return Ok(id.to_string());
            }
            self.session_id = None;
            self.terminal_tasks.clear();
        }
        let session =
            Session::new().map_err(|e| anyhow!("Failed to acquire a new session: {e}"))?;
        self.session_id = Some(session.get_id());
        Ok(session.get_id().to_string())
    }

    async fn release(&mut self, id: &str) -> Result<()> {
        // The explicit acquisition token identifies the session being released.
        // An environment variable must never redirect this operation elsewhere.
        let session_id = id
            .parse::<u32>()
            .map_err(|_| QrmiError::InvalidInput(format!("Invalid session ID: {id}")))?;
        let session = Session { id: session_id };
        match session.delete().await {
            Ok(true) => {
                if self.session_id == Some(session_id) {
                    self.session_id = None;
                }
                self.terminal_tasks
                    .retain(|(session, _), _| *session != session_id);
                Ok(())
            }
            Ok(false) => Err(anyhow!("Maestro refused to delete session {id}").into()),
            Err(error) => Err(anyhow!("Error deleting session {id}: {error}").into()),
        }
    }

    async fn task_start(&mut self, payload: Payload) -> Result<String> {
        let Payload::MaestroLocal {
            input,
            job_type,
            qubits,
            simulator_type,
            simulation_method,
            observables,
            config,
        } = payload
        else {
            return Err(QrmiError::UnsupportedPayload(format!("{payload:?}")));
        };

        // Reject malformed input before creating a task on the server.
        let task_type = if job_type.eq_ignore_ascii_case("execute") {
            TaskType::EXECUTE
        } else if job_type.eq_ignore_ascii_case("estimate") {
            if observables.trim().is_empty() {
                return Err(QrmiError::InvalidInput(
                    "Observables must be provided for ESTIMATE job type".into(),
                ));
            }
            TaskType::ESTIMATE
        } else {
            return Err(QrmiError::InvalidInput(format!(
                "Invalid job_type: {job_type}"
            )));
        };
        if input.trim().is_empty() || qubits == 0 {
            return Err(QrmiError::InvalidInput(
                "A nonempty QASM circuit and a positive qubit count are required".into(),
            ));
        }
        let options: serde_json::Value = serde_json::from_str(&config)?;
        if !options.is_object() {
            return Err(QrmiError::InvalidInput(
                "Maestro config must be a JSON object".into(),
            ));
        }

        let session = Session {
            id: self.get_session_id()?,
        };
        let task = session
            .create_task()
            .await
            .map_err(|e| anyhow!("Failed to create task: {e}"))?;
        let estimate = matches!(task_type, TaskType::ESTIMATE);
        let configured = async {
            Self::accepted("set task type", task.set_type(task_type).await)?;
            if estimate {
                Self::accepted(
                    "set observables",
                    task.set_observables_as_string(observables).await,
                )?;
            }
            Self::accepted("set qubits", task.set_qubits(qubits).await)?;
            Self::accepted(
                "set simulator type",
                task.set_simulator_type(simulator_type).await,
            )?;
            Self::accepted(
                "set simulation method",
                task.set_simulation_method(simulation_method).await,
            )?;
            Self::accepted("set QASM", task.set_qasm(input).await)?;
            Self::accepted("set options JSON", task.set_options_json(config).await)?;
            Self::accepted("execute task", task.execute().await)
        }
        .await;
        if let Err(error) = configured {
            // A failed submission otherwise loses the only handle to the task.
            if !matches!(task.cancel().await, Ok(true)) {
                log::warn!(
                    "Failed to clean up Maestro task {} after rejected submission",
                    task.id
                );
            }
            return Err(error);
        }
        self.terminal_tasks.remove(&(task.session_id, task.id));
        Ok(task.id.to_string())
    }

    async fn task_stop(&mut self, task_id: &str) -> Result<()> {
        let task = self.task(task_id)?;
        if matches!(
            self.task_status(task_id).await?,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            return Ok(());
        }
        let response = task.cancel().await;
        if matches!(response, Ok(true)) {
            self.terminal_tasks
                .insert((task.session_id, task.id), TaskStatus::Cancelled);
            return Ok(());
        }
        // A task may finish between the status check and the cancel request.
        // A negative acknowledgement while it remains active is a real failure.
        match self.task_status(task_id).await {
            Ok(TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled) => Ok(()),
            Err(error @ QrmiError::TaskNotFound(_)) => Err(error),
            _ => match response {
                Ok(false) => Err(anyhow!("Maestro refused to cancel active task {task_id}").into()),
                Err(error) => Err(anyhow!("Failed to cancel task {task_id}: {error}").into()),
                Ok(true) => unreachable!(),
            },
        }
    }

    async fn task_status(&mut self, task_id: &str) -> Result<TaskStatus> {
        let task = self.task(task_id)?;
        let key = (task.session_id, task.id);
        if let Some(status) = self.terminal_tasks.get(&key) {
            return Ok(status.clone());
        }
        Self::ensure_task_exists(&task).await?;
        let status =
            if Self::task_response(&task, "Failed to get failure status", task.failed().await)
                .await?
            {
                TaskStatus::Failed
            } else if Self::task_response(
                &task,
                "Failed to get completion status",
                task.finished().await,
            )
            .await?
            {
                TaskStatus::Completed
            } else if Self::task_response(
                &task,
                "Failed to get running status",
                task.running().await,
            )
            .await?
            {
                TaskStatus::Running
            } else {
                TaskStatus::Queued
            };
        if matches!(status, TaskStatus::Completed | TaskStatus::Failed) {
            self.terminal_tasks.insert(key, status.clone());
        }
        Ok(status)
    }

    async fn task_result(&mut self, task_id: &str) -> Result<TaskResult> {
        let task = self.task(task_id)?;
        let status = self.task_status(task_id).await?;
        if status != TaskStatus::Completed {
            return Err(QrmiError::TaskNotReady {
                task_id: task_id.to_string(),
                reason: format!("Task is not completed (current status: {status:?})"),
            });
        }
        // GET_RESULTS consumes the server-side task. Repeated result retrieval
        // reports TaskNotFound, while the known Completed status stays available.
        Self::ensure_task_exists(&task).await?;
        let value = Self::task_response(
            &task,
            "Failed to get results",
            task.get_results_as_string().await,
        )
        .await?;
        Ok(TaskResult { value })
    }

    // task_logs() and target() use the trait's UnsupportedFunction defaults.

    async fn metadata(&mut self) -> HashMap<String, String> {
        HashMap::from([("backend_name".to_string(), self.backend_name.clone())])
    }
}

#[cfg(test)]
#[path = "tests/local.rs"]
mod tests;
