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

use crate::models::{Payload, ResourceType, Target, TaskResult, TaskStatus};
use crate::QuantumResource;
use anyhow::anyhow;
use anyhow::{bail, Result};
//use log::warn;
use serde_json::json;
use std::collections::HashMap;
use std::env;

use maestro_local_api::maestro::session::{Session, Task, TaskType};

use async_trait::async_trait;

/// QRMI implementation for Maestro Local
pub struct MaestroLocal {
    pub(crate) backend_name: String,
    pub(crate) session_id: Option<u32>,
}

impl MaestroLocal {
    /// Constructs a Maestro Local instance.
    ///
    /// Environment variables used:
    /// * QRMI_JOB_ACQUISITION_TOKEN - (optional) pre‐set session ID
    pub fn new(backend_name: &str) -> Result<Self> {
        let acquisition_token: Option<u32> =
            env::var(format!("{backend_name}_QRMI_JOB_ACQUISITION_TOKEN"))
                .ok()
                .and_then(|s| s.parse::<u32>().ok());

        Ok(Self {
            backend_name: backend_name.to_string(),
            session_id: acquisition_token,
        })
    }

    fn get_session_id(&self) -> Result<u32> {
        let token_var = format!("{}_QRMI_JOB_ACQUISITION_TOKEN", self.backend_name);
        let env_session_id = env::var(&token_var);

        if let Ok(si) = env_session_id {
            let session_id = si
                .parse::<u32>()
                .map_err(|_| anyhow!("Invalid session ID: {}", si))?;
            Ok(session_id)
        } else if self.session_id.is_none() {
            Err(anyhow!(
                "Session ID not set. Please acquire a session first."
            ))
        } else {
            Ok(self.session_id.unwrap())
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
        let response = maestro_local_api::maestro_lib::ping().await;

        match response {
            maestro_local_api::maestro_lib::Response::OK(true) => Ok(true),
            maestro_local_api::maestro_lib::Response::OK(false) => Ok(false),
            maestro_local_api::maestro_lib::Response::ERROR(e) => {
                Err(anyhow!("Error pinging Maestro Local: {}", e))
            }
            _ => bail!("Unexpected response from Maestro Local ping"),
        }
    }

    async fn acquire(&mut self) -> Result<String> {
        // check if the session id is valid, otherwise create a new one
        if let Some(old_session_id) = self.session_id {
            // check to see if the server accepts it
            let response = Session::session_exists(old_session_id).await;
            if response.is_ok() && response.unwrap() {
                return Ok(old_session_id.to_string());
            }
        }

        let session = Session::new();
        if let Ok(session) = session {
            let session_id = session.get_id();
            self.session_id = Some(session_id);
            Ok(session_id.to_string())
        } else {
            Err(anyhow!("Failed to acquire a new session"))
        }
    }

    async fn release(&mut self, id: &str) -> Result<()> {
        let env_session_id = self.get_session_id();

        let session_id = if let Ok(env_session_id) = env_session_id {
            env_session_id
        } else {
            id.parse::<u32>()
                .map_err(|_| anyhow!("Invalid session ID: {}", id))?
        };

        let session = Session { id: session_id };
        let response = session.delete().await;

        match response {
            Ok(true) => Ok(()),
            Ok(false) => Err(anyhow!("Failed to delete session")),
            Err(e) => Err(anyhow!("Error deleting session: {}", e)),
        }
    }

    async fn task_start(&mut self, payload: Payload) -> Result<String> {
        let session_id = self.get_session_id()?;

        if let Payload::MaestroLocal {
            input,
            job_type,
            qubits,
            simulator_type,
            simulation_method,
            observables,
            config,
        } = payload
        {
            let session = Session { id: session_id };
            let response = session.create_task().await;

            if let Ok(task) = response {
                let task_id = task.get_id();
                // now set all the stuff and send it to be executed
                if job_type == "EXECUTE" || job_type == "execute" || job_type == "Execute" {
                    let result = task.set_type(TaskType::EXECUTE).await;
                    match result {
                        Ok(true) => {}
                        Ok(false) => {
                            bail!("Failed to set task type");
                        }
                        Err(e) => {
                            bail!("Error setting task type: {}", e);
                        }
                    };
                } else if job_type == "ESTIMATE" || job_type == "estimate" || job_type == "Estimate"
                {
                    let result = task.set_type(TaskType::ESTIMATE).await;
                    match result {
                        Ok(true) => {}
                        Ok(false) => {
                            bail!("Failed to set task type");
                        }
                        Err(e) => {
                            bail!("Error setting task type: {}", e);
                        }
                    };

                    if observables.is_empty() {
                        bail!("Observables must be provided for ESTIMATE job type");
                    } else {
                        let result = task.set_observables_as_string(observables).await;
                        match result {
                            Ok(true) => {}
                            Ok(false) => {
                                bail!("Failed to set observables");
                            }
                            Err(e) => {
                                bail!("Error setting observables: {}", e);
                            }
                        };
                    }
                } else {
                    bail!("Invalid job_type: {}", job_type);
                }

                let result = task.set_qubits(qubits).await;
                match result {
                    Ok(true) => {}
                    Ok(false) => {
                        bail!("Failed to set qubits");
                    }
                    Err(e) => {
                        bail!("Error setting qubits: {}", e);
                    }
                };
                let result = task.set_simulator_type(simulator_type).await;
                match result {
                    Ok(true) => {}
                    Ok(false) => {
                        bail!("Failed to set simulator type");
                    }
                    Err(e) => {
                        bail!("Error setting simulator type: {}", e);
                    }
                };
                let result = task.set_simulation_method(simulation_method).await;
                match result {
                    Ok(true) => {}
                    Ok(false) => {
                        bail!("Failed to set simulation method");
                    }
                    Err(e) => {
                        bail!("Error setting simulation method: {}", e);
                    }
                };

                let result = task.set_qasm(input).await;
                match result {
                    Ok(true) => {}
                    Ok(false) => {
                        bail!("Failed to set QASM");
                    }
                    Err(e) => {
                        bail!("Error setting QASM: {}", e);
                    }
                };
                let result = task.set_options_json(config).await;
                match result {
                    Ok(true) => {}
                    Ok(false) => {
                        bail!("Failed to set options JSON");
                    }
                    Err(e) => {
                        bail!("Error setting options JSON: {}", e);
                    }
                };

                let result = task.execute().await;
                match result {
                    Ok(true) => {}
                    Ok(false) => {
                        bail!("Failed to execute task");
                    }
                    Err(e) => {
                        bail!("Error executing task: {}", e);
                    }
                };

                Ok(task_id.to_string())
            } else {
                Err(anyhow!(
                    "Failed to start task, reason: {}",
                    response.unwrap_err()
                ))
            }
        } else {
            bail!(format!("Payload type is not supported. {:?}", payload))
        }
    }

    async fn task_stop(&mut self, task_id: &str) -> Result<()> {
        let session_id = self.get_session_id()?;
        let id = task_id
            .parse::<u32>()
            .map_err(|_| anyhow!("Invalid task ID: {}", task_id))?;

        let task = Task { id, session_id };
        task.cancel()
            .await
            .map_err(|e| anyhow!("Failed to cancel task: {}", e))?;

        Ok(())
    }

    async fn task_status(&mut self, task_id: &str) -> Result<TaskStatus> {
        let session_id = self.get_session_id()?;
        let id = task_id
            .parse::<u32>()
            .map_err(|_| anyhow!("Invalid task ID: {}", task_id))?;

        let task = Task { id, session_id };

        let response = task.finished().await;
        if let Ok(finished) = response {
            if finished {
                Ok(TaskStatus::Completed)
            } else {
                let response = task.running().await;
                if let Ok(running) = response {
                    if running {
                        Ok(TaskStatus::Running)
                    } else {
                        let response = task.exists().await;
                        if let Ok(queued) = response {
                            if queued {
                                Ok(TaskStatus::Queued)
                            } else {
                                // it might not exist anymore, it doesn't mean it's Cancelled
                                // might be finished but the results were retrieved so it doesn't exist anymore on the server
                                // we need to store cancelled tasks here for knowing
                                Ok(TaskStatus::Cancelled)
                            }
                        } else {
                            Err(anyhow!(
                                "Failed to get task queued status, reason: {}",
                                response.unwrap_err()
                            ))
                        }
                    }
                } else {
                    Err(anyhow!(
                        "Failed to get task running status, reason: {}",
                        response.unwrap_err()
                    ))
                }
            }
        } else {
            Err(anyhow!(
                "Failed to get task finished status, reason: {}",
                response.unwrap_err()
            ))
        }
    }

    async fn task_result(&mut self, task_id: &str) -> Result<TaskResult> {
        let session_id = self.get_session_id()?;
        let id = task_id
            .parse::<u32>()
            .map_err(|_| anyhow!("Invalid task ID: {}", task_id))?;

        let status = self.task_status(task_id).await?;
        if status == TaskStatus::Completed {
            let task = Task { id, session_id };
            let response = task.get_results_as_string().await;
            if let Ok(result) = response {
                Ok(TaskResult { value: result })
            } else {
                Err(anyhow!(
                    "Failed to get task result, reason: {}",
                    response.unwrap_err()
                ))
            }
        } else {
            Err(anyhow!("Task result not available, task is not completed"))
        }
    }

    async fn task_logs(&mut self, _task_id: &str) -> Result<String> {
        Ok("Logging not implemented for this QuantumResource".to_string())
    }

    async fn target(&mut self) -> Result<Target> {
        let resp = json!({});

        Ok(Target {
            value: resp.to_string(),
        })
    }

    async fn metadata(&mut self) -> HashMap<String, String> {
        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("backend_name".to_string(), self.backend_name.clone());
        metadata
    }
}

#[cfg(test)]
#[path = "tests/local.rs"]
mod tests;
