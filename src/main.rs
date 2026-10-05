use anyhow::{anyhow, Context, Result};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, Request, StatusCode, Uri},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use parking_lot::{Mutex, RwLock};
use randomx_rs::{RandomXCache, RandomXFlag, RandomXVM};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::hash_map::DefaultHasher,
    fs::{self, OpenOptions},
    hash::{Hash, Hasher},
    io::{BufReader, Write},
    net::SocketAddr,
    path::{Path as FsPath, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use sysinfo::System;
use tokio::time::sleep;
use tracing::{info, warn};

const APP_NAME: &str = concat!("Eureka Nexus Miner Official ", env!("CARGO_PKG_VERSION"));
const CHAIN_ID: u64 = 56;
const NETWORK: &str = "BNB Smart Chain Mainnet";
const EKNX: &str = "0xF54913A8d5E2AEBD0B62c6411cCf1b5B4aB069c9";
const GENESIS_MARKET: &str = "0x837dBE1D3b67e8315127c36a119E163e713ee73D";
const DEFAULT_BSC_RPC: &str = "https://bsc-dataseed.bnbchain.org";
const DEFAULT_POOL: &str = "https://pool.eurekanexus.pt";
const GPU_TEMP_WARN_C: f32 = 80.0;
const GPU_TEMP_CUTOFF_C: f32 = 85.0;
const GPU_TEMP_RESTART_C: f32 = 75.0;

fn gpu_temp_warning(temp: f32) -> bool {
    temp.is_finite() && temp >= GPU_TEMP_WARN_C
}

fn gpu_temp_cutoff(temp: f32) -> bool {
    temp.is_finite() && temp >= GPU_TEMP_CUTOFF_C
}

fn gpu_temp_cooled(temp: f32) -> bool {
    temp.is_finite() && temp > 0.0 && temp < GPU_TEMP_RESTART_C
}

fn gpu_restart_blocked(thermal_tripped: bool, temp: f32) -> bool {
    thermal_tripped && !gpu_temp_cooled(temp)
}
const TOKEN_DECIMALS: u32 = 18;
const DOMAIN_CPU: &[u8] = b"EKNX-RANDOMX-V1";
const CLIENT_VERSION: &str = "Eureka-Nexus-Miner-Official/1.0";

#[derive(RustEmbed)]
#[folder = "web/"]
struct WebAssets;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct MinerConfig {
    wallet: String,
    worker_name: String,
    pool_url: String,
    bsc_rpc: String,
    threads: usize,
    mining_mode: String,
    gpu_engine_path: String,
    gpu_stratum_addr: String,
    gpu_backend: String,
    auto_start: bool,
    open_dashboard: bool,
    dashboard_port: u16,
}

impl Default for MinerConfig {
    fn default() -> Self {
        Self {
            wallet: String::new(),
            worker_name: format!("eureka-{:08x}", machine_fingerprint() as u32),
            pool_url: DEFAULT_POOL.into(),
            bsc_rpc: DEFAULT_BSC_RPC.into(),
            threads: num_cpus::get().saturating_sub(1).max(1),
            mining_mode: "GPU".into(),
            gpu_engine_path: default_gpu_engine_path(),
            gpu_stratum_addr: "pool.eurekanexus.pt:3333".into(),
            gpu_backend: "AUTO".into(),
            auto_start: false,
            open_dashboard: true,
            dashboard_port: 8077,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChainSnapshot {
    updated_at: String,
    rpc_ok: bool,
    chain_id: u64,
    total_supply_eknx: String,
    wallet_balance_eknx: String,
    claimed_entitlement_wei: String,
    claimed_entitlement_eknx: String,
    market_balance_eknx: String,
    wallet_bnb: String,
    error: String,
}

impl Default for ChainSnapshot {
    fn default() -> Self {
        Self {
            updated_at: String::new(),
            rpc_ok: false,
            chain_id: 0,
            total_supply_eknx: "0".into(),
            wallet_balance_eknx: "0".into(),
            claimed_entitlement_wei: "0".into(),
            claimed_entitlement_eknx: "0".into(),
            market_balance_eknx: "0".into(),
            wallet_bnb: "0".into(),
            error: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PublicStatsSnapshot {
    connected_miners: u64,
    active_workers: u64,
    active_window_seconds: u64,
    total_wallets_ever: u64,
    top_miners: Vec<Value>,
    latest_claims: Vec<Value>,
    claim_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PoolSnapshot {
    updated_at: String,
    online: bool,
    server_name: String,
    server_version: String,
    network: String,
    chain_id: u64,
    accept_shares: bool,
    emission_enabled: bool,
    mining_start_unix: i64,
    current_epoch: u64,
    current_day: u64,
    era: u32,
    epoch_seconds: u64,
    gpu_epoch_cap_eknx: String,
    cpu_epoch_cap_eknx: String,
    minimum_claim_eknx: String,
    #[serde(default)]
    pool_fee_percent: u64,
    #[serde(default)]
    pool_fee_bps: u64,
    #[serde(default)]
    pool_fee_wallet: String,
    accepted_gpu_shares: u64,
    accepted_cpu_shares: u64,
    rejected_shares: u64,
    cumulative_total_eknx: String,
    cumulative_gpu_eknx: String,
    cumulative_cpu_eknx: String,
    last_share_at: String,
    public_stats: PublicStatsSnapshot,
    error: String,
}

impl Default for PoolSnapshot {
    fn default() -> Self {
        Self {
            updated_at: String::new(),
            online: false,
            server_name: String::new(),
            server_version: String::new(),
            network: NETWORK.into(),
            chain_id: CHAIN_ID,
            accept_shares: false,
            emission_enabled: false,
            mining_start_unix: 0,
            current_epoch: 0,
            current_day: 0,
            era: 0,
            epoch_seconds: 60,
            gpu_epoch_cap_eknx: "5".into(),
            cpu_epoch_cap_eknx: "0.8".into(),
            minimum_claim_eknx: String::new(),
            pool_fee_percent: 0,
            pool_fee_bps: 0,
            pool_fee_wallet: String::new(),
            accepted_gpu_shares: 0,
            accepted_cpu_shares: 0,
            rejected_shares: 0,
            cumulative_total_eknx: "0".into(),
            cumulative_gpu_eknx: "0".into(),
            cumulative_cpu_eknx: "0".into(),
            last_share_at: String::new(),
            public_stats: PublicStatsSnapshot::default(),
            error: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MinerStatus {
    running: bool,
    engine_ready: bool,
    cpu_engine_ready: bool,
    gpu_engine_ready: bool,
    gpu_engine_path: String,
    gpu_stratum_addr: String,
    gpu_backend: String,
    phase: String,
    mode: String,
    wallet_valid: bool,
    cpu_name: String,
    cpu_usage: f32,
    ram_used_gb: f64,
    ram_total_gb: f64,
    gpu_name: String,
    gpu_util: f32,
    gpu_temp_c: f32,
    gpu_power_w: f32,
    gpu_memory_used_mb: f32,
    gpu_memory_total_mb: f32,
    cpu_hashrate_hs: f64,
    gpu_hashrate_hs: f64,
    message: String,
    chain: ChainSnapshot,
    pool: PoolSnapshot,
}

impl Default for MinerStatus {
    fn default() -> Self {
        Self {
            running: false,
            engine_ready: false,
            cpu_engine_ready: true,
            gpu_engine_ready: false,
            gpu_engine_path: default_gpu_engine_path(),
            gpu_stratum_addr: "pool.eurekanexus.pt:3333".into(),
            gpu_backend: "AUTO".into(),
            phase: "BSC READY · ENGINE CHECK".into(),
            mode: "GPU KAWPOW + CPU RandomX".into(),
            wallet_valid: false,
            cpu_name: String::new(),
            cpu_usage: 0.0,
            ram_used_gb: 0.0,
            ram_total_gb: 0.0,
            gpu_name: "Not detected".into(),
            gpu_util: 0.0,
            gpu_temp_c: 0.0,
            gpu_power_w: 0.0,
            gpu_memory_used_mb: 0.0,
            gpu_memory_total_mb: 0.0,
            cpu_hashrate_hs: 0.0,
            gpu_hashrate_hs: 0.0,
            message: "RandomX CPU engine is integrated. Install the official KAWPOW GPU engine to enable GPU mining.".into(),
            chain: ChainSnapshot::default(),
            pool: PoolSnapshot::default(),
        }
    }
}

#[derive(Clone)]
struct AppState {
    config: Arc<RwLock<MinerConfig>>,
    status: Arc<RwLock<MinerStatus>>,
    stop: Arc<AtomicBool>,
    run_generation: Arc<AtomicU64>,
    cpu_hashes_total: Arc<AtomicU64>,
    gpu_hashrate_bits: Arc<AtomicU64>,
    gpu_child: Arc<Mutex<Option<Child>>>,
    gpu_thermal_tripped: Arc<AtomicBool>,
}

fn app_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("EurekaNexus")
        .join("MinerOfficial1")
}

fn config_path() -> PathBuf {
    app_dir().join("miner-config.json")
}

fn default_gpu_engine_path() -> String {
    if cfg!(target_os = "windows") {
        "engines\\kawpow\\kawpowminer.exe".into()
    } else {
        "engines/kawpow/kawpowminer".into()
    }
}

fn normalize_gpu_backend(raw: &str) -> &'static str {
    match raw.trim().to_ascii_uppercase().as_str() {
        "OPENCL" | "AMD" => "OPENCL",
        "CUDA" | "NVIDIA" => "CUDA",
        _ => "AUTO",
    }
}

fn gpu_backend_arg(cfg: &MinerConfig) -> &'static str {
    match normalize_gpu_backend(&cfg.gpu_backend) {
        "OPENCL" => "-G",
        "CUDA" => "-U",
        _ => {
            if Command::new("nvidia-smi")
                .arg("-L")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                "-U"
            } else {
                "-G"
            }
        }
    }
}

fn load_config() -> MinerConfig {
    if let Ok(raw) = fs::read_to_string(config_path()) {
        if let Ok(cfg) = serde_json::from_str(&raw) {
            return cfg;
        }
    }
    MinerConfig::default()
}

fn save_config(cfg: &MinerConfig) -> Result<()> {
    fs::create_dir_all(app_dir())?;
    fs::write(config_path(), serde_json::to_vec_pretty(cfg)?)?;
    Ok(())
}

fn machine_fingerprint() -> u64 {
    let source = format!(
        "{}|{}|{}",
        std::env::var("COMPUTERNAME").unwrap_or_default(),
        std::env::var("USERNAME").unwrap_or_default(),
        std::env::consts::ARCH
    );
    let mut h = DefaultHasher::new();
    source.hash(&mut h);
    h.finish()
}

fn valid_evm_address(addr: &str) -> bool {
    let s = addr.trim();
    s.len() == 42 && s.starts_with("0x") && s[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn normalize_mode(raw: &str) -> &'static str {
    match raw.to_ascii_uppercase().as_str() {
        "CPU" => "CPU",
        "BOTH" | "CPU + GPU" => "BOTH",
        _ => "GPU",
    }
}

fn parse_f32(s: Option<&str>) -> f32 {
    s.unwrap_or("0").trim().parse::<f32>().unwrap_or(0.0)
}

fn gpu_snapshot() -> (String, f32, f32, f32, f32, f32) {
    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,temperature.gpu,power.draw,memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output();
    if let Ok(out) = out {
        if out.status.success() {
            let raw = String::from_utf8_lossy(&out.stdout);
            if let Some(line) = raw.lines().next() {
                let p: Vec<&str> = line.split(',').map(|x| x.trim()).collect();
                if p.len() >= 6 {
                    return (
                        p[0].to_string(),
                        parse_f32(p.get(1).copied()),
                        parse_f32(p.get(2).copied()),
                        parse_f32(p.get(3).copied()),
                        parse_f32(p.get(4).copied()),
                        parse_f32(p.get(5).copied()),
                    );
                }
            }
        }
    }
    ("Not detected".into(), 0.0, 0.0, 0.0, 0.0, 0.0)
}

fn parse_hex_u128(s: &str) -> Result<u128> {
    let clean = s.trim().trim_start_matches("0x");
    if clean.is_empty() {
        return Ok(0);
    }
    u128::from_str_radix(clean, 16).map_err(|e| anyhow!("invalid hex quantity: {e}"))
}

fn units_to_decimal(v: u128, decimals: u32, max_fraction: usize) -> String {
    if decimals == 0 {
        return v.to_string();
    }
    let base = 10u128.pow(decimals);
    let whole = v / base;
    let frac = v % base;
    if frac == 0 || max_fraction == 0 {
        return whole.to_string();
    }
    let mut frac_s = format!("{:0width$}", frac, width = decimals as usize);
    frac_s.truncate(max_fraction.min(frac_s.len()));
    while frac_s.ends_with('0') {
        frac_s.pop();
    }
    if frac_s.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{frac_s}")
    }
}

async fn json_rpc(rpc: &str, method: &str, params: Value) -> Result<Value> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let body = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
    let response: Value = client
        .post(rpc)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if !response["error"].is_null() {
        return Err(anyhow!("RPC error: {}", response["error"]));
    }
    Ok(response["result"].clone())
}

async fn eth_call(rpc: &str, to: &str, data: &str) -> Result<String> {
    let v = json_rpc(rpc, "eth_call", json!([{"to":to,"data":data}, "latest"])).await?;
    v.as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("eth_call returned no hex result"))
}

async fn erc20_balance(rpc: &str, token: &str, owner: &str) -> Result<u128> {
    if !valid_evm_address(owner) {
        return Ok(0);
    }
    let data = format!("0x70a08231{:0>64}", owner.trim_start_matches("0x"));
    parse_hex_u128(&eth_call(rpc, token, &data).await?)
}

async fn erc20_total_supply(rpc: &str, token: &str) -> Result<u128> {
    parse_hex_u128(&eth_call(rpc, token, "0x18160ddd").await?)
}

#[derive(Debug, Clone, Deserialize)]
struct CpuChallenge {
    id: String,
    algorithm: String,
    seed_hex: String,
    epoch_index: u64,
    difficulty_bits: u32,
    expires_unix: i64,
    randomx_key_hex: Option<String>,
    verifier_protocol: u32,
}

#[derive(Debug, Deserialize)]
struct CpuShareReply {
    accepted: bool,
    reason: String,
}

struct CpuRandomXEngine {
    key: Vec<u8>,
    vm: Option<RandomXVM>,
}

impl CpuRandomXEngine {
    fn new() -> Self {
        Self {
            key: Vec::new(),
            vm: None,
        }
    }

    fn ensure_key(&mut self, key: &[u8]) -> Result<()> {
        if self.vm.is_some() && self.key == key {
            return Ok(());
        }
        let flags = RandomXFlag::get_recommended_flags();
        let cache =
            RandomXCache::new(flags, key).map_err(|e| anyhow!("RandomX cache init failed: {e}"))?;
        let vm = RandomXVM::new(flags, Some(cache), None)
            .map_err(|e| anyhow!("RandomX VM init failed: {e}"))?;
        self.key = key.to_vec();
        self.vm = Some(vm);
        Ok(())
    }

    fn hash(&self, input: &[u8]) -> Result<Vec<u8>> {
        self.vm
            .as_ref()
            .ok_or_else(|| anyhow!("RandomX VM unavailable"))?
            .calculate_hash(input)
            .map_err(|e| anyhow!("RandomX hash failed: {e}"))
    }
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    let clean = value.trim().trim_start_matches("0x");
    hex::decode(clean).map_err(|e| anyhow!("invalid hex: {e}"))
}

fn wallet_bytes(addr: &str) -> Result<[u8; 20]> {
    if !valid_evm_address(addr) {
        return Err(anyhow!("invalid BSC wallet"));
    }
    let raw = hex::decode(&addr.trim()[2..])?;
    let mut out = [0u8; 20];
    out.copy_from_slice(&raw);
    Ok(out)
}

fn randomx_input(
    seed: &[u8; 32],
    wallet: &str,
    worker: &str,
    epoch: u64,
    nonce: u64,
) -> Result<Vec<u8>> {
    let wb = wallet_bytes(wallet)?;
    let worker_bytes = worker.as_bytes();
    if worker_bytes.len() > u16::MAX as usize {
        return Err(anyhow!("worker name too long"));
    }
    let mut pre = Vec::with_capacity(64 + worker_bytes.len());
    pre.extend_from_slice(DOMAIN_CPU);
    pre.extend_from_slice(seed);
    pre.extend_from_slice(&wb);
    pre.extend_from_slice(&(worker_bytes.len() as u16).to_be_bytes());
    pre.extend_from_slice(worker_bytes);
    pre.extend_from_slice(&epoch.to_be_bytes());
    pre.extend_from_slice(&nonce.to_le_bytes());
    Ok(pre)
}

fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut bits = 0u32;
    for b in bytes {
        if *b == 0 {
            bits += 8;
        } else {
            bits += b.leading_zeros();
            break;
        }
    }
    bits
}

fn cpu_worker_loop(state: AppState, run_id: u64, worker_index: usize) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(35))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("RandomX worker {worker_index}: HTTP client error: {e}");
            return;
        }
    };
    let mut engine = CpuRandomXEngine::new();

    while state.run_generation.load(Ordering::SeqCst) == run_id
        && !state.stop.load(Ordering::SeqCst)
    {
        let cfg = state.config.read().clone();
        let base = cfg.pool_url.trim_end_matches('/').to_string();
        let challenge = client
            .get(format!("{base}/v1/challenge"))
            .query(&[
                ("wallet", cfg.wallet.as_str()),
                ("worker", cfg.worker_name.as_str()),
                ("lane", "cpu"),
            ])
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json::<CpuChallenge>());

        let c = match challenge {
            Ok(c) => c,
            Err(e) => {
                warn!("RandomX worker {worker_index}: challenge unavailable: {e}");
                thread::sleep(Duration::from_secs(2));
                continue;
            }
        };
        if c.algorithm != "RANDOMX-EUREKA-V1" || c.verifier_protocol != 1 {
            warn!("RandomX worker {worker_index}: unsupported pool challenge");
            thread::sleep(Duration::from_secs(2));
            continue;
        }

        let seed_raw = match decode_hex(&c.seed_hex) {
            Ok(v) if v.len() == 32 => v,
            _ => {
                warn!("RandomX worker {worker_index}: bad seed");
                continue;
            }
        };
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&seed_raw);
        let key = match c.randomx_key_hex.as_deref().map(decode_hex) {
            Some(Ok(v)) => v,
            _ => {
                warn!("RandomX worker {worker_index}: missing RandomX key");
                continue;
            }
        };
        if let Err(e) = engine.ensure_key(&key) {
            warn!("RandomX worker {worker_index}: {e:#}");
            thread::sleep(Duration::from_secs(2));
            continue;
        }

        let mut nonce = (worker_index as u64) << 56;
        let mut pending_hashes = 0u64;
        loop {
            if state.run_generation.load(Ordering::Relaxed) != run_id
                || state.stop.load(Ordering::Relaxed)
            {
                if pending_hashes > 0 {
                    state
                        .cpu_hashes_total
                        .fetch_add(pending_hashes, Ordering::Relaxed);
                }
                return;
            }
            if chrono::Utc::now().timestamp() >= c.expires_unix {
                if pending_hashes > 0 {
                    state
                        .cpu_hashes_total
                        .fetch_add(pending_hashes, Ordering::Relaxed);
                }
                break;
            }
            let input =
                match randomx_input(&seed, &cfg.wallet, &cfg.worker_name, c.epoch_index, nonce) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("RandomX worker {worker_index}: input error: {e}");
                        break;
                    }
                };
            let hash = match engine.hash(&input) {
                Ok(h) => h,
                Err(e) => {
                    warn!("RandomX worker {worker_index}: hash error: {e}");
                    break;
                }
            };
            pending_hashes += 1;
            if pending_hashes >= 1024 {
                state
                    .cpu_hashes_total
                    .fetch_add(pending_hashes, Ordering::Relaxed);
                pending_hashes = 0;
            }

            if hash.len() == 32 && leading_zero_bits(&hash) >= c.difficulty_bits {
                if pending_hashes > 0 {
                    state
                        .cpu_hashes_total
                        .fetch_add(pending_hashes, Ordering::Relaxed);
                }
                let hashrate = state.status.read().cpu_hashrate_hs;
                let body = json!({
                    "challenge_id": c.id,
                    "wallet": cfg.wallet,
                    "worker_name": cfg.worker_name,
                    "nonce": nonce,
                    "hash_hex": format!("0x{}", hex::encode(&hash)),
                    "mix_hash_hex": Value::Null,
                    "hashrate_hs": hashrate,
                    "client_version": CLIENT_VERSION
                });
                let submit = client
                    .post(format!("{base}/v1/share/cpu"))
                    .json(&body)
                    .send()
                    .and_then(|r| r.error_for_status())
                    .and_then(|r| r.json::<CpuShareReply>());
                match submit {
                    Ok(reply) if reply.accepted => {
                        info!("RandomX share accepted (worker {worker_index})")
                    }
                    Ok(reply) => warn!("RandomX share rejected: {}", reply.reason),
                    Err(e) => warn!("RandomX share submit error: {e}"),
                }
                break;
            }
            nonce = nonce.wrapping_add(1);
        }
    }
}

