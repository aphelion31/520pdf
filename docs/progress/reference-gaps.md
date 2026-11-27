# iHatePDF reference gaps

Compared repository main `29eb82110f22f97e6a22253412c640ee9f1813a5` README, pdfUtils.ts and mathUtils.ts on 2026-11-20. Implementations are native additions to existing Axum app, not a React migration or copied source.

Added: PDF to PNG/JPEG ZIP, image target-size compression, equation PNG via safe Matplotlib mathtext, resume JSON to PDF, bounded in-memory cross-tool file reuse with explicit clear. Existing merge/split/sign/watermark/encryption/preview and compression target UI retained.

Differences: resume builder currently takes JSON rather than a visual form. No full LaTeX resume compiler or equation editor/palette. Full TeX execution was not imported because untrusted TeX introduces file/process access risk, mathtext explicitly supports a subset. No claim of full iHatePDF parity. No client-side privacy claim: files upload to Axum only on Process. File vault clears on refresh, explicit clear, or removal, bounded to 30 files/49 MiB.

No third-party code copied. iHatePDF reference is MIT licensed. Existing 520pdf brand retained. No rasterization added to default PDF compression.
