# Vendored noVNC (pinned)

The noVNC client engine (`core/`) and its one dependency (`vendor/pako`),
vendored unmodified from the [noVNC](https://github.com/novnc/noVNC) release
**v1.6.0** — see `LICENSE.txt` (core is MPL-2.0). Embedded into the `ewm`
binary at build time (`ewm/build.rs`) and served by the built-in web console
(`web.rs`, notes/REMOTE.md Phase 5).

`index.html` is EWM's own minimal console page (not part of noVNC): it loads
`core/rfb.js` and connects back to the host/port it was served from.

To update: copy `core/` and `vendor/` from a newer tagged release, update the
version here and in notes/REMOTE.md, and re-run the browser gate.