fn strip_ansi_codes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.peek().copied() == Some('[') {
                chars.next();

                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            continue;
        }

        out.push(ch);
    }

    out
}

fn parse_hashrate_line(line: &str) -> Option<f64> {
    let cleaned = strip_ansi_codes(line).replace(',', ".");
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();

    // kawpowminer prefixes its periodic mining statistics with "m".
    // This prevents "Difficulty : 268.43 Mh" from being mistaken
    // for the GPU hashrate.
    if tokens.first().copied() != Some("m") {
        return None;
    }

    for pair in tokens.windows(2) {
        let Ok(value) = pair[0].parse::<f64>() else {
            continue;
        };

        let unit = pair[1]
            .trim_matches(|c: char| c == '[' || c == ']' || c == ':' || c == ',')
            .to_ascii_lowercase();

        let multiplier = match unit.as_str() {
            "gh/s" | "gh" => 1e9,
            "mh/s" | "mh" => 1e6,
            "kh/s" | "kh" => 1e3,
            "h/s" | "h" => 1.0,
            _ => continue,
        };

        return Some(value * multiplier);
    }

    None
}

fn pipe_gpu_log<R: std::io::Read + Send + 'static>(
    reader: R,
    state: AppState,
    label: &'static str,
) {
    thread::spawn(move || {
        let _ = fs::create_dir_all("data");
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open("data/kawpow-engine.log");
        let log = log.ok().map(|f| Arc::new(Mutex::new(f)));

        let mut reader = BufReader::new(reader);
        let mut chunk = [0u8; 4096];
        let mut pending = Vec::<u8>::new();

        let process_line = |line: &str| {
            let cleaned = strip_ansi_codes(line);

            if cleaned.trim().is_empty() {
                return;
            }

            if cleaned.contains("Disconnected from")
                || cleaned.contains("No connection. Suspend mining")
                || cleaned.contains("Connection refused")
            {
                state
                    .gpu_hashrate_bits
                    .store(0f64.to_bits(), Ordering::Relaxed);
            } else if let Some(hs) = parse_hashrate_line(&cleaned) {
                state
                    .gpu_hashrate_bits
                    .store(hs.to_bits(), Ordering::Relaxed);
            }

            if let Some(file) = &log {
                let mut f = file.lock();
                let _ = writeln!(f, "[{label}] {cleaned}");
            }
        };

        loop {
            match std::io::Read::read(&mut reader, &mut chunk) {
                Ok(0) => {
                    if !pending.is_empty() {
                        let line = String::from_utf8_lossy(&pending).into_owned();
                        process_line(&line);
                    }
                    break;
                }

                Ok(n) => {
                    for &byte in &chunk[..n] {
                        if byte == b'\r' || byte == b'\n' {
                            if !pending.is_empty() {
                                let line = String::from_utf8_lossy(&pending).into_owned();
                                pending.clear();
                                process_line(&line);
                            }
                        } else {
                            pending.push(byte);
                        }
                    }
                }

                Err(_) => break,
            }
        }
    });
}
fn start_gpu_engine(state: &AppState, cfg: &MinerConfig) -> Result<()> {
    let path = FsPath::new(&cfg.gpu_engine_path);
    if !path.is_file() {
        return Err(anyhow!(
            "KAWPOW engine not found at {}. Run ./INSTALL_KAWPOW_ENGINE_LINUX.sh",
            cfg.gpu_engine_path
        ));
    }
    let addr = cfg
        .gpu_stratum_addr
        .trim()
        .trim_start_matches("stratum+tcp://")
        .trim_start_matches("stratum1+tcp://");
    if addr.is_empty() || !addr.contains(':') {
        return Err(anyhow!("gpu_stratum_addr must be host:port"));
    }
    let login = format!("{}.{}", cfg.wallet, cfg.worker_name);
    let pool = format!("stratum+tcp://{login}@{addr}");
    let mut cmd = Command::new(path);
    cmd.arg(gpu_backend_arg(cfg))
        .arg("-P")
        .arg(pool)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .with_context(|| format!("cannot launch {}", cfg.gpu_engine_path))?;
    if let Some(out) = child.stdout.take() {
        pipe_gpu_log(out, state.clone(), "OUT");
    }
    if let Some(err) = child.stderr.take() {
        pipe_gpu_log(err, state.clone(), "ERR");
    }
    *state.gpu_child.lock() = Some(child);
    Ok(())
}

