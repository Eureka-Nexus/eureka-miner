# Security Policy

## Wallet and keys

Eureka Nexus Miner only requires a public BNB Smart Chain wallet address.

The Miner must never request, transmit or store:
- private keys
- seed phrases
- mnemonic phrases
- treasury keys
- operator signing keys

Claim transactions are signed by the user's own browser wallet after explicit approval.

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
