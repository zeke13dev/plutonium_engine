# Security Policy

## Supported versions

plutonium_engine is pre-1.0. Security fixes are prioritized for the latest published release and the current `main` branch.

| Version | Supported |
| ------- | --------- |
| latest  | Yes       |
| older pre-1.0 releases | Best effort |

## Reporting a vulnerability

Please do not report security vulnerabilities through public GitHub issues.

Use one of these private channels instead:

- GitHub Security Advisory: https://github.com/zeke13dev/plutonium_engine/security/advisories/new
- Email: `coding@zeke13.com`

Include as much of the following as possible:

- affected version or commit SHA
- target platform and feature flags
- minimal reproduction steps or proof of concept
- impact assessment, including whether the issue affects native, wasm, asset loading, text/font handling, input, or rendering paths
- any known mitigations

## Response expectations

Maintainers will acknowledge reports as soon as practical, investigate privately, and coordinate a fix and disclosure timeline based on severity. Public disclosure should wait until a fix or mitigation is available unless there is active exploitation or another safety reason to disclose sooner.

## Scope

Security-sensitive areas include:

- unsafe or untrusted asset parsing paths
- texture, font, SVG, and raster loading
- wasm/browser integration
- filesystem, clipboard, and input handling
- dependencies with known vulnerabilities

General correctness bugs, crashes with trusted input, and performance regressions can be reported through normal issues unless they create a security impact.