fn stop_gpu_engine(state: &AppState) {
    if let Some(mut child) = state.gpu_child.lock().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    state
        .gpu_hashrate_bits
        .store(0f64.to_bits(), Ordering::Relaxed);
}

fn launch_cpu_workers(state: AppState, run_id: u64, threads: usize) {
    for i in 0..threads.max(1) {
        let s = state.clone();
        let _ = thread::Builder::new()
            .name(format!("eureka-randomx-{i}"))
            .spawn(move || cpu_worker_loop(s, run_id, i));
    }
}

async fn telemetry_loop(state: AppState) {
    let mut sys = System::new_all();
    let mut last_cpu_hashes = state.cpu_hashes_total.load(Ordering::Relaxed);
    let mut last_tick = Instant::now();
    loop {
        sys.refresh_all();
        let used = sys.used_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        let total = sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
        let cpu = sys.global_cpu_usage();
        let cpu_name = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| std::env::consts::ARCH.to_string());
        let (gpu_name, gpu_util, gpu_temp, gpu_power, gpu_used, gpu_total) = gpu_snapshot();
        let cfg = state.config.read().clone();
        let wallet_valid = valid_evm_address(&cfg.wallet);
        let gpu_ready = FsPath::new(&cfg.gpu_engine_path).is_file();
        let mode = normalize_mode(&cfg.mining_mode);
        let engine_ready = match mode {
            "CPU" => true,
            "BOTH" => gpu_ready,
            _ => gpu_ready,
        };

        let currently_running = state.status.read().running;
        let mut thermal_tripped = state.gpu_thermal_tripped.load(Ordering::Relaxed);

        if thermal_tripped && !currently_running && gpu_temp_cooled(gpu_temp) {
            state.gpu_thermal_tripped.store(false, Ordering::Relaxed);
            thermal_tripped = false;
        }

        if !thermal_tripped && currently_running && mode != "CPU" && gpu_temp_cutoff(gpu_temp) {
            let gpu_process_running = state.gpu_child.lock().is_some();

            if gpu_process_running {
                warn!(
                    "GPU thermal safety cutoff: {:.0}C >= {:.0}C",
                    gpu_temp, GPU_TEMP_CUTOFF_C
                );

                stop_gpu_engine(&state);

                state.gpu_thermal_tripped.store(true, Ordering::Relaxed);

                thermal_tripped = true;
            }
        }

        // Automatic KAWPOW recovery after a thermal cutoff.
        // The mining session remains active; GPU resumes only after
        // cooling well below the 85C emergency limit.
        if thermal_tripped && currently_running && mode != "CPU" && gpu_temp_cooled(gpu_temp) {
            let gpu_process_running = state.gpu_child.lock().is_some();

            if !gpu_process_running {
                match start_gpu_engine(&state, &cfg) {
                    Ok(()) => {
                        info!(
                            "GPU cooled to {:.0}C (< {:.0}C). Automatically restarting KAWPOW.",
                            gpu_temp, GPU_TEMP_RESTART_C
                        );

                        state.gpu_thermal_tripped.store(false, Ordering::Relaxed);

                        thermal_tripped = false;
                    }
                    Err(e) => {
                        warn!(
                            "Automatic KAWPOW restart failed after thermal cutoff: {}",
                            e
                        );
                    }
                }
            }
        }

        let now_hashes = state.cpu_hashes_total.load(Ordering::Relaxed);
        let elapsed = last_tick.elapsed().as_secs_f64().max(0.001);
        let cpu_hashrate = now_hashes.saturating_sub(last_cpu_hashes) as f64 / elapsed;
        last_cpu_hashes = now_hashes;
        last_tick = Instant::now();
        let gpu_hashrate = f64::from_bits(state.gpu_hashrate_bits.load(Ordering::Relaxed));

        let gpu_exited = {
            let mut guard = state.gpu_child.lock();
            let exited = match guard.as_mut() {
                Some(child) => matches!(child.try_wait(), Ok(Some(_))),
                None => false,
            };
            if exited {
                guard.take();
            }
            exited
        };
        if gpu_exited {
            state
                .gpu_hashrate_bits
                .store(0f64.to_bits(), Ordering::Relaxed);
        }

        {
            let mut s = state.status.write();
            s.cpu_name = cpu_name;
            s.cpu_usage = cpu;
            s.ram_used_gb = used;
            s.ram_total_gb = total;
            s.gpu_name = gpu_name;
            s.gpu_util = gpu_util;
            s.gpu_temp_c = gpu_temp;
            s.gpu_power_w = gpu_power;
            s.gpu_memory_used_mb = gpu_used;
            s.gpu_memory_total_mb = gpu_total;
            s.wallet_valid = wallet_valid;
            s.cpu_hashrate_hs = if s.running { cpu_hashrate } else { 0.0 };
            s.gpu_hashrate_hs = if s.running { gpu_hashrate } else { 0.0 };
            s.cpu_engine_ready = true;
            s.gpu_engine_ready = gpu_ready;
            s.gpu_engine_path = cfg.gpu_engine_path.clone();
            s.gpu_stratum_addr = cfg.gpu_stratum_addr.clone();
            s.gpu_backend = normalize_gpu_backend(&cfg.gpu_backend).into();
            s.engine_ready = engine_ready;
            if thermal_tripped && mode != "CPU" {
                s.gpu_hashrate_hs = 0.0;

                if mode == "BOTH" {
                    s.phase = "THERMAL SAFETY · GPU STOPPED · CPU ACTIVE".into();
                    s.message = format!(
                        "KAWPOW paused for GPU thermal safety at the 85°C limit. Current GPU temperature: {:.0}°C. RandomX CPU mining remains active. KAWPOW will restart automatically below {:.0}°C.",
                        gpu_temp,
                        GPU_TEMP_RESTART_C
                    );
                } else {
                    s.phase = "THERMAL SAFETY · GPU STOPPED".into();
                    s.message = format!(
                        "KAWPOW paused for GPU thermal safety at the 85°C limit. Current GPU temperature: {:.0}°C. KAWPOW will restart automatically below {:.0}°C.",
                        gpu_temp,
                        GPU_TEMP_RESTART_C
                    );
                }
            } else if !s.running {
                if engine_ready {
                    s.phase = "BSC READY · REAL ENGINES READY".into();
                    s.message = if gpu_ready {
                        "RandomX CPU + KAWPOW GPU engines are ready.".into()
                    } else {
                        "RandomX CPU engine is ready.".into()
                    };
                } else {
                    s.phase = "BSC READY · GPU ENGINE REQUIRED".into();
                    s.message = format!("Install KAWPOW engine: {}", cfg.gpu_engine_path);
                }
            } else if gpu_exited && mode != "CPU" {
                s.phase = "GPU ERROR · KAWPOW STOPPED".into();
                s.message = "KAWPOW GPU engine exited. Check data/kawpow-engine.log.".into();
            } else if mode != "CPU" && gpu_temp_warning(gpu_temp) {
                s.phase = "WARNING · GPU HOT".into();
                s.message = format!(
                    "GPU temperature is {:.0}°C. Automatic KAWPOW cutoff occurs at {:.0}°C.",
                    gpu_temp, GPU_TEMP_CUTOFF_C
                );
            } else if (mode == "GPU" || mode == "BOTH") && gpu_hashrate <= 0.0 {
                s.phase = "CONNECTING · KAWPOW".into();
                s.message =
                    "KAWPOW started. Waiting for Stratum connection, DAG generation and real hashrate."
                        .into();
            } else {
                match mode {
                    "CPU" => {
                        s.phase = "MINING · RANDOMX ACTIVE".into();
                        s.message = "RandomX CPU mining is active.".into();
                    }
                    "BOTH" => {
                        s.phase = "MINING · KAWPOW + RANDOMX ACTIVE".into();
                        s.message = format!(
                            "GPU mining active at {:.2} MH/s. RandomX CPU mining is also running.",
                            gpu_hashrate / 1_000_000.0
                        );
                    }
                    _ => {
                        s.phase = "MINING · KAWPOW ACTIVE".into();
                        s.message = format!(
                            "KAWPOW GPU mining active at {:.2} MH/s.",
                            gpu_hashrate / 1_000_000.0
                        );
                    }
                }
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
}

async fn eknx_claimed_entitlement(rpc: &str, token: &str, owner: &str) -> Result<u128> {
    if !valid_evm_address(owner) {
        return Ok(0);
    }

    let owner_hex = owner.trim().trim_start_matches("0x").to_ascii_lowercase();

    let address_word = format!("{:0>64}", owner_hex);

    let data = format!("0x380e2b71{}", address_word);

    let value = json_rpc(
        rpc,
        "eth_call",
        json!([
            {
                "to": token,
                "data": data
            },
            "latest"
        ]),
    )
    .await?;

    parse_hex_u128(value.as_str().unwrap_or("0x0"))
}

async fn chain_loop(state: AppState) {
    loop {
        let cfg = state.config.read().clone();
        let snap = async {
            let chain_hex = json_rpc(&cfg.bsc_rpc, "eth_chainId", json!([])).await?;
            let chain_id = parse_hex_u128(chain_hex.as_str().unwrap_or("0x0"))? as u64;
            let supply = erc20_total_supply(&cfg.bsc_rpc, EKNX).await?;
            let market = erc20_balance(&cfg.bsc_rpc, EKNX, GENESIS_MARKET).await?;
            let wallet_bal = if valid_evm_address(&cfg.wallet) {
                erc20_balance(&cfg.bsc_rpc, EKNX, &cfg.wallet).await?
            } else {
                0
            };

            let claimed_entitlement = if valid_evm_address(&cfg.wallet) {
                eknx_claimed_entitlement(&cfg.bsc_rpc, EKNX, &cfg.wallet).await?
            } else {
                0
            };

            let bnb = if valid_evm_address(&cfg.wallet) {
                let v = json_rpc(
                    &cfg.bsc_rpc,
                    "eth_getBalance",
                    json!([cfg.wallet, "latest"]),
                )
                .await?;
                parse_hex_u128(v.as_str().unwrap_or("0x0"))?
            } else {
                0
            };
            Ok::<ChainSnapshot, anyhow::Error>(ChainSnapshot {
                updated_at: chrono::Utc::now().to_rfc3339(),
                rpc_ok: chain_id == CHAIN_ID,
                chain_id,
                total_supply_eknx: units_to_decimal(supply, TOKEN_DECIMALS, 6),
                wallet_balance_eknx: units_to_decimal(wallet_bal, TOKEN_DECIMALS, 8),
                claimed_entitlement_wei: claimed_entitlement.to_string(),
                claimed_entitlement_eknx: units_to_decimal(claimed_entitlement, TOKEN_DECIMALS, 8),
                market_balance_eknx: units_to_decimal(market, TOKEN_DECIMALS, 6),
                wallet_bnb: units_to_decimal(bnb, 18, 8),
                error: if chain_id == CHAIN_ID {
                    String::new()
                } else {
                    format!("Unexpected chain ID: {chain_id}")
                },
            })
        }
        .await;
        match snap {
            Ok(snap) => state.status.write().chain = snap,
            Err(e) => {
                let mut s = state.status.write();
                s.chain.rpc_ok = false;
                s.chain.error = e.to_string();
                s.chain.updated_at = chrono::Utc::now().to_rfc3339();
            }
        }
        sleep(Duration::from_secs(12)).await;
    }
}

async fn pool_loop(state: AppState) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(7))
        .build()
        .expect("HTTP client");
    loop {
        let cfg = state.config.read().clone();
        let base = cfg.pool_url.trim_end_matches('/').to_string();
        let result = async {
            let health: Value = client
                .get(format!("{base}/health"))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let network: Value = client
                .get(format!("{base}/v1/network"))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let public_stats: PublicStatsSnapshot =
                match client.get(format!("{base}/v1/public/stats")).send().await {
                    Ok(r) if r.status().is_success() => r.json().await.unwrap_or_default(),
                    _ => PublicStatsSnapshot::default(),
                };

            let account: Value = if valid_evm_address(&cfg.wallet) {
                match client
                    .get(format!("{base}/v1/account/{}", cfg.wallet))
                    .send()
                    .await
                {
                    Ok(r) if r.status().is_success() => {
                        r.json().await.unwrap_or_else(|_| json!({}))
                    }
                    _ => json!({}),
                }
            } else {
                json!({})
            };

            Ok::<PoolSnapshot, anyhow::Error>(PoolSnapshot {
                updated_at: chrono::Utc::now().to_rfc3339(),
                online: health["ok"].as_bool().unwrap_or(false),
                server_name: health["name"].as_str().unwrap_or(APP_NAME).to_string(),
                server_version: health["version"].as_str().unwrap_or("").to_string(),
                network: health["network"].as_str().unwrap_or(NETWORK).to_string(),
                chain_id: health["chain_id"].as_u64().unwrap_or(CHAIN_ID),
                accept_shares: network["accept_shares"].as_bool().unwrap_or(false),
                emission_enabled: network["emission_enabled"].as_bool().unwrap_or(false),
                mining_start_unix: health["mining_start_unix"].as_i64().unwrap_or(0),
                current_epoch: network["current_epoch"].as_u64().unwrap_or(0),
                current_day: network["current_day"].as_u64().unwrap_or(0),
                era: network["era"].as_u64().unwrap_or(0) as u32,
                epoch_seconds: health["epoch_seconds"].as_u64().unwrap_or(60),
                gpu_epoch_cap_eknx: network["gpu_epoch_cap_eknx"]
                    .as_str()
                    .unwrap_or("5")
                    .to_string(),
                cpu_epoch_cap_eknx: network["cpu_epoch_cap_eknx"]
                    .as_str()
                    .unwrap_or("0.8")
                    .to_string(),
                minimum_claim_eknx: network["minimum_claim_eknx"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                pool_fee_percent: network["pool_fee_percent"].as_u64().unwrap_or(0),
                pool_fee_bps: network["pool_fee_bps"].as_u64().unwrap_or(0),
                pool_fee_wallet: network["pool_fee_wallet"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                accepted_gpu_shares: account["accepted_gpu_shares"].as_u64().unwrap_or(0),
                accepted_cpu_shares: account["accepted_cpu_shares"].as_u64().unwrap_or(0),
                rejected_shares: account["rejected_shares"].as_u64().unwrap_or(0),
                cumulative_total_eknx: account["cumulative_total_eknx"]
                    .as_str()
                    .unwrap_or("0")
                    .to_string(),
                cumulative_gpu_eknx: account["cumulative_gpu_eknx"]
                    .as_str()
                    .unwrap_or("0")
                    .to_string(),
                cumulative_cpu_eknx: account["cumulative_cpu_eknx"]
                    .as_str()
                    .unwrap_or("0")
                    .to_string(),
                last_share_at: account["last_share_at"].as_str().unwrap_or("").to_string(),
                public_stats,
                error: String::new(),
            })
        }
        .await;

        match result {
            Ok(pool) => state.status.write().pool = pool,
            Err(e) => {
                let mut s = state.status.write();
                s.pool.online = false;
                s.pool.error = e.to_string();
                s.pool.updated_at = chrono::Utc::now().to_rfc3339();
            }
        }
        sleep(Duration::from_secs(3)).await;
    }
}

async fn api_status(State(state): State<AppState>) -> Json<Value> {
    let s = state.status.read().clone();
    let c = state.config.read().clone();
    Json(json!({
        "name": APP_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "config": c,
        "status": s,
        "token": {
            "name":"Eureka Nexus",
            "symbol":"EKNX",
            "network":NETWORK,
            "chain_id":CHAIN_ID,
            "decimals":TOKEN_DECIMALS,
            "contract":EKNX,
            "genesis_market":GENESIS_MARKET,
            "bscscan_token":format!("https://bscscan.com/token/{EKNX}"),
            "bscscan_contract":format!("https://bscscan.com/address/{EKNX}"),
            "bscscan_market":format!("https://bscscan.com/address/{GENESIS_MARKET}")
        },
        "mining": {
            "gpu_algorithm":"KAWPOW-EUREKA-V1",
            "cpu_algorithm":"RANDOMX-EUREKA-V1",
            "epoch_seconds":60,
            "claim_model":"daily cumulative Merkle entitlement",
            "client_engine_ready":s.engine_ready,
            "cpu_engine_ready":s.cpu_engine_ready,
            "gpu_engine_ready":s.gpu_engine_ready,
            "gpu_engine_path":s.gpu_engine_path,
            "gpu_stratum_addr":s.gpu_stratum_addr,
            "gpu_backend":s.gpu_backend
        },
        "security": {
            "stores_private_keys":false,
            "stores_seed_phrases":false,
            "client_can_move_funds":false,
            "operator_keys_in_client":false
        }
    }))
}

async fn api_config_get(State(state): State<AppState>) -> Json<MinerConfig> {
    Json(state.config.read().clone())
}

async fn api_config_save(
    State(state): State<AppState>,
    Json(mut cfg): Json<MinerConfig>,
) -> Result<Json<Value>, (StatusCode, String)> {
    cfg.wallet = cfg.wallet.trim().to_string();
    cfg.worker_name = cfg.worker_name.trim().to_string();
    cfg.pool_url = cfg.pool_url.trim().trim_end_matches('/').to_string();
    cfg.bsc_rpc = cfg.bsc_rpc.trim().to_string();
    cfg.threads = cfg.threads.clamp(1, num_cpus::get().max(1));
    cfg.mining_mode = normalize_mode(&cfg.mining_mode).to_string();
    cfg.gpu_engine_path = cfg.gpu_engine_path.trim().to_string();
    cfg.gpu_stratum_addr = cfg.gpu_stratum_addr.trim().to_string();
    cfg.gpu_backend = normalize_gpu_backend(&cfg.gpu_backend).to_string();
    if !cfg.wallet.is_empty() && !valid_evm_address(&cfg.wallet) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid BNB Smart Chain wallet address".into(),
        ));
    }
    if cfg.pool_url.is_empty()
        || !(cfg.pool_url.starts_with("http://") || cfg.pool_url.starts_with("https://"))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Pool URL must start with http:// or https://".into(),
        ));
    }
    if cfg.bsc_rpc.is_empty()
        || !(cfg.bsc_rpc.starts_with("http://") || cfg.bsc_rpc.starts_with("https://"))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "BSC RPC URL must start with http:// or https://".into(),
        ));
    }
    if cfg.gpu_stratum_addr.is_empty() || !cfg.gpu_stratum_addr.contains(':') {
        return Err((
            StatusCode::BAD_REQUEST,
            "GPU Stratum address must be host:port".into(),
        ));
    }
    save_config(&cfg).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    *state.config.write() = cfg;
    Ok(Json(json!({"ok":true})))
}

