use crate::simulation::{SimulationEngine, SimulationResult};
use crate::errors::AppError;
use chrono;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::task;
use tracing::{error, info};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Job {
    pub id: String,
    pub status: JobStatus,
    pub result: Option<SimulationResult>,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

pub struct JobManager {
    jobs: Arc<DashMap<String, Job>>,
    engine: SimulationEngine,
}

impl JobManager {
    pub fn new(engine: SimulationEngine) -> Self {
        Self {
            jobs: Arc::new(DashMap::new()),
            engine,
        }
    }

    pub async fn submit_job(
        &self,
        contract_id: String,
        function_name: String,
        args: Vec<String>,
        ledger_overrides: Option<std::collections::HashMap<String, String>>,
    ) -> String {
        let job_id = Uuid::new_v4().to_string();
        let job = Job {
            id: job_id.clone(),
            status: JobStatus::Pending,
            result: None,
            error: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        self.jobs.insert(job_id.clone(), job);

        let jobs = self.jobs.clone();
        let engine = self.engine.clone();

        // Spawn isolated worker task
        task::spawn(async move {
            let job_id_clone = job_id.clone();
            if let Some(mut job) = jobs.get_mut(&job_id) {
                job.status = JobStatus::Running;
                job.updated_at = chrono::Utc::now();
            }

            match engine.simulate_from_contract_id(&contract_id, &function_name, &args, ledger_overrides).await {
                Ok(result) => {
                    if let Some(mut job) = jobs.get_mut(&job_id) {
                        job.status = JobStatus::Completed;
                        job.result = Some(result);
                        job.updated_at = chrono::Utc::now();
                    }
                    info!(job_id = %job_id_clone, "Job completed successfully");
                }
                Err(e) => {
                    if let Some(mut job) = jobs.get_mut(&job_id) {
                        job.status = JobStatus::Failed;
                        job.error = Some(e.to_string());
                        job.updated_at = chrono::Utc::now();
                    }
                    error!(job_id = %job_id_clone, error = %e, "Job failed");
                }
            }
        });

        job_id
    }

    pub fn get_job(&self, job_id: &str) -> Option<Job> {
        self.jobs.get(job_id).map(|j| j.clone())
    }

    pub fn list_jobs(&self) -> Vec<Job> {
        self.jobs.iter().map(|j| j.clone()).collect()
    }

    // Cleanup old completed/failed jobs to prevent memory leaks
    pub fn cleanup_old_jobs(&self, max_age_seconds: i64) {
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(max_age_seconds);
        self.jobs.retain(|_, job| {
            matches!(job.status, JobStatus::Pending | JobStatus::Running) || job.updated_at > cutoff
        });
    }
}