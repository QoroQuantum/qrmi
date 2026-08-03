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
use std::collections::HashMap;
use std::env;

use async_trait::async_trait;

/// QRMI implementation for Maestro Local
pub struct MaestroLocal {
    pub(crate) backend_name: String,
    pub(crate) session_id: i32,
    pub(crate) job_id: i32,
}

impl MaestroLocal {
    /// Constructs a Maestro Local instance.
    ///
    /// Environment variables used:
    /// * QRMI_JOB_ACQUISITION_TOKEN - (optional) pre‐set session ID
    pub fn new(backend_name : &str) -> Result<Self> {
        let acquisition_token = env::var(format!("{backend_name}_QRMI_JOB_ACQUISITION_TOKEN"))
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap();

        let job_id: i32 = env::var("QRMI_JOB_UID")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap();

        Ok(Self {
            backend_name: backend_name.to_string(),
            session_id: acquisition_token,
            job_id,
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
        Err(anyhow!("is_accessible not available"))
    }

    async fn acquire(&mut self) -> Result<String> {
        Err(anyhow!("acquire not available"))
    }

    async fn release(&mut self, _id: &str) -> Result<()> {
        let token_var = format!("{}_QRMI_JOB_ACQUISITION_TOKEN", self.backend_name);
        let session_id = env::var(&token_var)
            .map_err(|_| anyhow!("{token_var} environment variable is not set"))?;

        Err(anyhow!("release not available"))
    }

    async fn task_start(&mut self, payload: Payload) -> Result<String> {
        let token_var = format!("{}_QRMI_JOB_ACQUISITION_TOKEN", self.backend_name);
        let session_id = env::var(&token_var)
            .map_err(|_| anyhow!("{token_var} environment variable is not set"))?;

        if let Payload::MaestroLocal { input, job_type, config } = payload {
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
        Err(anyhow!("Task logs not available"))
    }

    async fn target(&mut self) -> Result<Target> {
        Err(anyhow!("Target not available"))
    }

    async fn metadata(&mut self) -> HashMap<String, String> {
        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("backend_name".to_string(), self.backend_name.clone());
        metadata
    }
}