async fn api_start(State(state): State<AppState>) -> Result<Json<Value>, (StatusCode, String)> {
    let cfg = state.config.read().clone();
    let status = state.status.read().clone();
    if status.running {
        return Ok(Json(json!({"ok":true,"already_running":true})));
    }
    if !valid_evm_address(&cfg.wallet) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Configure a valid BSC wallet before mining".into(),
        ));
    }
    if !status.pool.online {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Official mining server is offline".into(),
        ));
    }
    if !status.pool.accept_shares || !status.pool.emission_enabled {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "EKNX mining is still OFF on the Official Mining Server".into(),
        ));
    }

    if status.pool.mining_start_unix <= 0 {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "EKNX mining is not activated on-chain yet".into(),
        ));
    }

    let mode = normalize_mode(&cfg.mining_mode);

    if mode == "GPU" || mode == "BOTH" {
        let thermal_tripped = state.gpu_thermal_tripped.load(Ordering::Relaxed);

        if thermal_tripped {
            let (_, _, gpu_temp, _, _, _) = gpu_snapshot();

            if gpu_restart_blocked(true, gpu_temp) {
                let detail = if gpu_temp > 0.0 {
                    format!("Current GPU temperature is {:.0}°C.", gpu_temp)
                } else {
                    "GPU temperature could not be verified.".to_string()
                };

                return Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    format!(
                        "GPU thermal safety lockout. {detail} Cool the GPU below {:.0}°C before restarting KAWPOW.",
                        GPU_TEMP_RESTART_C
                    ),
                ));
            }

            state.gpu_thermal_tripped.store(false, Ordering::Relaxed);
        }
    }

    if (mode == "GPU" || mode == "BOTH") && !FsPath::new(&cfg.gpu_engine_path).is_file() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "KAWPOW engine not installed at {}. Run INSTALL_KAWPOW_ENGINE_LINUX.sh",
                cfg.gpu_engine_path
            ),
        ));
    }

    state.stop.store(false, Ordering::SeqCst);
    let run_id = state.run_generation.fetch_add(1, Ordering::SeqCst) + 1;
    state.cpu_hashes_total.store(0, Ordering::Relaxed);
    state
        .gpu_hashrate_bits
        .store(0f64.to_bits(), Ordering::Relaxed);

    if mode == "GPU" || mode == "BOTH" {
        if let Err(e) = start_gpu_engine(&state, &cfg) {
            state.stop.store(true, Ordering::SeqCst);
            state.run_generation.fetch_add(1, Ordering::SeqCst);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        }
    }
    if mode == "CPU" || mode == "BOTH" {
        launch_cpu_workers(state.clone(), run_id, cfg.threads);
    }

    {
        let mut s = state.status.write();
        s.running = true;
        s.phase = "MINING · REAL KAWPOW / RANDOMX".into();
        s.message = format!("Mining started in {mode} mode. Shares are validated by the Eureka Nexus Official Mining Server.");
    }
    Ok(Json(json!({"ok":true,"mode":mode,"run_id":run_id})))
}

