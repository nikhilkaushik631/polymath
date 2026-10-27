# Local PDF tools in Polymath

Open **Home → PDF Tools** for Combine, Organize pages, Compress, OCR, Protect, Redact, Export and related actions. The complete catalog is expanded by default. **Editing & review**, below a section divider, holds text/image editing, comments, forms, signing and other document work. The separate Reader workspace stays focused on reading PDFs and source files. This is a navigation change, not a claim that every iLovePDF feature has been implemented. [Local OCR](OCR.md) explains the English model, on-demand progress and language limits.

The [iLovePDF tool list](https://www.ilovepdf.com/) was reviewed on 2026-10-14 as a description of useful workflows. Its layout, icons, names and assets are not copied. We found open-source iLovePDF API clients, not source for the website; Polymath's implementation uses local, independently licensed Rust code.

| Workflow | Current local implementation | Gap to close |
| --- | --- | --- |
| Merge, split, organize, rotate, crop, page numbers | Reader's page and combine commands are available | Batch polish and larger-file testing |
| Compress, protect, unlock, redact, OCR, compare, PDF/A, forms | Reader has corresponding commands and panels | Check results with diverse real-world files; OCR language/model coverage is limited |
| PDF to Word, text, HTML, PNG | Reader export commands are available | DOCX fidelity varies; JPG output and semantic Markdown export need dedicated work |
| Word, spreadsheet, slides to PDF | Writer, Sheets and Slides have PDF export commands | A single Reader batch-conversion flow is not wired up |
| PDF to editable Excel or PowerPoint | Reader catalog marks these as planned | Table and slide reconstruction need new engines and fidelity tests |
| Repair damaged PDFs | The parser tolerates some damaged input | No dedicated Repair action or recovery report |
| Scan from a camera, request remote signatures, AI summary/translation, cloud workflows | No local equivalents in this build | Keep remote services out of the ordinary local app; consider opt-in local alternatives separately |

The strongest Rust foundation is [PdfCraft](https://github.com/storytold/pdfcraft), already forked as Reader. It already uses a vendored [lopdf](https://github.com/J-F-Liu/lopdf) for PDF structure work. [oxidizePdf](https://github.com/bzsanti/oxidizePdf) offers additional pure-Rust manipulation and PDF/A validation, and [pdf_oxide](https://github.com/yfedoseev/pdf_oxide) offers text/image extraction and PDF-to-Markdown ideas. These are research candidates, not added dependencies. Any integration needs a specific missing workflow, license review, and output tests before it replaces Reader code.

For coding agents, use `polymath_commands {workspace:"reader"}` and the Reader CLI's headless PDF tools. The PDF Tools sidebar uses the same tool-catalog IDs, so UI discovery and command discovery stay aligned.
