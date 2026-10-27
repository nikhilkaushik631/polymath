# Native UI stack

Polymath renders its shared shell and editor integrations with Rust, eframe and egui 0.36. Home uses original egui drawing code for workspace previews and Inter for its interface typography. The editor surfaces remain in their sibling Rust repositories; switching workspaces keeps their state in memory.

| Library | Decision | Reason |
| --- | --- | --- |
| [egui_extras 0.36](https://docs.rs/egui_extras/0.36.0/egui_extras/) | In use | Image/SVG loading and virtualized tables for Reader's source outline and media queue. |
| [egui_dock 0.21.1](https://docs.rs/egui_dock/0.21.1/egui_dock/) | In use | Reader's source preview has a resizable Document/Outline dock. Home stays a workspace launcher. |
| [egui-shadcn 0.5.0](https://github.com/FerrisMind/shadcn-rs) | Local egui 0.36 port in `vendor/egui-shadcn` | The published crate targets egui 0.33. Polymath ports its egui API calls and uses its real button, badge, input and switch components in Home, the shared toolbar, Reader source preview, media controls and Settings. The port retains its MIT notice. |
| [egui-material3 0.0.9](https://docs.rs/crate/egui-material3/0.0.9/source/Cargo.toml) | Design reference | Also targets egui 0.33. |
| [Ouroboros UI](https://github.com/Type-zero-labs/ouroboros-ui) | Design reference | Targets egui 0.34.1; its repository is archived. |

The shadcn port maps semantic colors to Polymath's neutral surfaces and blue/teal/green/terracotta/amber workspace accents. It does not change document colors or editor fonts. Home's file library and the five editor engines remain their existing egui implementations; this is a progressive component adoption, not a replacement of every editor UI.

Reader's source outline is populated from Markdown headings and common code declarations. Selecting an entry scrolls the read-only document to that line. Files without an outline show only Document. Dock layout lives with the open source file, so switching workspaces preserves it without modifying the file.