async fn api_stop(State(state): State<AppState>) -> Json<Value> {
    state.stop.store(true, Ordering::SeqCst);
    state.run_generation.fetch_add(1, Ordering::SeqCst);
    stop_gpu_engine(&state);
    let mut s = state.status.write();
    s.running = false;
    s.cpu_hashrate_hs = 0.0;
    s.gpu_hashrate_hs = 0.0;
    s.phase = "BSC READY · STOPPED".into();
    s.message = "Mining stopped by user.".into();
    Json(json!({"ok":true}))
}

fn update_metadata_for_platform(release: &Value, current: &str, platform: &str) -> Result<Value> {
    let tag = release["tag_name"]
        .as_str()
        .context("Missing release version")?;

    let latest = semver::Version::parse(tag.strip_prefix('v').unwrap_or(tag))?;

    let installed = semver::Version::parse(current)?;

    if release["draft"].as_bool() != Some(false)
        || release["prerelease"].as_bool() != Some(false)
        || !latest.pre.is_empty()
    {
        return Err(anyhow!("Expected a published stable release"));
    }

    let assets = release["assets"]
        .as_array()
        .context("Missing release assets")?;

    let asset = |name: &str| -> Option<Value> {
        let expected =
            format!("https://github.com/Eureka-Nexus/eureka-miner/releases/download/{tag}/{name}");

        assets
            .iter()
            .find(|a| {
                a["name"].as_str() == Some(name)
                    && a["browser_download_url"].as_str() == Some(expected.as_str())
            })
            .map(|_| {
                json!({
                    "name": name,
                    "url": expected
                })
            })
    };

    let (package_name, secondary_name) = match platform {
        "windows-x86_64" => (
            format!("Eureka-Nexus-Miner-Setup-{latest}.exe"),
            Some(format!(
                "Eureka-Nexus-Miner-Official-{latest}-Windows-x86_64.zip"
            )),
        ),

        "linux-x86_64" => (
            format!("Eureka-Nexus-Miner-Official-{latest}-Linux-x86_64.tar.gz"),
            None,
        ),

        other => {
            return Err(anyhow!("Unsupported update platform: {other}"));
        }
    };

    let package_asset = asset(&package_name);

    let checksum_asset = asset(&format!("{package_name}.sha256.txt"))
        .or_else(|| asset(&format!("{package_name}.sha256")));

    let secondary_asset = secondary_name.as_ref().and_then(|name| asset(name));

    let secondary_checksum_asset = secondary_name.as_ref().and_then(|name| {
        asset(&format!("{name}.sha256.txt")).or_else(|| asset(&format!("{name}.sha256")))
    });

    Ok(json!({
        "current": current,
        "latest": latest.to_string(),
        "update_available":
            latest.cmp_precedence(&installed).is_gt(),

        "platform": platform,

        "package_asset": package_asset,
        "checksum_asset": checksum_asset,

        "secondary_asset": secondary_asset,
        "secondary_checksum_asset":
            secondary_checksum_asset
    }))
}

