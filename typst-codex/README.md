# Typst Codex

Type [Typst](https://typst.app) symbol names anywhere and get the symbol. Generated from Typst's [`codex`](https://github.com/typst/codex) crate, so the names match Typst's [`sym`](https://typst.app/docs/reference/symbols/sym/) and [`emoji`](https://typst.app/docs/reference/symbols/emoji/) modules exactly. Symbols from codex 0.3.0.

## Usage

Wrap a Typst symbol name in semicolons:

| Type | Get |
|-|-|
| `;alpha;` | α |
| `;arrow.r.double;` | ⇒ |
| `;integral;` | ∫ |
| `;emoji.rocket;` | 🚀 |
| `;alpha;;beta;` | αβ |

Can't remember a name? Press <kbd>Alt</kbd>+<kbd>Space</kbd> to open Espanso's search, then search by name (`arrow`) or paste a glyph (`⇒`).

Modifiers follow codex's order: `;arrow.r.double;` works, `;arrow.double.r;` doesn't. Emoji are prefixed with `emoji.`.

## Why semicolons?

Most Hub packages use `:name:`, but semicolons are better here:

- **No Shift key** – `;` is on the home row of US layouts.
- **No collisions** – `:smile:`, `:star:` and friends are already emoji shortcodes in Slack, Discord, GitHub and the all-emojis package.
- **Closing delimiter** – Typst names nest, so `;arrow` can't fire before you finish typing `;arrow.r.double;`. It also lets symbols sit side by side.

## Tip

If you write Typst in an editor, you probably want the real `sym.arrow.r.double` there, not ⇒. Consider [disabling Espanso for that app](https://espanso.org/docs/configuration/app-specific-configurations/).
