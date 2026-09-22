# OCR speed and conversion progress, 2026-09-07

## Changes

- Medium Paddle recognition now feeds one cropped line per inference. On this Apple M4, batching six lines was substantially slower. The model, dictionary, detection resolution and layout predictor are unchanged. The CPU thread pool defaults to at most four available threads; `CASY_OCR_THREADS=1..8` is available for diagnostics.
- Disabled unused word-box calculation; detected line bounding boxes, confidence, reading order and source maps remain.
- Standalone Markdown conversion skips searchable PDF generation. Document-library processing and revision still produce and validate searchable PDFs.
- Standalone images are decoded directly, bounded to a 2400-pixel longest edge, with transparency composited onto white. Multi-frame and image-allocation protections remain.
- Conversion events carry job identity, phase, completed/total pages, elapsed time and estimated remaining time. The dialog subscribes before invoking conversion, ignores other jobs, cleans up listeners, and distinguishes finalizing from saved output. ETA uses observed page throughput.
- Engine artifacts include per-page render/OCR/layout timings. Standalone temporary artifacts are removed after saving Markdown.

## Measurements

Local copies of pages from the supplied 62-page PDF were used without changing the original. Each sample includes process/model startup. Machine load varies; these are sample measurements, not a full-document SLA.

| Sample | Previous engine | Optimized engine |
| --- | ---: | ---: |
| First page as PDF | 8.36 s | 5.55 s |
| Dense body page as PDF | 11.94 s | 3.99-5.30 s |
| Same first page as PNG | Not measured | 2.93 s |

The dense page's OCR stage fell from 7.52 s with four threads and six-line batches to 2.58 s with single-line inference in the tuning run. PDF/image output retains identical region coordinates. Compared with the old engine, sampled text is identical after whitespace normalization; confidence values can differ with batch shape. The dense page has the same 34 regions, with one space changed in its page number.

The old native 62-page task completed successfully at 2026-09-07 17:05:43 local time (62 pages, 89,156 Markdown bytes). This audit does not claim an optimized full-62-page timing.

## Verification

- Rust application: 212 library tests passed, one opt-in OCR test skipped; four pipeline tests passed again after adding the Markdown-only validation regression.
- Engine: ten release/model tests passed, two opt-in tests skipped in the regular suite; the actual multilingual/forged-layer OCR test then passed explicitly, including searchable PDF output.
- Actual Markdown-only runs passed: PDF, PNG, dense text, four-page complex layout, Chinese/English/German/French/Japanese, vertical Japanese and deceptive hidden text. No searchable PDF emitted; source-map artifacts retained.
- TypeScript and the progress component regression passed: listener ordering, canonical-path changes, stale-job isolation, ETA and finalizing before completion.
- Real command bridge UI regression passed: PDF/TXT/DOCX, bad-file retry, output preservation and Word export; database integrity `ok`, zero foreign-key and browser errors. Profile: `/Users/only/Documents/Casy-Local-Test/conversion-ui-jIwz8k`.
- Desktop/mobile progress screenshots use simulated native events; the actual conversions above use the Rust bridge. Benchmark scripts, results and screenshots: `/Users/only/Documents/Casy-Local-Test/ocr-speed-20260907`.

## Package

- DMG: `/Users/only/Downloads/Casy-0.1.0-ocr-speed-20260907.dmg`, 808,199,819 bytes; Apple Silicon, macOS 26+, ad-hoc beta.
- SHA256: `6babe313bb49b7e440acb251207fb527576c5a7d88280789e27644819586f078`.
- Full desktop build, signature verification, all 2418 runtime file hashes, OCR/rendering availability, local E5 embeddings (768 dimensions), Zvec FP16 HNSW and DMG checksum verification passed.
- Implementation commit: `a7b6007`. No installed application or formal database was replaced by this task.