fn current_update_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows-x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux-x86_64"
    } else {
        "unsupported"
    }
}

async fn api_update_check() -> Json<Value> {
    let current = env!("CARGO_PKG_VERSION");
    let result: Result<Value> = async {
        let client = reqwest::Client::builder()
            .user_agent(format!("EurekaNexusMiner/{current}"))
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let response = client
            .get("https://api.github.com/repos/Eureka-Nexus/eureka-miner/releases/latest")
            .header(header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!("GitHub returned HTTP {}", response.status()));
        }
        let release: Value = response.json().await?;
        update_metadata_for_platform(&release, current, current_update_platform())
    }
    .await;
    Json(match result {
        Ok(metadata) => metadata,
        Err(error) => json!({
            "current": current, "latest": null, "update_available": null,
            "platform": current_update_platform(), "package_asset": null, "checksum_asset": null, "secondary_asset": null, "secondary_checksum_asset": null,
            "error": format!("Could not check for updates: {error}")
        }),
    })
}

async fn api_claim_info(
    State(state): State<AppState>,
    Path(day): Path<u64>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let cfg = state.config.read().clone();
    if !valid_evm_address(&cfg.wallet) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Configure a valid BSC wallet first".into(),
        ));
    }
    let url = format!(
        "{}/v1/claim/{}/{}",
        cfg.pool_url.trim_end_matches('/'),
        cfg.wallet,
        day
    );
    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    let code = response.status();
    let raw = response.text().await.unwrap_or_default();
    if !code.is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            if raw.is_empty() {
                format!("Pool returned {code}")
            } else {
                raw
            },
        ));
    }
    let value: Value = serde_json::from_str(&raw)
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Invalid claim JSON: {e}")))?;
    Ok(Json(value))
}

