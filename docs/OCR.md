# Local OCR in Polymath

Open a PDF in **PDF Tools**, choose **Recognize text**, select the pages, and start the job. OCR runs only after that action. The app loads its local recognition model on the worker thread, then shows a progress card while it processes pages. You can cancel a document run; completed pages are kept in the result. The source image is left unchanged and recognized words become searchable PDF text.

The current English engine is [`ocrs`](https://github.com/robertknight/ocrs) with two bundled model files totaling about **11.7 MB** (2.4 MB detection and 9.3 MB recognition). It needs no Python or Paddle runtime. The OCR dialog offers English because that is the language these installed models can actually read. It does not infer table cells or reliably recognize Indian scripts.

## Model choice for this Mac

| Option | Fit for the 8 GB M1 Mac | Status |
| --- | --- | --- |
| Existing `ocrs` | Small bundled files; loads only when OCR is requested | **Current English default** |
| Apple Vision text recognition | Uses macOS frameworks, so there is no additional model bundle; needs a separate engine adapter and output comparison | Research alternative for English |
| PP-OCRv5 mobile | Official recognition files for [Devanagari, Tamil and Telugu](https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/algorithm/PP-OCRv5/PP-OCRv5_multi_languages.en.md) are about 8 MB each, plus a shared detector, but the runtime and peak memory must be measured on this Mac | Candidate only if more languages are requested |
| PaddleOCR-VL-1.6 | [1.92 GB checkpoint](https://huggingface.co/PaddlePaddle/PaddleOCR-VL-1.6/tree/main), plus runtime memory; good for complex layout and tables but too large for the default path. [Official Apple Silicon instructions](https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/pipeline_usage/PaddleOCR-VL-Apple-Silicon.en.md) report validation on M4, not M1 | Not installed or invoked |

Small model files do not guarantee a small process: PaddleOCR's own [PP-OCRv5 pipeline benchmarks](https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/algorithm/PP-OCRv5/PP-OCRv5.en.md) report gigabytes of host memory for the Python pipeline on their tested CPUs. Those numbers are not M1 measurements. English-only OCR should keep using the existing engine unless a measured quality problem warrants changing it.
