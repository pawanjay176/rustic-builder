use clap::Parser;
use eth2::Timeouts;
use execution_layer::Config;
use rustic_builder::builder_impl::RusticBuilder;
use sensitive_url::SensitiveUrl;
use std::ops::Deref;
use std::path::PathBuf;
use std::time::Duration;
use std::{net::Ipv4Addr, sync::Arc};
use task_executor::ShutdownReason;
use tracing::{instrument, Level};
use tracing_core::LevelFilter;
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;
use types::{Address, ChainSpec, MainnetEthSpec};

#[derive(Parser)]
#[clap(about = "Rustic Builder", version = "0.1.0", author = "@pawanjay176")]
struct BuilderConfig {
    #[clap(
        long,
        help = "URL of the execution engine",
        default_value = "http://localhost:8551"
    )]
    execution_endpoint: String,
    #[clap(
        long,
        help = "URL of the beacon node",
        default_value = "http://localhost:5052"
    )]
    beacon_node: String,
    #[clap(
        long,
        help = "File path which contain the corresponding hex-encoded JWT secrets for the provided \
            execution endpoint."
    )]
    jwt_secret: PathBuf,
    #[clap(long, help = "Address to listen on", default_value = "127.0.0.1")]
    address: Ipv4Addr,
    #[clap(long, help = "Port to listen on", default_value_t = 8650)]
    port: u16,
    #[clap(long, short = 'l', help = "Set the log level", default_value = "info")]
    log_level: Level,
    #[clap(
        long,
        help = "Fee recipient to use in case of missing registration.",
        requires("empty-payloads")
    )]
    default_fee_recipient: Option<Address>,
    #[clap(
        long,
        help = "Builder always returns a bid with value set to max",
        default_value_t = false
    )]
    set_max_bid_value: bool,
    #[clap(
        long,
        help = "If set to true, builder will return a bid even for non registered validators",
        default_value_t = false
    )]
    allow_unregistered_validators: bool,
    #[clap(
        long,
        help = "Hex-encoded secret key for the builder. If provided, it will be used as the secret key for the builder."
    )]
    builder_secret_key: Option<String>,
}

#[instrument]
#[tokio::main]
async fn main() -> Result<(), String> {
    let builder_config: BuilderConfig = BuilderConfig::parse();
    let log_level: LevelFilter = builder_config.log_level.into();

    // Initialize logging
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::OFF.into())
        .parse(format!(
            "rustic_builder={},execution_layer={}",
            log_level, log_level
        ))
        .unwrap();

    // Set up the tracing subscriber with the filter
    tracing_subscriber::Registry::default()
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_filter(filter),
        )
        .with(ErrorLayer::default())
        .init();

    tracing::info!("Starting mock relay");

    let beacon_url = SensitiveUrl::parse(builder_config.beacon_node.as_str())
        .map_err(|e| format!("Failed to parse beacon URL: {:?}", e))?;
    let beacon_client =
        eth2::BeaconNodeHttpClient::new(beacon_url, Timeouts::set_all(Duration::from_secs(12)));
    let config = beacon_client
        .get_config_spec::<types::ConfigAndPreset>()
        .await
        .map_err(|e| format!("Failed to get config spec: {:?}", e))?;
    let spec = ChainSpec::from_config::<MainnetEthSpec>(config.data.config())
        .ok_or_else(|| String::from("Unable to parse chain spec from config"))?;
    let builder_secret_key = builder_config
        .builder_secret_key
        .as_ref()
        .map(|key| {
            hex::decode(key).map_err(|e| format!("Failed to decode builder secret key: {:?}", e))
        })
        .transpose()?;
    let url = SensitiveUrl::parse(builder_config.execution_endpoint.as_str())
        .map_err(|e| format!("Failed to parse execution endpoint URL: {:?}", e))?;

    // Convert slog logs from the EL to tracing logs.
    // TODO(pawan): get rid of this abomination once we switch
    // to tracing in lighthouse
    let drain = tracing_slog::TracingSlogDrain;
    let log_root = slog::Logger::root(drain, slog::o!());

    let (shutdown_tx, _shutdown_rx) = futures_channel::mpsc::channel::<ShutdownReason>(1);
    let (_signal, exit) = async_channel::bounded(1);
    let task_executor = task_executor::TaskExecutor::new(
        tokio::runtime::Handle::current(),
        exit,
        log_root.clone(),
        shutdown_tx,
    );

    let config = Config {
        execution_endpoint: Some(url),
        secret_file: Some(builder_config.jwt_secret),
        suggested_fee_recipient: builder_config.default_fee_recipient,
        ..Default::default()
    };

    let el = execution_layer::ExecutionLayer::<MainnetEthSpec>::from_config(
        config,
        task_executor.clone(),
        log_root.clone(),
    )
    .map_err(|e| format!("Failed to create execution layer: {:?}", e))?;

    let spec = Arc::new(spec);
    let mock_builder = execution_layer::test_utils::MockBuilder::new(
        el,
        beacon_client.clone(),
        builder_config.allow_unregistered_validators,
        false,
        builder_config.set_max_bid_value,
        spec.clone(),
        builder_secret_key.as_deref(),
        log_root,
    );
    let rustic_builder = Arc::new(RusticBuilder::new(mock_builder, spec));
    tracing::info!("Initialized mock builder");

    let pubkey = rustic_builder.deref().public_key();
    tracing::info!("Builder pubkey: {pubkey:?}");

    let listener = tokio::net::TcpListener::bind(format!(
        "{}:{}",
        builder_config.address, builder_config.port
    ))
    .await
    .unwrap();

    let builder_preparer = rustic_builder.clone();
    task_executor.spawn(
        {
            async move {
                tracing::info!("Starting preparation service");
                let _result = builder_preparer.prepare_execution_layer().await;
                tracing::error!("Preparation service stopped");
            }
        },
        "preparation service",
    );
    tracing::info!("Listening on {:?}", listener.local_addr());
    let app = builder_server::server::new(rustic_builder);
    task_executor.spawn(
        async {
            tracing::info!("Starting builder server");
            axum::serve(listener, app).await.expect("server failed");
        },
        "rustic_server",
    );

    task_executor.exit().await;
    tracing::info!("Shutdown complete.");

    Ok(())
}