async fn api_relay_claim(
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let cfg = state.config.read().clone();

    if !valid_evm_address(&cfg.wallet) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Configure a valid BSC wallet first".into(),
        ));
    }

    let url = format!("{}/v1/relay-claim", cfg.pool_url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response = client
        .post(url)
        .json(&json!({
            "wallet": cfg.wallet
        }))
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("Could not reach Eureka claim relayer: {e}"),
            )
        })?;

    let code = response.status();
    let raw = response.text().await.unwrap_or_default();

    if !code.is_success() {
        let upstream_status =
            StatusCode::from_u16(code.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

        let message = serde_json::from_str::<Value>(&raw)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .and_then(Value::as_str)
                    .or_else(|| v.get("message").and_then(Value::as_str))
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| {
                if raw.is_empty() {
                    format!("Eureka claim relayer returned HTTP {}", code.as_u16())
                } else {
                    raw.clone()
                }
            });

        return Err((upstream_status, message));
    }

    let value: Value = serde_json::from_str(&raw).map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Invalid claim relayer JSON: {e}"),
        )
    })?;

    Ok(Json(value))
}

async fn index_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match WebAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not found"))
            .unwrap(),
    }
}

fn dashboard_request_allowed(headers: &HeaderMap, port: u16) -> bool {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
        return false;
    }
    if let Some(origin) = headers.get(header::ORIGIN) {
        if origin.to_str().ok() != Some(format!("http://{host}").as_str()) {
            return false;
        }
    }
    !matches!(
        headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()),
        Some("cross-site")
    )
}

