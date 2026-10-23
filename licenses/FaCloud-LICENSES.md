# FaCloud Licensing Guide

> Status: Current
> Owner: Repository maintainers
> Canonical for: repository license boundary and attribution guidance

[English](LICENSES.md) | [简体中文](LICENSES.zh-CN.md)

This guide explains where the repository's authoritative license texts apply.
It does not replace those texts and is not legal advice.

## License Boundary

| Scope | License | Authoritative text |
| --- | --- | --- |
| `desktop/crates/facloud_ui_core/` | MIT | [`LICENSE-MIT`](LICENSE-MIT) |
| `desktop/crates/facloud_ui/` | MIT | [`LICENSE-MIT`](LICENSE-MIT) |
| All other FaCloud-owned repository code | AGPL-3.0-only | [`LICENSE-AGPL`](LICENSE-AGPL) |
| Bundled third-party components and assets | Their stated terms | License or notice adjacent to the component |

Renaming the inherited UI crates from `warpui_core` / `warpui` to
`facloud_ui_core` / `facloud_ui` did not change the upstream MIT boundary. New
changes inside those two crates are contributed under MIT. New FaCloud-owned
changes elsewhere are contributed under AGPL-3.0-only.

## Attribution

- Inherited Warp portions are Copyright (C) 2020-2026 Denver Technologies,
  Inc.
- FaCloud-specific modifications are Copyright (C) 2026 their respective
  contributors.
- Keep [`NOTICE`](NOTICE), both root license files, and component-level notices
  in source and binary redistributions.

## AGPL Network Use

AGPL section 13 can require an operator that modifies covered software and lets
users interact with it over a network to offer those users the Corresponding
Source of the running modified version. Operators are responsible for applying
the license to their deployment and distribution model.

## Trademarks

The open-source licenses grant copyright permissions; they do not grant rights
to Warp or other third-party trademarks. FaCloud is an independent project and
must not be presented as an official Warp release.
