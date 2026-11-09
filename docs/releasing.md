# Releasing Umbra

A release is a tag. Pushing a tag such as `v0.4.1` runs
[.github/workflows/release.yml](../.github/workflows/release.yml), which tests on Linux, macOS and
Windows, builds six archives, and publishes a GitHub release with them and `SHA256SUMS`.

| Archive | Platform |
|---|---|
| `umbra-aarch64-apple-darwin.tar.gz` | macOS on Apple silicon |
| `umbra-x86_64-apple-darwin.tar.gz` | macOS on Intel |
| `umbra-x86_64-unknown-linux-musl.tar.gz` | Linux on x86_64, a static binary |
| `umbra-aarch64-unknown-linux-musl.tar.gz` | Linux on arm64, a static binary |
| `umbra-x86_64-pc-windows-msvc.zip` | Windows on x86_64 |
| `umbra-aarch64-pc-windows-msvc.zip` | Windows on arm64 |

Each archive holds the program, `README.md` and `LICENSE`. The names carry no version, so
`releases/latest/download/<name>` always gives the newest one.

## Cutting a release

From a clean checkout of `main`:

```powershell
pwsh packaging/release.ps1 -Part patch     # or minor, major
pwsh packaging/release.ps1 -NoBump         # the version Cargo.toml already has
pwsh packaging/release.ps1 -WhatIf         # say what would happen
pwsh packaging/release.ps1 -AfterTag       # the tag is pushed already: wait for its release and carry on
```

When the release workflow fails, fix the cause on `main`, build the existing tag again with
`gh workflow run release.yml --ref main -f tag=v<version>`, and run `release.ps1 -AfterTag`.

[packaging/release.ps1](../packaging/release.ps1) raises the version in `Cargo.toml` and
`Cargo.lock`, commits, tags and pushes, waits for the release, then:

1. writes `Formula/umbra.rb` in the tap
   [Black-Rainbow-Labs/homebrew-umbra](https://github.com/Black-Rainbow-Labs/homebrew-umbra) from
   [packaging/homebrew/umbra.rb.template](../packaging/homebrew/umbra.rb.template) and the
   release's `SHA256SUMS`, and pushes it. The tap's own workflow then runs `brew install` and
   `brew test` on macOS and Linux;
2. writes [packaging/acp-registry/umbra/agent.json](../packaging/acp-registry/umbra/agent.json) for
   the new version and pushes it;
3. installs the release on the machine it ran on.

## The ACP registry

The [ACP registry](https://github.com/agentclientprotocol/registry) lists Umbra from
`umbra/agent.json` and `umbra/icon.svg` in that repository, copied from
[packaging/acp-registry/umbra](../packaging/acp-registry/umbra). Once listed, the registry raises
the version by itself from this repository's GitHub releases every hour, so a release needs no
change there. The registry's own checks can be run from a checkout of it:

```sh
uv run --with jsonschema .github/workflows/build_registry.py
python3 .github/workflows/verify_agents.py --auth-check --agent umbra
```
