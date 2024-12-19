use std::time::{Duration, SystemTime, UNIX_EPOCH};

use builder_server::BlockId;
use eth2::{reqwest, BeaconNodeHttpClient};
use tokio::time::Instant;
use types::{EthSpec, ExecPayload};

pub async fn get_header<E: EthSpec>(beacon_client: BeaconNodeHttpClient) -> Result<(), String> {
    // Get genesis time from beacon client
    let genesis_time = beacon_client
        .get_beacon_genesis()
        .await
        .map_err(|_| "couldn't get beacon genesis".to_string())?
        .data
        .genesis_time;

    // Get current time since UNIX epoch
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system time before Unix epoch".to_string())?
        .as_secs();

    // Calculate time until next interval
    let time_since_genesis = now.saturating_sub(genesis_time);
    let intervals_passed = time_since_genesis.div_euclid(12);
    let next_interval = genesis_time + (intervals_passed + 1) * 12;
    let wait_duration = Duration::from_secs(next_interval.saturating_sub(now));

    tracing::info!(
        "Starting interval timer in {} seconds",
        wait_duration.as_secs()
    );

    // Create interval timer starting at the next aligned interval
    let mut interval_timer =
        tokio::time::interval_at(Instant::now() + wait_duration, Duration::from_secs(12));

    loop {
        let _ = interval_timer.tick().await;

        process_head::<E>(&beacon_client).await?;
    }
}

async fn process_head<E: EthSpec>(beacon_client: &BeaconNodeHttpClient) -> Result<(), String> {
    let block = beacon_client
        .get_beacon_blocks::<E>(BlockId::Head)
        .await
        .map_err(|_| "couldn't get head".to_string())?
        .ok_or_else(|| "missing head block".to_string())?
        .data;
    let head_execution_hash = block
        .message()
        .body()
        .execution_payload()
        .map_err(|_| "pre-merge block".to_string())?
        .block_hash();
    let slot = block.message().slot();

    tracing::info!(
        "Sending get_header for slot {}, head_execution_hash: {}, block_hash: {}",
        slot,
        head_execution_hash,
        block.canonical_root()
    );

    let url = format!(
        "http://localhost:8650/eth/v1/builder/header/{}/{}/0xa376d9d740b19cb4fbdabdfff995dec77b05ddaaec19fa6900f8a35c7e46c80b45fd049233ce05235d2a92b3ed00961b",
        slot + 1,
        head_execution_hash,
    );

    let client = reqwest::Client::new();
    let _response = client.get(&url).send().await.unwrap().text().await.unwrap();
    // dbg!(&_response);
    Ok(())
}
