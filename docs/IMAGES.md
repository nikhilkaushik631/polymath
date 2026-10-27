# Images

Images is Polymath's local Lightcraft photo workspace. Opening a supported image first shows a read-only viewer in Reader. The top-bar **Edit in Images** action adds the original in place to the Polymath Images library under Application Support; it does not move the file. The editor's adjustments are non-destructive until you export to a path you choose.

Use Home's **Images** card to enter the editor directly, or open an image and choose **Edit in Images** from the viewer. The viewer offers Fit, zoom and drag-to-pan without changing the file. Make adjustments in the image editor and export a new file. Check the exported dimensions, appearance and format before replacing or sharing an original. Supported formats depend on Lightcraft's import/export engines; common inputs include PNG, JPEG, TIFF, WebP and supported camera RAW files.

For agents, `polymath_commands {workspace:"images"}` exposes available engine commands. Inspect the selected image with `polymath_document {workspace:"images"}`, discover adjustment/export parameters, then verify the output file on disk. The [Images control protocol](../../lightcraft/docs/control-protocol.md) has detailed command schemas and limitations.
