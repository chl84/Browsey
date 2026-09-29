# PDF regression fixtures

`encrypted.pdf` is PDFium's encryption test input, downloaded unchanged from:

https://github.com/chromium/pdfium/blob/27ddf161579f79510b361d0016ccc7f0cdffdc6d/testing/resources/encrypted.pdf

SHA-256: `cbbead7185d9cd46daecaf60165a9aa7b9f2daceb3623218c328cb015ae862d5`

User password: `1234`; owner password: `5678`. These are public fixture values,
documented in the same commit's `cpdf_security_handler_embeddertest.cpp`.
The upstream BSD-style license is included as `LICENSE`. This fixture is for
tests only and is not included in Browsey's packaged resources.

Other test PDFs are generated independently by a minimal Rust PDF writer in
`src/commands/thumbnails/thumbnails_pdf_tests.rs`. They cover page rotation,
standard fonts, extreme aspect ratios, metadata, malformed inputs, repeated
calls, and concurrent thumbnail/metadata requests without external test tools.