async fn protect_dashboard(
    State(port): State<u16>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !dashboard_request_allowed(request.headers(), port) {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::from(
                "Dashboard requests must come from its local origin",
            ))
            .unwrap();
    }
    next.run(request).await
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("Eureka Nexus Miner {}\nUsage: eureka-nexus-miner-official [--help | --version]\nRun without arguments to open the local dashboard. Mining starts only from the dashboard.\nLinux config: $XDG_CONFIG_HOME/EurekaNexus/MinerOfficial1/miner-config.json (default ~/.config).\nWindows config: %APPDATA%\\EurekaNexus\\MinerOfficial1\\miner-config.json.", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.len() == 1 && (args[0] == "--version" || args[0] == "-V") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if !args.is_empty() {
        return Err(anyhow!("Unknown arguments; use --help"));
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "eureka_nexus_miner_official=info".into()),
        )
        .init();

    let cfg = load_config();
    let port = cfg.dashboard_port;
    let state = AppState {
        config: Arc::new(RwLock::new(cfg.clone())),
        status: Arc::new(RwLock::new(MinerStatus::default())),
        stop: Arc::new(AtomicBool::new(true)),
        run_generation: Arc::new(AtomicU64::new(0)),
        cpu_hashes_total: Arc::new(AtomicU64::new(0)),
        gpu_hashrate_bits: Arc::new(AtomicU64::new(0f64.to_bits())),
        gpu_child: Arc::new(Mutex::new(None)),
        gpu_thermal_tripped: Arc::new(AtomicBool::new(false)),
    };

    let app = Router::new()
        .route("/api/status", get(api_status))
        .route("/api/update/check", get(api_update_check))
        .route("/api/config", get(api_config_get).post(api_config_save))
        .route("/api/start", post(api_start))
        .route("/api/stop", post(api_stop))
        .route("/api/claim/:day", get(api_claim_info))
        .route("/api/relay-claim", post(api_relay_claim))
        .fallback(get(index_handler))
        .layer(middleware::from_fn_with_state(port, protect_dashboard))
        .with_state(state.clone());

    tokio::spawn(telemetry_loop(state.clone()));
    tokio::spawn(chain_loop(state.clone()));
    tokio::spawn(pool_loop(state.clone()));

    if cfg.auto_start {
        warn!("auto_start is configured; mining still requires the Official Server gate to be open and a valid wallet");
    }

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("{} dashboard: http://{}", APP_NAME, addr);
    let desktop_mode = std::env::var("EUREKA_DESKTOP_MODE")
        .map(|v| v == "1")
        .unwrap_or(false);

    if cfg.open_dashboard && !desktop_mode {
        let url = format!("http://127.0.0.1:{port}");
        tokio::spawn(async move {
            sleep(Duration::from_millis(700)).await;
            let _ = webbrowser::open(&url);
        });
    }

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("cannot bind dashboard to {addr}"))?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod update_tests {
    use super::*;

    fn release(tag: &str) -> Value {
        json!({"tag_name": tag, "draft": false, "prerelease": false, "assets": []})
    }

    #[test]
    fn semantic_version_precedence() {
        for (current, latest, expected) in [
            ("1.1.1", "v1.1.0", false),
            ("1.1.1", "v1.1.1", false),
            ("1.9.0", "v1.10.0", true),
            ("1.1.1-rc.1", "v1.1.1", true),
            ("1.1.1+local", "v1.1.1+release", false),
        ] {
            assert_eq!(
                update_metadata_for_platform(&release(latest), current, "windows-x86_64").unwrap()
                    ["update_available"],
                expected
            );
        }
        for invalid in ["v1.2", "garbage", "v1.2.0-rc.1"] {
            assert!(
                update_metadata_for_platform(&release(invalid), "1.1.1", "windows-x86_64").is_err()
            );
        }
        let mut draft = release("v1.2.0");
        draft["draft"] = json!(true);
        assert!(update_metadata_for_platform(&draft, "1.1.1", "windows-x86_64").is_err());
    }

    #[test]
    fn assets_must_match_official_release() {
        let mut data = release("v1.2.0");

        let setup = "Eureka-Nexus-Miner-Setup-1.2.0.exe";

        let setup_sha = format!("{setup}.sha256.txt");

        let zip = "Eureka-Nexus-Miner-Official-1.2.0-Windows-x86_64.zip";

        let zip_sha = format!("{zip}.sha256.txt");

        let linux = "Eureka-Nexus-Miner-Official-1.2.0-Linux-x86_64.tar.gz";

        let linux_sha = format!("{linux}.sha256.txt");

        let url = |name: &str| {
            format!("https://github.com/Eureka-Nexus/eureka-miner/releases/download/v1.2.0/{name}")
        };

        data["assets"] = json!([
            {
                "name": setup,
                "browser_download_url": url(setup)
            },
            {
                "name": setup_sha,
                "browser_download_url": url(&setup_sha)
            },
            {
                "name": zip,
                "browser_download_url": url(zip)
            },
            {
                "name": zip_sha,
                "browser_download_url": url(&zip_sha)
            },
            {
                "name": linux,
                "browser_download_url": url(linux)
            },
            {
                "name": linux_sha,
                "browser_download_url": url(&linux_sha)
            }
        ]);

        let windows = update_metadata_for_platform(&data, "1.1.1", "windows-x86_64").unwrap();

        assert_eq!(windows["package_asset"]["name"], setup);

        assert_eq!(windows["checksum_asset"]["name"], setup_sha);

        assert_eq!(windows["secondary_asset"]["name"], zip);

        let linux_result = update_metadata_for_platform(&data, "1.1.1", "linux-x86_64").unwrap();

        assert_eq!(linux_result["package_asset"]["name"], linux);

        assert_eq!(linux_result["checksum_asset"]["name"], linux_sha);

        data["assets"][0]["browser_download_url"] = json!("https://untrusted.example/setup.exe");

        let windows = update_metadata_for_platform(&data, "1.1.1", "windows-x86_64").unwrap();

        assert!(windows["package_asset"].is_null());

        assert!(
            update_metadata_for_platform(&release("v1.2.0"), "1.1.1", "linux-x86_64").unwrap()
                ["package_asset"]
                .is_null()
        );
    }

    #[tokio::test]
    #[ignore = "Requires live GitHub access"]
    async fn official_release_check() {
        let Json(result) = api_update_check().await;
        assert!(result.get("error").is_none(), "{result}");
        assert_eq!(result["current"], env!("CARGO_PKG_VERSION"));
        assert!(result["latest"].is_string());
        assert!(result["update_available"].is_boolean());
        println!("{result}");
    }
}

#[cfg(test)]
mod thermal_safety_tests {
    use super::*;

    #[test]
    fn thermal_thresholds_are_correct() {
        assert!(!gpu_temp_warning(79.9));
        assert!(gpu_temp_warning(80.0));
        assert!(gpu_temp_warning(84.9));

        assert!(!gpu_temp_cutoff(84.9));
        assert!(gpu_temp_cutoff(85.0));
        assert!(gpu_temp_cutoff(95.0));

        assert!(!gpu_temp_cooled(0.0));
        assert!(gpu_temp_cooled(74.9));
        assert!(!gpu_temp_cooled(75.0));

        assert!(gpu_restart_blocked(true, 85.0));
        assert!(gpu_restart_blocked(true, 75.0));
        assert!(gpu_restart_blocked(true, 0.0));
        assert!(!gpu_restart_blocked(true, 74.9));
        assert!(!gpu_restart_blocked(false, 90.0));
    }
}

#[cfg(test)]
mod dashboard_origin_tests {
    use super::*;
    #[test]
    fn rejects_remote_origins_and_rebinding_hosts() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:8077".parse().unwrap());
        assert!(dashboard_request_allowed(&headers, 8077));
        headers.insert(header::ORIGIN, "https://untrusted.example".parse().unwrap());
        assert!(!dashboard_request_allowed(&headers, 8077));
        headers.insert(header::ORIGIN, "null".parse().unwrap());
        assert!(!dashboard_request_allowed(&headers, 8077));
        headers.insert(header::ORIGIN, "http://127.0.0.1:8077".parse().unwrap());
        assert!(dashboard_request_allowed(&headers, 8077));
        headers.remove(header::ORIGIN);
        headers.insert(header::HOST, "untrusted.example:8077".parse().unwrap());
        assert!(!dashboard_request_allowed(&headers, 8077));
        headers.insert(header::HOST, "localhost:8077".parse().unwrap());
        headers.insert("sec-fetch-site", "cross-site".parse().unwrap());
        assert!(!dashboard_request_allowed(&headers, 8077));
    }
}
