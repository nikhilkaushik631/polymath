# Polymath compatibility port

This directory starts from the published `egui-shadcn` 0.5.0 crate by FerrisMind (MIT). The published crate uses egui 0.33; Polymath and all five editor integrations use egui 0.36. This path dependency keeps one egui version in the application.

The port updates changed egui calls (`Context::content_rect`, `TextEdit::frame`, scroll input and ID salts), adds serialization for persistent calendar/collapsible state when egui persistence is enabled, and adds accessibility labels to the button and switch components used by Polymath. The optional plotting adapter is excluded from this port. Other upstream component behavior is retained.

Polymath uses the port's button, badge, input and switch components. `src/ui_kit.rs` maps their semantic colors to Polymath's light/dark surfaces and workspace accents. The upstream MIT license is in `LICENSE-MIT` and copied into the macOS bundle.
