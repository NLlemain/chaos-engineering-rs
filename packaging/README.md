# Package Manager Publishing

The release workflow renders the Homebrew formula and WinGet manifest templates with the SHA-256 values of the attested `v0.2.0` binaries. Rendered files are attached to the GitHub release.

- Homebrew: publish `chaos-engineering-rs.rb` in the `Ninian-Lemain/homebrew-chaos-engineering` tap.
- WinGet: validate `NinianLemain.ChaosEngineeringRs.yaml` with `winget validate`, then submit it to `microsoft/winget-pkgs`.
- crates.io: add the repository secret `CARGO_REGISTRY_TOKEN`, then manually run the `Publish crates.io` workflow with `execute` enabled.
- Docker: tags on `ghcr.io/ninian-lemain/chaos-engineering-rs` are built automatically from `main` and release tags.

Templates deliberately contain placeholders and are not themselves valid submissions. This prevents a package-manager entry from claiming a checksum before the release artifact exists.
