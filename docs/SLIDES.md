# Slides

Slides is the presentation workspace. Open a `.pptx`, `.potx`, `.ppsx` or native Deckcraft file from Home to edit it. Reader can preview a presentation locally as an in-memory PDF; **Edit in Slides** opens the original. Polymath preserves the presentation session while you move to another workspace and checks its unsaved state on quit.

Use the slide rail, canvas and editing ribbon to compose the deck. Save an editable presentation or export PDF when you need a shareable fixed-layout copy. Check exported slides visually and reopen the editable file before handing it off, especially when fonts, images or page geometry matter.

PPTX paragraphs can carry font size, weight, family and color on `a:pPr/a:defRPr` while individual text runs leave those fields empty. Slides now inherits these properties during rendering and keeps them when saving the presentation. This fixes tiny or missing text in decks authored this way. Fidelity can still vary for unsupported PowerPoint effects, so inspect exported slides before sharing a complex deck.

Coding agents should inspect `polymath_commands {workspace:"slides"}` for the supported slide, shape, text and export parameters and render the result for review. See the [Slides control protocol](../../slides/docs/control-protocol.md) for the full command schema.
