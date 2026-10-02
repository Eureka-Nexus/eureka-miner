# Eureka Nexus Miner Official 1.0

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

Minimum claim policy: 100 EKNX.

Mining cannot start before on-chain EKNX mining activation.

## GPU thermal safety

- Below 80 C: normal operation
- 80 C to 84.9 C: warning
- 85 C or above: KAWPOW automatically stops

After a thermal cutoff, GPU mining remains locked until the GPU cools below 80 C and the user starts mining again manually.

In BOTH mode, CPU RandomX mining can remain active after a GPU thermal cutoff.

## Wallet security

The Miner only stores a public BSC wallet address.

It never stores private keys, seed phrases, operator keys or treasury keys.

Claim transactions are confirmed and signed inside the user's own browser wallet.

## Dashboard

Local dashboard:

`http://127.0.0.1:8077`

The interface can request a compatible browser wallet to add:

- BNB Smart Chain Mainnet
- Official EKNX token

## Build

Linux:

`cargo build --release --bin eureka-nexus-miner-official`

Windows:

`.\BUILD_WINDOWS_RELEASE.ps1`

## Licensing

Original Eureka Nexus Miner source code is licensed under the MIT License.

RandomX and randomx-rs use the BSD 3-Clause License.

RavenCommunity kawpowminer uses GPL-3.0 and remains a separate third-party executable.

See LICENSE, LICENSES and THIRD_PARTY_NOTICES.md.
