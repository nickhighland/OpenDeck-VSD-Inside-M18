# Project roadmap

Keep this checklist focused on public, device-agnostic work. Do not add private device identifiers, personal profile exports, local paths, or credentials.

## M18 behavior and VSD Craft compatibility

- [ ] Verify on hardware: computer-wide inactivity starts the screensaver; the first M18 press wakes it without activating the assigned action.
- [ ] Verify video and photo screensavers cover the full display, preserve LED state, and loop or advance reliably.
- [ ] Verify occupied-slot drag/drop swaps complete actions, states, settings, and artwork; empty-slot drop remains a move.
- [ ] Preserve the distinction between ordinary Hotkey and Super Hotkey, including switch variants and imported shortcut semantics.
- [ ] Implement persistent boot-logo replacement only after the complete upload protocol, validation, acknowledgement, and recovery behavior are understood.
- [ ] Continue VSD Craft feature audit and port remaining useful action families as native M18 actions.
- [ ] Capture representative VSD Craft screenshots for comparison; do not commit personal profile exports or device identifiers.

## Build, release, and operations

- [x] Add an Unraid verification workflow for trusted pushes to `main` and manual runs; keep release builds on GitHub-hosted platform runners.
- [ ] Verify the dedicated repository-level Unraid runner completes Rust tests and frontend checks/builds.
- [ ] Keep public pull-request jobs off the persistent Unraid runner; do not add an unsafe self-hosted `pull_request` trigger.
- [ ] Document profile backup/import, macOS background launch, Linux/Unraid setup, and recovery steps.
- [ ] Run long-duration reconnect and crash-recovery testing on supported hardware.
