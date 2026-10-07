# Security Policy

## Wallet and keys

Eureka Nexus Miner only requires a public BNB Smart Chain wallet address.

The Miner must never request, transmit or store:
- private keys
- seed phrases
- mnemonic phrases
- treasury keys
- operator signing keys

The Miner never signs, requests or broadcasts EKNX payout transactions.

Automatic EKNX payouts are executed by Eureka Nexus infrastructure and sent directly to the configured public BSC receiving address. Eureka Nexus pays the BSC transaction gas.

## Automatic payout security

The Miner exposes payout information as read-only status.

There is no manual claim action and no local payout relay execution endpoint.

The current production minimum automatic payout is **5 EKNX**.

Eligibility is checked automatically **every 4 hours**.

The Official Mining Server remains authoritative for payout state and policy.

Never provide a private key, seed phrase or BNB payment to receive an EKNX mining payout.

## Local dashboard

The dashboard binds to:

127.0.0.1:8077

It is not intended to be exposed directly to the public Internet.

## Official network

Official pool:
https://pool.eurekanexus.pt

BNB Smart Chain Mainnet:
Chain ID 56

Official EKNX contract:
0xF54913A8d5E2AEBD0B62c6411cCf1b5B4aB069c9

## Vulnerability reports

Do not publish private keys, secrets, signing material or exploitable payout
vulnerabilities in public GitHub issues.

Use GitHub private security reporting or Security Advisories when available.

## Mining software

Third-party mining engines may trigger antivirus or endpoint-security alerts.

Do not disable security software blindly. Verify downloads and SHA-256 hashes.
