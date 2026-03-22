use gateway::*;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "gateway", version, about = "Cloud Model Access Gateway")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the gateway server
    Serve {
        /// Path to gateway.toml config file
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },

    /// Manage model routes
    Route {
        #[command(subcommand)]
        action: RouteAction,
    },

    /// Query cost records
    Cost {
        #[command(subcommand)]
        action: CostAction,
    },

    /// Manage budget ceilings
    Budget {
        #[command(subcommand)]
        action: BudgetAction,
    },

    /// Search operational logs
    Log {
        #[command(subcommand)]
        action: LogAction,
    },

    /// Search audit logs
    Audit {
        #[command(subcommand)]
        action: AuditAction,
    },

    /// Trigger kill switch
    Kill {
        /// Kill mode: drain or halt
        #[arg(long)]
        mode: String,
        /// Admin API base URL
        #[arg(long, default_value = "http://127.0.0.1:8801")]
        admin_url: String,
    },

    /// Resume from drain/halt
    Resume {
        /// Admin API base URL
        #[arg(long, default_value = "http://127.0.0.1:8801")]
        admin_url: String,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum RouteAction {
    /// List all routes
    List {
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
    /// Add a route from a JSON file
    Add {
        #[arg(long)]
        file: PathBuf,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
    /// Update a route field
    Update {
        route_key: String,
        #[arg(long)]
        field: String,
        #[arg(long)]
        value: String,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
    /// Disable a route
    Disable {
        route_key: String,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
    /// Enable a route
    Enable {
        route_key: String,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
    /// Show route version history
    History {
        route_key: String,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
    /// Rollback to a previous version
    Rollback {
        route_key: String,
        #[arg(long)]
        version: u32,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
    /// Fire a manual health probe
    Probe {
        route_key: String,
        #[arg(long, default_value = "http://127.0.0.1:8801")]
        admin_url: String,
    },
}

#[derive(Subcommand)]
enum CostAction {
    /// Spending summary
    Summary {
        #[arg(long)]
        caller: Option<String>,
        #[arg(long)]
        route: Option<String>,
        #[arg(long, default_value = "24h")]
        window: String,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
    /// Top routes by spend
    TopRoutes {
        #[arg(long, default_value = "30d")]
        window: String,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
    /// Top callers by spend
    TopCallers {
        #[arg(long, default_value = "30d")]
        window: String,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
}

#[derive(Subcommand)]
enum BudgetAction {
    /// Set a budget ceiling
    Set {
        #[arg(long)]
        caller: Option<String>,
        #[arg(long)]
        route: Option<String>,
        #[arg(long)]
        global: bool,
        #[arg(long)]
        daily: f64,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
    /// Show current budget state
    Show {
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
}

#[derive(Subcommand)]
enum LogAction {
    /// Search operational logs
    Search {
        #[arg(long)]
        caller: Option<String>,
        #[arg(long)]
        route: Option<String>,
        #[arg(long)]
        outcome: Option<String>,
        #[arg(long)]
        request_id: Option<String>,
        #[arg(long)]
        error_code: Option<String>,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        window: Option<String>,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
    /// Live tail of logs
    Tail {
        #[arg(long)]
        level: Option<String>,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
}

#[derive(Subcommand)]
enum AuditAction {
    /// Search audit logs
    Search {
        #[arg(long)]
        event: Option<String>,
        #[arg(long)]
        severity: Option<String>,
        #[arg(long)]
        window: Option<String>,
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Reload configuration (sends to running server)
    Reload {
        #[arg(long, default_value = "http://127.0.0.1:8801")]
        admin_url: String,
    },
    /// Validate a config file offline
    Validate {
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    panorama_logging::init("gateway", Some("data/panorama_logs.db"));

    match cli.command {
        Commands::Serve {
            config: config_path,
        } => {
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(run_server(config_path));
        }
        Commands::Config {
            action: ConfigAction::Validate { config: config_path },
        } => match config::GatewayConfig::load(&config_path) {
            Ok(_) => {
                println!("config at {} is valid", config_path.display());
            }
            Err(e) => {
                eprintln!("config validation failed: {e}");
                std::process::exit(1);
            }
        },
        Commands::Route { action } => {
            handle_route_command(action);
        }
        _ => {
            tracing::warn!("command not yet implemented");
        }
    }
}

async fn run_server(config_path: PathBuf) {
    tracing::info!("loading config from {}", config_path.display());
    let cfg = match config::GatewayConfig::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("failed to load config: {e}");
            std::process::exit(1);
        }
    };

    // Initialize databases
    let route_store_conn = db::init_route_store_db(std::path::Path::new(&cfg.database.route_store_path))
        .expect("failed to init route store DB");
    let operational_conn = db::init_operational_db(std::path::Path::new(&cfg.database.operational_db_path))
        .expect("failed to init operational DB");
    let audit_conn = db::init_audit_db(std::path::Path::new(&cfg.database.audit_db_path))
        .expect("failed to init audit DB");
    let cost_conn = db::init_route_store_db(std::path::Path::new(&cfg.database.route_store_path))
        .expect("failed to init cost accountant connection");

    let route_store = routes::store::RouteStore::new(route_store_conn);
    let operational_logger = logging::operational::OperationalLogger::new(operational_conn);
    let audit_logger = logging::audit::AuditLogger::new(audit_conn);
    let cost_accountant = accounting::cost::CostAccountant::new(cost_conn);
    let budget_enforcer = accounting::budget::BudgetEnforcer::new(cfg.budgets.clone());
    let rate_limiter = rate_limit::limiter::RateLimiter::new(cfg.rate_limits.clone());

    let deduplicator = if cfg.deduplication.enabled {
        Some(dedup::fingerprint::Deduplicator::new(cfg.deduplication.window_secs))
    } else {
        None
    };

    let kill_switch = kill_switch::controller::KillSwitchController::new(
        cfg.kill_switch.auto_drain_on_consecutive_criticals,
        cfg.kill_switch.auto_halt_on_credential_scrub,
    );

    let health_map = routes::health::new_health_map();

    // Compile sanitization rules
    let inbound_rules = sanitizer::rules::SanitizationRuleSet::compile(&cfg.sanitizer)
        .expect("failed to compile inbound sanitization rules");
    let outbound_rules = sanitizer::rules::SanitizationRuleSet::compile(&cfg.sanitizer)
        .expect("failed to compile outbound sanitization rules");

    let inbound_sanitizer = sanitizer::inbound::InboundSanitizer::new(inbound_rules, 1_000_000);
    let outbound_sanitizer = sanitizer::outbound::OutboundSanitizer::new(outbound_rules);

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(cfg.server.request_timeout_secs))
        .build()
        .expect("failed to build HTTP client");

    // Register with Cloak
    let cloak_url = std::env::var("CLOAK_URL").unwrap_or_else(|_| "http://localhost:8300".into());
    let cloak_token = std::env::var("CLOAK_MANIFEST_TOKEN").unwrap_or_default();
    let cloak_config = cloak_sdk::CloakConfig::new(
        &cloak_url,
        &cloak_token,
        "gateway",
        "gateway",
        env!("CARGO_PKG_VERSION"),
    )
    .with_capabilities(vec!["llm_dispatch".into(), "route_management".into()]);

    let cloak_state = cloak_sdk::CloakState::new();
    let cloak_client = cloak_sdk::CloakClient::new(cloak_config);

    match cloak_client.register(&cloak_state).await {
        Ok(halt_url) => {
            tracing::info!("registered with Cloak");
            cloak_client.spawn_halt_listener(cloak_state.clone(), halt_url);
        }
        Err(e) => {
            tracing::warn!("Cloak registration failed (continuing without): {e}");
        }
    }

    let host = cfg.server.host.clone();
    let port = cfg.server.port;
    let admin_port = cfg.server.admin_port;

    let state = std::sync::Arc::new(server::AppState {
        config: cfg,
        cloak: cloak_state,
        route_store,
        http_client,
        operational_logger,
        audit_logger,
        cost_accountant,
        budget_enforcer,
        rate_limiter,
        deduplicator,
        kill_switch,
        health_map,
        inbound_sanitizer,
        outbound_sanitizer,
    });

    let request_router = server::build_request_router(state.clone());
    let admin_router = server::build_admin_router(state.clone());

    let request_addr = format!("{host}:{port}");
    let admin_addr = format!("{host}:{admin_port}");

    tracing::info!("starting request server on {request_addr}");
    tracing::info!("starting admin server on {admin_addr}");

    let request_listener = tokio::net::TcpListener::bind(&request_addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("failed to bind request port {request_addr}: {e}");
            std::process::exit(1);
        });

    let admin_listener = tokio::net::TcpListener::bind(&admin_addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("failed to bind admin port {admin_addr}: {e}");
            std::process::exit(1);
        });

    // Start health probes if enabled
    if state.config.health_probing.enabled {
        let probe_routes = state.route_store.list_routes().unwrap_or_default();
        let _probe_handles = routes::health::start_health_probes(
            state.health_map.clone(),
            probe_routes,
            state.http_client.clone(),
        );
    }

    let request_server = axum::serve(request_listener, request_router);
    let admin_server = axum::serve(admin_listener, admin_router);

    tracing::info!("gateway is operational");

    // Run both servers, shutdown on SIGTERM/SIGINT
    tokio::select! {
        result = request_server => {
            if let Err(e) = result {
                tracing::error!("request server error: {e}");
            }
        }
        result = admin_server => {
            if let Err(e) = result {
                tracing::error!("admin server error: {e}");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("received shutdown signal, draining...");
            state.kill_switch.trigger_drain();
        }
    }

    tracing::info!("gateway shutdown complete");
}

fn handle_route_command(action: RouteAction) {
    match action {
        RouteAction::List { config: config_path } => {
            let cfg = config::GatewayConfig::load(&config_path).expect("failed to load config");
            let conn = db::init_route_store_db(std::path::Path::new(&cfg.database.route_store_path))
                .expect("failed to open route store");
            let store = routes::store::RouteStore::new(conn);
            let routes = store.list_routes().expect("failed to list routes");
            if routes.is_empty() {
                println!("no routes configured");
            } else {
                for r in &routes {
                    let status = if r.active { "active" } else { "disabled" };
                    println!(
                        "  {} ({}) — {} [{}] v{}",
                        r.route_key, r.display_name, r.model_id, status, r.version
                    );
                }
            }
        }
        RouteAction::Add { file, config: config_path } => {
            let cfg = config::GatewayConfig::load(&config_path).expect("failed to load config");
            let conn = db::init_route_store_db(std::path::Path::new(&cfg.database.route_store_path))
                .expect("failed to open route store");
            let store = routes::store::RouteStore::new(conn);

            let json_str = std::fs::read_to_string(&file)
                .unwrap_or_else(|e| {
                    eprintln!("failed to read {}: {e}", file.display());
                    std::process::exit(1);
                });
            let route: types::Route = serde_json::from_str(&json_str)
                .unwrap_or_else(|e| {
                    eprintln!("invalid route JSON: {e}");
                    std::process::exit(1);
                });
            store.add_route(&route).expect("failed to add route");
            println!("added route: {}", route.route_key);
        }
        RouteAction::Disable { route_key, config: config_path } => {
            let cfg = config::GatewayConfig::load(&config_path).expect("failed to load config");
            let conn = db::init_route_store_db(std::path::Path::new(&cfg.database.route_store_path))
                .expect("failed to open route store");
            let store = routes::store::RouteStore::new(conn);
            store.disable_route(&route_key).expect("failed to disable route");
            println!("disabled route: {route_key}");
        }
        RouteAction::Enable { route_key, config: config_path } => {
            let cfg = config::GatewayConfig::load(&config_path).expect("failed to load config");
            let conn = db::init_route_store_db(std::path::Path::new(&cfg.database.route_store_path))
                .expect("failed to open route store");
            let store = routes::store::RouteStore::new(conn);
            store.enable_route(&route_key).expect("failed to enable route");
            println!("enabled route: {route_key}");
        }
        RouteAction::Update { route_key, field, value, config: config_path } => {
            let cfg = config::GatewayConfig::load(&config_path).expect("failed to load config");
            let conn = db::init_route_store_db(std::path::Path::new(&cfg.database.route_store_path))
                .expect("failed to open route store");
            let store = routes::store::RouteStore::new(conn);
            store.update_route(&route_key, &field, &value).expect("failed to update route");
            println!("updated route {route_key}: {field} = {value}");
        }
        RouteAction::History { route_key, config: config_path } => {
            let cfg = config::GatewayConfig::load(&config_path).expect("failed to load config");
            let conn = db::init_route_store_db(std::path::Path::new(&cfg.database.route_store_path))
                .expect("failed to open route store");
            let store = routes::store::RouteStore::new(conn);
            let history = store.route_history(&route_key).expect("failed to get history");
            if history.is_empty() {
                println!("no history for route: {route_key}");
            } else {
                for r in &history {
                    println!("  v{} — {} ({})", r.version, r.model_id, r.updated_at);
                }
            }
        }
        RouteAction::Rollback { route_key, version, config: config_path } => {
            let cfg = config::GatewayConfig::load(&config_path).expect("failed to load config");
            let conn = db::init_route_store_db(std::path::Path::new(&cfg.database.route_store_path))
                .expect("failed to open route store");
            let store = routes::store::RouteStore::new(conn);
            store.rollback_route(&route_key, version).expect("failed to rollback route");
            println!("rolled back route {route_key} to version {version}");
        }
        RouteAction::Probe { .. } => {
            eprintln!("probe requires a running server — use the admin API");
        }
    }
}

