# Typst Codex for Espanso

[![CI](https://github.com/munr0/espanso-typst-codex/actions/workflows/ci.yml/badge.svg)](https://github.com/munr0/espanso-typst-codex/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/munr0/espanso-typst-codex)](https://github.com/munr0/espanso-typst-codex/releases)

Type [Typst](https://typst.app) symbol names anywhere and get the symbol – `;arrow.r.double;` becomes ⇒.

An [Espanso](https://espanso.org) package with every symbol and emoji from Typst's [`sym`](https://typst.app/docs/reference/symbols/sym/) and [`emoji`](https://typst.app/docs/reference/symbols/emoji/) modules – about 2,500 of them, with the exact names you already use in Typst. It works in any app: your browser, chat, notes, email.

| Type | Get |
|-|-|
| `;alpha;` | α |
| `;arrow.r.double;` | ⇒ |
| `;integral;` | ∫ |
| `;emoji.rocket;` | 🚀 |
| `;alpha;;beta;` | αβ |

Can't remember a name? Press <kbd>Alt</kbd>+<kbd>Space</kbd> and search by name or glyph.

## Getting Started

### 1. Install Espanso

- **Windows:** `winget install --id Espanso.Espanso -e`
- **macOS:** `brew install espanso`, then grant Accessibility permission when prompted
- **Linux:** follow the [official install guide](https://espanso.org/docs/install/linux/) – the method depends on your distro and on X11 vs Wayland. Then run `espanso service register` and `espanso start`.

### 2. Install the package

From the Espanso Hub:
```sh
espanso install typst-codex
```

Or straight from this repo:
```sh
espanso install typst-codex --git https://github.com/munr0/espanso-typst-codex --git-branch release --external
```

### Update

```sh
espanso package update typst-codex
```

Run `espanso restart` if triggers don't fire afterwards.

## Usage

See the [package README](typst-codex/README.md) for the details – including why it's `;name;` and not `:name:`.

## Always Up to Date

The package is generated straight from [`codex`](https://github.com/typst/codex), the crate Typst itself uses for symbol names. When codex releases, this repo picks it up automatically, and new symbols land in a new [release](https://github.com/munr0/espanso-typst-codex/releases) and on the Espanso Hub. Each release lists exactly which symbols were added or changed.

Versions follow semver, independent of codex's version:

| Bump | Means |
|-|-|
| major | Triggers were removed or renamed – something you type may stop working |
| minor | New codex version – new or changed symbols |
| patch | Same symbols, small fixes (e.g. search labels) |

## Contributing

See [DEV.md](DEV.md) for how the generator and release pipeline work.

## Credits

Symbols come from [typst/codex](https://github.com/typst/codex), licensed under Apache-2.0.
