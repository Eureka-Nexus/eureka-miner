# Changelog

## 1.1.2

- Stop GPU and CPU mining automatically if the Windows desktop backend exits unexpectedly.
- Terminate the Windows mining Job Object on backend failure so KAWPOW/RandomX cannot remain orphaned.
- Improve KAWPOW telemetry parsing so carriage-return status lines report real GPU hashrate reliably.
- Verify STOP completion before reporting mining as stopped.
- Enforce GPU thermal warning at 80 C, automatic KAWPOW cutoff at 85 C and automatic recovery below 75 C.
- Clear stale Merkle proofs and re-fetch the latest settlement before every claim attempt.
- Keep claim availability at zero when no valid published settlement exists.
- Use the live Official Mining Server minimum claim policy, currently 50 EKNX.
- Add platform-aware update metadata for Windows x86-64 and Linux x86-64 release packages.
- Align Windows desktop, backend, installer, Linux package and release metadata to version 1.1.2.

## 1.1.1

- Route EKNX claims through the official Eureka Nexus Relayer so miners do not need BNB for claim gas.
- Keep private keys and seed phrases out of the Miner; claims use only the configured public BSC wallet.
- Read the minimum claim policy live from the Official Mining Server instead of using a hardcoded value.
- Remove the obsolete browser-side direct `claimMining` transaction builder and gas submission flow.
- Preserve optional browser-wallet helpers for setting the receiving address, adding BNB Smart Chain and adding EKNX.
- Keep Windows launcher process cleanup through a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
- Align Windows desktop, backend, installer and release metadata to version 1.1.1.

## 1.1.0

- Windows desktop application with embedded icon and version metadata.
- Closing the desktop window stops the backend and its mining processes.
- Setup installs official KAWPOW automatically with SHA-256 verification and reports installation failures.
- Windows release script builds the desktop/backend, ZIP, Setup and checksum files in one invocation.

## 1.0.1

- Make `--help` and `--version` exit before dashboard/network startup; reject unknown arguments.
- Restrict dashboard requests to the expected local Host and browser origin; remove permissive CORS.
- Add regression tests for foreign origins, opaque origins and rebinding hosts.
- Add installation, isolated-test and prelaunch documentation.

## 1.0.0

Initial Windows/Linux BSC miner release with integrated RandomX and external KAWPOW support.
