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
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;

use maestro_local_api::maestro::session::{Session, Task, TaskConfig, TaskType};

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
            if response.is_ok() && response.unwrap() == true {
                return Ok(old_session_id.to_string());
            }
        }

        let session = Session::new();
        if session.is_ok() {
            let session_id = session.unwrap().get_id();
            self.session_id = Some(session_id);
            Ok(session_id.to_string())
        } else {
            Err(anyhow!("Failed to acquire a new session"))
        }
    }

    async fn release(&mut self, id: &str) -> Result<()> {
        let token_var = format!("{}_QRMI_JOB_ACQUISITION_TOKEN", self.backend_name);
        let env_session_id = env::var(&token_var);

        let session_id;
        if env_session_id.is_err() {
            session_id = id
                .parse::<u32>()
                .map_err(|_| anyhow!("Invalid session ID: {}", id))?;
        } else {
            let si = env_session_id.unwrap();
            session_id = si
                .parse::<u32>()
                .map_err(|_| anyhow!("Invalid session ID: {}", si))?;
        }

        let session = Session { id: session_id };

        Err(anyhow!("release not available"))
    }

    async fn task_start(&mut self, payload: Payload) -> Result<String> {
        let token_var = format!("{}_QRMI_JOB_ACQUISITION_TOKEN", self.backend_name);
        let session_id = env::var(&token_var)
            .map_err(|_| anyhow!("{token_var} environment variable is not set"))?;

        if let Payload::MaestroLocal {
            input,
            job_type,
            config,
        } = payload
        {
            Err(anyhow!("task_start not available for MaestroLocal payload"))
        } else {
            bail!(format!("Payload type is not supported. {:?}", payload))
        }
    }

    async fn task_stop(&mut self, task_id: &str) -> Result<()> {
        Err(anyhow!("task_stop not available"))
    }

    async fn task_status(&mut self, task_id: &str) -> Result<TaskStatus> {
        Err(anyhow!("Task status not available"))
    }

    async fn task_result(&mut self, task_id: &str) -> Result<TaskResult> {
        Err(anyhow!("Task result not available"))
    }

    async fn task_logs(&mut self, task_id: &str) -> Result<String> {
        Ok("Logging not implemented for this QuantumResource".to_string())
    }

    async fn target(&mut self) -> Result<Target> {
        let mut resp = json!({});

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
