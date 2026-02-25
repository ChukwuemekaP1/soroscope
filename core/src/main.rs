mod auth;
mod benchmarks;
mod errors;
mod jobs;
mod parser;
mod simulation;

use crate::errors::AppError;
use crate::jobs::JobManager;
use crate::simulation::{SimulationCache, SimulationEngine, SimulationResult};
use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, HeaderName, HeaderValue},
    middleware,
    routing::{get, post},
    Extension, Router,
};
use config::{Config, ConfigError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AppConfig {
    server_port: u16,
    rust_log: String,
    soroban_rpc_url: String,
    jwt_secret: String,
    network_passphrase: String,
    /// Redis URL reserved for the distributed cache migration (issue #65).
    /// Unused in the MVP in-memory implementation — present so the config
    /// surface is stable when Redis is wired in.
    redis_url: String,
}

fn load_config() -> Result<AppConfig, ConfigError> {
    dotenvy::dotenv().ok();

    let settings = Config::builder()
        .add_source(config::Environment::default())
        .set_default("server_port", 8080)?
        .set_default("rust_log", "info")?
        .set_default("soroban_rpc_url", "https://soroban-testnet.stellar.org")?
        .set_default("jwt_secret", "dev-secret-change-in-production")?
        .set_default("network_passphrase", "Test SDF Network ; September 2015")?
        .set_default("redis_url", "redis://127.0.0.1:6379")?
        .build()?;

    settings.try_deserialize()
}

/// Shared application state injected into every Axum handler via [`State`].
struct AppState {
    #[allow(dead_code)] // will be used when RPC simulation is wired into analyze handler
    engine: SimulationEngine,
    cache: Arc<SimulationCache>,
    job_manager: Arc<JobManager>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AnalyzeRequest {
    #[schema(example = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC")]
    pub contract_id: String,
    #[schema(example = "hello")]
    pub function_name: String,
    #[schema(example = "[]")]
    pub args: Option<Vec<String>>,
    /// Map of Key-Base64 to Value-Base64 ledger entry overrides
    pub ledger_overrides: Option<HashMap<String, String>>,
}

#[derive(Serialize, ToSchema)]
pub struct ResourceReport {
    /// CPU instructions consumed
    #[schema(example = 1500)]
    pub cpu_instructions: u64,
    /// RAM bytes consumed
    #[schema(example = 3000)]
    pub ram_bytes: u64,
    /// Ledger read bytes
    #[schema(example = 1024)]
    pub ledger_read_bytes: u64,
    /// Ledger write bytes
    #[schema(example = 512)]
    pub ledger_write_bytes: u64,
    /// Transaction size in bytes
    #[schema(example = 450)]
    pub transaction_size_bytes: u64,
    /// Report showing which data was injected vs live
    pub state_dependency: Option<Vec<StateDependencyReport>>,
}

#[derive(Serialize, ToSchema)]
pub struct AnalyzeResponse {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub job_id: String,
}

/// Convert a `SimulationResult` (library type) into the API `ResourceReport`.
fn to_report(result: &SimulationResult) -> ResourceReport {
    ResourceReport {
        cpu_instructions: result.resources.cpu_instructions,
        ram_bytes: result.resources.ram_bytes,
        ledger_read_bytes: result.resources.ledger_read_bytes,
        ledger_write_bytes: result.resources.ledger_write_bytes,
        transaction_size_bytes: result.resources.transaction_size_bytes,
        state_dependency: result.state_dependency.as_ref().map(|deps| {
            deps.iter()
                .map(|d| StateDependencyReport {
                    key: d.key.clone(),
                    source: format!("{:?}", d.source),
                })
                .collect()
        }),
    }
}

#[utoipa::path(
    post,
    path = "/analyze",
    request_body = AnalyzeRequest,
    responses(
        (status = 200, description = "Analysis job submitted", body = AnalyzeResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Job submission failed")
    ),
    security(
        ("jwt" = [])
    ),
    tag = "Analysis"
)]
async fn analyze(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AnalyzeRequest>,
) -> Result<Json<AnalyzeResponse>, AppError> {
    tracing::info!(
        contract_id = %payload.contract_id,
        function_name = %payload.function_name,
        "Received analyze request"
    );

    let args = payload.args.clone().unwrap_or_default();
    let job_id = state.job_manager.submit_job(
        payload.contract_id,
        payload.function_name,
        args,
        payload.ledger_overrides,
    ).await;

    Ok(Json(AnalyzeResponse { job_id }))
}

#[utoipa::path(
    get,
    path = "/job/{job_id}",
    responses(
        (status = 200, description = "Job status retrieved", body = Job),
        (status = 404, description = "Job not found"),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("job_id" = String, Path, description = "Job ID")
    ),
    security(
        ("jwt" = [])
    ),
    tag = "Jobs"
)]
async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> Result<Json<crate::jobs::Job>, AppError> {
    if let Some(job) = state.job_manager.get_job(&job_id) {
        Ok(Json(job))
    } else {
        Err(AppError::NotFound(format!("Job {} not found", job_id)))
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(analyze, auth::challenge_handler, auth::verify_handler),
    components(schemas(
        AnalyzeRequest, ResourceReport,
        auth::ChallengeRequest, auth::ChallengeResponse,
        auth::VerifyRequest, auth::VerifyResponse
    )),
    tags(
        (name = "Analysis", description = "Soroban contract resource analysis endpoints"),
        (name = "Auth", description = "SEP-10 wallet authentication")
    ),
    info(
        title = "SoroScope API",
        version = "0.1.0",
        description = "API for analyzing Soroban smart contract resource consumption"
    )
)]
struct ApiDoc;

