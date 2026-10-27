# Sheets

Sheets handles spreadsheets and delimited data, including `.xlsx`, `.xlsm`, Excel templates (`.xltx`, `.xltm`), `.xlsb`, `.ods`, `.csv` and `.tsv` where supported by its import engine. Home's **Open file** routes them here. Reader can render a read-only PDF preview first; **Edit in Sheets** opens the original for changes. Macro-enabled files and templates open as imports and require **Save As .xlsx**; their originals are never overwritten by Sheets. Legacy Excel `.xls` needs conversion to `.xlsx` in another app because Sheets does not import it yet.

Use the grid and formula bar to edit cells, then save or export through Sheets. CSV and TSV are plain tabular formats; use a workbook format if you need to retain workbook structure or richer formatting. Reopen saved output and inspect key cells, formulas and sheets. Workspace switching preserves the active Sheets session; unsaved changes are included in Polymath's quit guard.

Agents can use `polymath_commands {workspace:"sheets"}` to discover cell, sheet and export actions, then `polymath_document {workspace:"sheets"}` to inspect results. The [Sheets control protocol](../../sheets/docs/control-protocol.md) describes command parameters and data model details.
