# Changelog

## 1.0.1

- Make `--help` and `--version` exit before dashboard/network startup; reject unknown arguments.
- Restrict dashboard requests to the expected local Host and browser origin; remove permissive CORS.
- Add regression tests for foreign origins, opaque origins and rebinding hosts.
- Add installation, isolated-test and prelaunch documentation.

## 1.0.0

Initial Windows/Linux BSC miner release with integrated RandomX and external KAWPOW support.
