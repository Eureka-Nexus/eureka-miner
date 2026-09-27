use anyhow::Result;
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::process::Command;
use std::time::{Duration, Instant};
use sysinfo::System;
use tracing::{info, warn};

const APP_NAME: &str = "Eureka Miner";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(
    name = "eureka-miner",
    version,
    about = "Official Eureka Nexus (EKNX) CPU/GPU miner"
)]
struct Args {
    /// Public Solana wallet address used to receive EKNX rewards
    #[arg(long)]
    wallet: Option<String>,

    /// Number of CPU mining threads
    #[arg(long, default_value_t = default_threads())]
    threads: usize,

    /// Hardware mode
    #[arg(long, value_enum, default_value = "both")]
    mode: MiningMode,

    /// Eureka mining server
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    server: String,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum MiningMode {
    Cpu,
    Gpu,
    Both,
}

#[derive(Debug, Serialize)]
struct HardwareInfo {
    cpu: String,
    logical_cores: usize,
    total_memory_gb: f64,
    gpus: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ChallengeResponse {
    challenge: String,
    difficulty: usize,
}

#[derive(Debug, Serialize)]
struct ShareRequest {
    challenge: String,
    nonce: u64,
    hash: String,
}

#[derive(Debug, Deserialize)]
struct ShareResponse {
    accepted: bool,
    message: String,
    total_accepted_shares: u64,
}

fn default_threads() -> usize {
    num_cpus::get().saturating_sub(1).max(1)
}

fn validate_solana_address(address: &str) -> bool {
    match bs58::decode(address).into_vec() {
        Ok(bytes) => bytes.len() == 32,
        Err(_) => false,
    }
}

fn detect_hardware() -> HardwareInfo {
    let mut system = System::new_all();
    system.refresh_all();

    let cpu = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let logical_cores = system.cpus().len();

    let total_memory_gb =
        system.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;

    HardwareInfo {
        cpu,
        logical_cores,
        total_memory_gb,
        gpus: detect_gpus(),
    }
}

#[cfg(target_os = "windows")]
fn detect_gpus() -> Vec<String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name",
        ])
        .output();

    match output {
        Ok(result) if result.status.success() => String::from_utf8_lossy(&result.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect(),

        _ => Vec::new(),
    }
}

#[cfg(not(target_os = "windows"))]
fn detect_gpus() -> Vec<String> {
    let nvidia = Command::new("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader"])
        .output();

    if let Ok(result) = nvidia {
        if result.status.success() {
            let cards: Vec<String> = String::from_utf8_lossy(&result.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToString::to_string)
                .collect();

            if !cards.is_empty() {
                return cards;
            }
        }
    }

    Vec::new()
}

async fn check_server(server: &str) -> bool {
    let url = format!("{}/health", server.trim_end_matches('/'));

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            warn!("Could not create HTTP client: {error}");
            return false;
        }
    };

    match client.get(url).send().await {
        Ok(response) if response.status().is_success() => {
            info!("Mining server: ONLINE");
            true
        }

        Ok(response) => {
            warn!("Mining server returned HTTP {}", response.status());
            false
        }

        Err(_) => {
            warn!("Mining server: OFFLINE");
            false
        }
    }
}

async fn mine_test_share(server: &str) -> Result<()> {
    let client = reqwest::Client::new();

    let challenge_url =
        format!("{}/challenge", server.trim_end_matches('/'));

    let challenge: ChallengeResponse = client
        .get(challenge_url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    println!();
    println!("==============================================");
    println!("         MINING CHALLENGE RECEIVED");
    println!("==============================================");
    println!("Challenge: {}", challenge.challenge);
    println!("Difficulty: {}", challenge.difficulty);
    println!();

    let target = "0".repeat(challenge.difficulty);

    let start = Instant::now();
    let mut nonce: u64 = 0;

    println!("Mining started...");

    loop {
        let input = format!("{}:{}", challenge.challenge, nonce);

        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());

        let result = hasher.finalize();
        let hash = hex::encode(result);

        if hash.starts_with(&target) {
            let elapsed = start.elapsed().as_secs_f64();
            let hashes = nonce + 1;
            let hashrate = hashes as f64 / elapsed.max(0.001);

            println!();
            println!("==============================================");
            println!("              VALID SHARE FOUND");
            println!("==============================================");
            println!("Nonce: {}", nonce);
            println!("Hash: {}", hash);
            println!("Hashes tested: {}", hashes);
            println!("Time: {:.2} seconds", elapsed);
            println!("Hashrate: {:.2} H/s", hashrate);

            let share = ShareRequest {
                challenge: challenge.challenge.clone(),
                nonce,
                hash,
            };

            let share_url =
                format!("{}/share", server.trim_end_matches('/'));

            let response: ShareResponse = client
                .post(share_url)
                .json(&share)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            println!();
            println!("==============================================");
            println!("              SERVER RESPONSE");
            println!("==============================================");
            println!("Accepted: {}", response.accepted);
            println!("Message: {}", response.message);
            println!(
                "Total accepted shares: {}",
                response.total_accepted_shares
            );

            if response.accepted {
                println!();
                println!("*** FIRST EUREKA NEXUS SHARE ACCEPTED ***");
            }

            break;
        }

        nonce = nonce.wrapping_add(1);

        if nonce % 1_000_000 == 0 {
            let elapsed = start.elapsed().as_secs_f64();
            let hashrate = nonce as f64 / elapsed.max(0.001);

            println!(
                "Mining... {:>10} hashes | {:>10.0} H/s",
                nonce,
                hashrate
            );
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    println!();
    println!("================================================");
    println!("              EUREKA NEXUS");
    println!("              {APP_NAME} v{APP_VERSION}");
    println!("              Token: EKNX");
    println!("================================================");
    println!();

    let args = Args::parse();

    if let Some(wallet) = &args.wallet {
        if !validate_solana_address(wallet) {
            anyhow::bail!("Invalid Solana public wallet address.");
        }

        info!("Solana wallet: {}", wallet);
    } else {
        warn!("No wallet configured.");
        warn!("A public Solana address will be required to receive EKNX.");
    }

    let hardware = detect_hardware();

    println!("Hardware detected:");
    println!("CPU: {}", hardware.cpu);
    println!("Logical CPU cores: {}", hardware.logical_cores);
    println!("RAM: {:.2} GB", hardware.total_memory_gb);

    if hardware.gpus.is_empty() {
        println!("GPU: none detected");
    } else {
        for (index, gpu) in hardware.gpus.iter().enumerate() {
            println!("GPU {}: {}", index + 1, gpu);
        }
    }

    let threads = args.threads.min(hardware.logical_cores.max(1));

    println!();
    println!("Configuration:");
    println!("Mode: {:?}", args.mode);
    println!("CPU threads: {}", threads);
    println!("Server: {}", args.server);
    println!();

    let server_online = check_server(&args.server).await;

    if !server_online {
        anyhow::bail!("Mining server is offline.");
    }

    println!();
    println!("Eureka Miner initialized successfully.");
    println!("Starting Eureka Nexus PoW protocol test...");

    mine_test_share(&args.server).await?;

    Ok(())
}