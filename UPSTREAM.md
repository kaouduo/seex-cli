# Attribution and local changes

SeEx by Ref42 — https://github.com/ref42/seex

Original copyright and LICENSE are retained. This is a local adaptation of the
master source archive downloaded on 2026-10-15 (package version 1.1.2), not an
upstream release. The exact archive SHA-256 is recorded in upstream-archive.sha256.

Changes: public library entry point; separate console executable seex-cli;
dispatch for the existing CLI definitions; read-only EasyEDA Pro project scan;
direct online model-UUID STEP retrieval; JSON results and process exit codes;
Windows build instructions. The original GUI entry point remains intact.
GUI dependencies and icon generation are now enabled only with `--features gui`;
the default build is the console tool. Cargo.lock includes the ZIP reader dependency.
HTTP uses native TLS (Windows SChannel on Windows), retaining certificate validation,
instead of the upstream default AWS-LC Rustls provider.

No commercial-use permission or change to the original license is implied.
The source tree and original LICENSE accompany this local build.
