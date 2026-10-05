# Eureka Nexus Miner Official 1.1.2

Official CPU/GPU miner for the Eureka Nexus (EKNX) ecosystem on BNB Smart Chain Mainnet.

## Official network

- Network: BNB Smart Chain Mainnet
- Chain ID: 56
- Native currency: BNB
- Token: Eureka Nexus (EKNX)
- Decimals: 18
- EKNX contract: `0xF54913A8d5E2AEBD0B62c6411cCf1b5B4aB069c9`
- Genesis Market: `0x837dBE1D3b67e8315127c36a119E163e713ee73D`
- Official pool: `https://pool.eurekanexus.pt`
- GPU Stratum: `pool.eurekanexus.pt:3333`

## Download and install

Download Eureka Nexus Miner 1.1.2 from [GitHub Releases](https://github.com/Eureka-Nexus/eureka-miner/releases). Windows and Linux x86-64 use the same Miner version. Always download the matching SHA-256 checksum together with the package and verify it before running the Miner.

Linux x86-64:

Download:

`Eureka-Nexus-Miner-Official-1.1.2-Linux-x86_64.tar.gz`

and:

`Eureka-Nexus-Miner-Official-1.1.2-Linux-x86_64.tar.gz.sha256.txt`

Then verify and run:

```bash
sha256sum -c Eureka-Nexus-Miner-Official-1.1.2-Linux-x86_64.tar.gz.sha256.txt
tar -xzf Eureka-Nexus-Miner-Official-1.1.2-Linux-x86_64.tar.gz
cd Eureka-Nexus-Miner-Official-1.1.2-Linux-x86_64
sha256sum -c SHA256SUMS.txt
./eureka-nexus-miner-official --version
./eureka-nexus-miner-official
```

Windows x86-64: download and run `Eureka-Nexus-Miner-Setup-1.1.2.exe`. The installer creates the Eureka Nexus Miner application and shortcuts automatically.

Open `http://127.0.0.1:8077`, configure your public BSC wallet and select CPU, GPU or BOTH. No mining begins merely by opening the dashboard. The on-chain mining activation gate must be open before mining can start.

### GPU engine

The Windows 1.1.2 installer automatically downloads the official RavenCommunity kawpowminer 1.2.4 runtime and verifies its published SHA-256. Internet access is required during installation. Download or verification failures are reported by Setup; rerun Setup after correcting the connection. Normal users do not need to run PowerShell or install KAWPOW manually. kawpowminer remains a separate third-party GPL-3.0 component. Compatible GPU drivers are still required.

CPU mode uses the integrated RandomX engine and does not require that GPU download.

### Configuration and troubleshooting

Linux configuration: `${XDG_CONFIG_HOME:-$HOME/.config}/EurekaNexus/MinerOfficial1/miner-config.json`.
Windows configuration: `%APPDATA%\EurekaNexus\MinerOfficial1\miner-config.json`.

If port 8077 is occupied, identify the existing process before stopping it. For an isolated Linux test, set `XDG_CONFIG_HOME` to a temporary directory containing `EurekaNexus/MinerOfficial1/miner-config.json` with `dashboard_port` set to another loopback port, `open_dashboard` false and `auto_start` false. Do not change another instance's configuration.

Version 1.1.2 supports `--help` and `--version` without starting the dashboard. Configuration `auto_start` does not bypass the official mining gate or automatically begin mining.

## Mining

CPU algorithm:

`RANDOMX-EUREKA-V1`

GPU algorithm:

`KAWPOW-EUREKA-V1`

RandomX CPU mining is integrated in the Miner.
GPU mining uses RavenCommunity kawpowminer as a separate external process.

Only work validated by the Official Mining Server earns EKNX.
Client-reported hashrate alone does not create rewards.

## Pool fee and rewards

The Official Mining Server applies a transparent 1% pool fee.

- Miner allocation: 99%
- Pool fee: 1%
- Fee: 100 basis points
- Fee wallet: `0x42f58c8a09bce3a00faf553aac60b0daf320858b`

Rewards are finalized at epoch close and added to the miner's cumulative Merkle entitlement.

The official minimum claim policy is currently **50 EKNX**. The Miner reads the active minimum live from the Official Mining Server rather than hardcoding it, so the server remains authoritative.

Mining cannot start before on-chain EKNX mining activation.

## GPU thermal safety

- Below 80 C: normal operation
- 80 C to 84.9 C: warning
- 85 C or above: KAWPOW automatically stops

After a thermal cutoff, the mining session remains active. KAWPOW automatically restarts only after the GPU cools below 75 C. In BOTH mode, RandomX CPU mining can continue while the GPU is cooling.

## Wallet security

The Miner only stores a public BSC wallet address.

It never stores private keys, seed phrases, operator keys or treasury keys.

The Miner does not sign claim transactions with the mining wallet. After the user confirms the claim in the Miner, the official Eureka Nexus Relayer submits the BSC transaction and pays the gas. The Miner only uses the configured public BSC receiving address.

## Dashboard

Local dashboard:

`http://127.0.0.1:8077`

The interface can request a compatible browser wallet to add:

- BNB Smart Chain Mainnet
- Official EKNX token

## Build

Linux:

`./BUILD_LINUX_RELEASE.sh`

Windows:

`.\BUILD_WINDOWS_RELEASE.ps1`

## Licensing

Original Eureka Nexus Miner source code is licensed under the MIT License.

RandomX and randomx-rs use the BSD 3-Clause License.

RavenCommunity kawpowminer uses GPL-3.0 and remains a separate third-party executable.

See LICENSE, LICENSES and THIRD_PARTY_NOTICES.md.

## Release verification

See [CHANGELOG.md](CHANGELOG.md) for version changes and the [network launch checklist](https://github.com/Eureka-Nexus/eureka-docs/blob/main/LAUNCH_CHECKLIST.md) for operational gates. Published packages have SHA-256 checksums; they are not represented as code-signed binaries.