async fn health_check() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("SoroScope Starting...");

    let config = load_config().expect("Failed to load configuration");
    tracing::info!("SoroScope initialized with config: {:?}", config);
    tracing::info!(
        redis_url = %config.redis_url,
        "Cache config: using Redis for distributed caching"
    );

    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "benchmark" {
        tracing::info!("Starting SoroScope Benchmark...");

        let possible_paths = vec![
            "target/wasm32-unknown-unknown/release/soroban_token_contract.wasm",
            "../target/wasm32-unknown-unknown/release/soroban_token_contract.wasm",
        ];

        let mut wasm_path = None;
        for p in possible_paths {
            let path = PathBuf::from(p);
            if path.exists() {
                wasm_path = Some(path);
                break;
            }
        }

        if let Some(path) = wasm_path {
            if let Err(e) = benchmarks::run_token_benchmark(path) {
                tracing::error!("Benchmark failed: {}", e);
            }
        } else {
            tracing::error!(
                "Could not find soroban_token_contract.wasm. Build the contract first."
            );
        }

        return;
    }

    tracing::info!("Starting SoroScope API Server...");

    let auth_state = Arc::new(auth::AuthState::new(
        config.jwt_secret.clone(),
        None,
        config.network_passphrase.clone(),
    ));
    tracing::info!(
        "SEP-10 server account: {}",
        auth_state.server_stellar_address()
    );
    let app_state = Arc::new(AppState {
        engine: SimulationEngine::new(config.soroban_rpc_url.clone()),
        cache: SimulationCache::new(&config.redis_url).await.expect("Failed to connect to Redis"),
        job_manager: Arc::new(JobManager::new(SimulationEngine::new(config.soroban_rpc_url.clone()))),
    });

    // Spawn background job cleanup task
    let job_manager_cleanup = app_state.job_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // every 5 minutes
        loop {
            interval.tick().await;
            job_manager_cleanup.cleanup_old_jobs(3600); // cleanup jobs older than 1 hour
        }
    });

    let cors = CorsLayer::new().allow_origin(Any);

    let protected = Router::new()
        .route("/analyze", post(analyze))
        .route("/job/:job_id", get(get_job))
        .route_layer(middleware::from_fn(auth::auth_middleware));

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route(
            "/",
            get(|| async {
                "Hello from SoroScope! Usage: cargo run -p soroscope-core -- benchmark"
            }),
        )
        .route("/health", get(health_check))
        .route("/auth/challenge", post(auth::challenge_handler))
        .route("/auth/verify", post(auth::verify_handler))
        .merge(protected)
        .layer(Extension(auth_state))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state); // ← thread AppState through all handlers

    let bind_addr = format!("0.0.0.0:{}", config.server_port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!(
        "Server listening on http://{}",
        listener.local_addr().unwrap()
    );
    tracing::info!(
        "Swagger UI available at http://{}/swagger-ui",
        listener.local_addr().unwrap()
    );

    axum::serve(listener, app)
        .await
        .expect("Server failed to start");
}
