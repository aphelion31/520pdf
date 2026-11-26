# REA Shipped-Artifact Inspection & Release Audit (`520pdf` v0.2.0)

Recorded using REA MCP tools against the compiled `520pdf` release binary, built React Router 8 SPA (`web/build/client`), and upstream reference bundle (`../iHatePDF/dist/index.js`).

## 1. Release Binary Hardening (`target/release/pdf520`)

- **Evidence ID**: `ev_fb8b12efcff297f572e617e9aebbc844399b6040a3552d33c276ef62d5891444`
- **Format / Arch**: Mach-O `arm64` (64-bit, `8,275,792` bytes)
- **SHA-256**: `4b196ad3179e5440c7fc07c8850f81a733c1a5df0f17f10fd4157fada9a024ef`

| Property | State | Basis |
| --- | --- | --- |
| `pie` (ASLR) | `enabled` | `MH_PIE` flag set |
| `allow_stack_execution` | `disabled` | `MH_ALLOW_STACK_EXECUTION` clear |
| `no_heap_execution` | `disabled` | `MH_NO_HEAP_EXECUTION` clear |
| `stack_protector` | `enabled` | Undefined symbol `___stack_chk_fail` present |
| `code_signature_present` | `enabled` | `LC_CODE_SIGNATURE` load command present |
| `encrypted` | `disabled` | No `LC_ENCRYPTION_INFO` command |

## 2. Release Exposure Audits

### Rust Binary (`target/release/pdf520`)
- **Evidence ID**: `ev_8edceee54cd870ac8f2af5a89598933ae1c133cfc917e9ec73b22863c52fffb0`
- **Result**: `exposed: 0`, `no_exposure_observed_in_assessed_checks: true` (`not_observed: 4`, `not_applicable: 6`, `unknown: 2`, `not_assessed: 3`) — zero `.dSYM`, `.pdb`, debug companions, or embedded JS source maps.

### React Router 8 Client Bundle (`web/build/client`)
- **Evidence ID**: `ev_f9bf5ee7451c3f0e15e2b3fb62df1595bf4ed438809278b2eb8a77157f39af00`
- **SHA-256**: `90e4ac671eb9cd832c6f980b1b38eaec572d1bec7a5c5398a027e39c00498d9c`
- **Result**: `exposed: 0`, `no_exposure_observed_in_assessed_checks: true` — zero credential markers or source maps exposed in the built client assets.

### Embedded Crate Metadata Probe (`inspect_rust_binary`)
- **Evidence IDs**: `ev_fd057295303526d11c3822ce3356c53a594cb0ce64d27d4eeb2226deb3f7712a` / view `ev_f812954b7bd26a02df902c7b46d1b209c26a29050ef115a965987846ea6a6765`
- **Observation**: No `.dep-v0` (`cargo-auditable`) metadata embedded in stripped release binary.

## 3. Upstream Reference Bundle Analysis (`../iHatePDF/dist/index.js`)

- **Evidence ID**: `ev_884284cb242d5f6ba345dafbac88e3beb26b3b93ed04355f3fd94aad6df9ffa7`
- **SHA-256**: `4b7a8c462e2aa0856c7137a4bfb795e7a852667879a9e3aa7a58af9f3ddb02ca`
- **Observation**: Express server bundle (`4,961` bytes) parsed cleanly (`786` AST nodes, `34` findings), confirming static client hosting + single `GET /api/status` backend JSON contract.
