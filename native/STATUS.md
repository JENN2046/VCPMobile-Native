# Current implementation status

Repository: `JENN2046/VCPMobile-Native`; branch: `codex/native-rust-m0-m1`; tracker: Issue #1. Read latest Actions and Issue comments before treating this snapshot as current.

## Verified baseline

`6c6b4c4700e2f06358431dd0fe7f90cad6835619` / run `40290030154`: Android and iOS jobs SUCCESS. iOS 9/9 tests passed, no skips or expected failures; this includes all six formerly failing real Keychain tests and the original three UI tests. Scope: cloud build and simulator, not devices or actual VCP service. The prior failed run40287260542 remains retained.

## This development batch, not yet its own PASS

Source-backed one-use HTTPS pairing, explicit no-retry clients, credential persistence before session GET, late-response fencing, native connection/recovery/removal screens. Eight new Rust tests (including five synthetic real-TLS scenarios) and two connection UI tests per platform; existing UI/vault assertions preserved. Details: `docs/M2_PAIRING.md`. Cargo manifests and lock unchanged.

M1 baseline simulator criterion passed. M2 implementation continues; actual isolated NativeSync pairing/history/submission/ACK-loss verification remains incomplete. M3–M5, physical phones and distribution signing remain incomplete. No production, main, old repository, real history or credentials modified.

## History and next step

All earlier status entries are preserved verbatim in `docs/history/STATUS_THROUGH_6c6b4c4.md`; their earlier “no source/no vault/no lock” statements are historical, not current. Existing source materials, vaults and locked dependencies are available.

Next: inspect this new batch's actual CI, repair only real failures, then wire authenticated Topic history and pending reconciliation. Do not repeat active CI or overwrite concurrent HEAD; continue on the same development branch.
