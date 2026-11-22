<p align="center"><img src="packaging/urlmenu.svg" width="128" alt=""></p>

# urlmenu

A link handler for Linux that asks which browser to open each link in, using your own launcher (rofi, fuzzel, wofi, tofi, walker, bemenu, dmenu…) with its theme and keybindings. It draws no window of its own.

- Finds browsers from their desktop entries, including their Firefox and Chromium profiles.
- Adds a private-window variant for each browser.
- Applies rules: a host glob or a URL regex opens a given browser or profile without asking.
- Unwraps redirect links (Outlook safelinks, Google `/url`, Facebook, Instagram) and strips tracking parameters (`utm_*`, `fbclid`, `gclid`…).
- Copies the link to the clipboard (`wl-copy`, or `xclip` on X11).

## Install

```sh
cargo install --path .
urlmenu setup                # default handler and shell completion
urlmenu setup --handler      # default handler only
urlmenu setup --completion   # shell completion only
urlmenu setup --undo         # remove both, restore the previous handler
urlmenu setup --print        # show what it would do
urlmenu --help               # every command; `urlmenu <command> --help` for its options
```

With Nix, `nix run github:bitxikeria/urlmenu` runs it; the flake also exports `overlays.default`. On NixOS, set the handler with `xdg.mime.defaultApplications` for `x-scheme-handler/http` and `x-scheme-handler/https` to `urlmenu.desktop`.

## Usage

```sh
urlmenu https://example.com   # rule match, or picker
urlmenu                       # pick a browser to start, without a URL
urlmenu list                  # targets with their ids and commands
urlmenu explain <url>         # cleaned URL, the rule that handles it, the site history
urlmenu forget <url|host>     # reset the picks remembered for a site (--all: every site)
urlmenu --dry-run <url>       # print the command as JSON instead of running it
```

## Shell completion

Subcommands, options, remembered sites (`explain`, `forget`) and target ids (`open --target`) complete from the live state. `urlmenu setup` (or `setup --completion`) writes a completion script for each shell found in `PATH`:

| Shell | Script |
|---|---|
| fish | `$XDG_CONFIG_HOME/fish/completions/urlmenu.fish` |
| bash | `$XDG_DATA_HOME/bash-completion/completions/urlmenu`, loaded by bash-completion |
| zsh | `$XDG_DATA_HOME/zsh/site-functions/_urlmenu`; add `fpath=(~/.local/share/zsh/site-functions $fpath)` to `~/.zshrc` before `compinit` |

## Launchers

| Launcher | Icons | Private window and copy | Selection |
|---|---|---|---|
| `rofi` | yes | shortcuts, reminded under the URL | target id |
| `fuzzel` | yes | shortcuts, reminded above the prompt | index |
| `bemenu` | no | `[Private]` and Copy rows | label |
| `wofi`, `tofi`, `walker`, `dmenu`, `{ command = [...] }` | no | `[Private]` and Copy rows | label |

Without `launcher`, urlmenu uses the first of rofi, fuzzel, wofi, tofi, walker, bemenu and dmenu found in `PATH`.

Texts are in English or French, following `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` and `LANG` in that order. Browser names come from the localized `Name[…]` of their desktop entries.

rofi runs in script mode: it calls `urlmenu __rofi <mode>` to list rows and to record the choice, and the link reaches it through `$URLMENU_URL`.

## Suggestions

When the picker opens, the cursor is on the suggested target, so `Enter` opens it. The list keeps its order in rofi, fuzzel and bemenu; launchers that cannot preselect a row (wofi, tofi, walker, dmenu, custom commands) list the suggestion first instead.

The suggestion is:

1. the target with the best score for this site (host without `www.`): each pick from the picker adds 1 to its rank, weighted by the age of the last pick (×4 under an hour, ×2 under a day, ×0.5 under a week, ×0.25 beyond), like zoxide;
2. otherwise `favorite_browser`.

Rules still open directly, without the picker. Picks are kept in `$XDG_STATE_HOME/urlmenu/history.json` (500 sites at most). Above a total rank of 20 per site, ranks age by 10% and the faded ones drop. A private pick suggests the same browser and profile in both views.

## External pickers

Any interface can draw the menu and hand the choice back:

```sh
urlmenu items <url>                     # menu as JSON
urlmenu open --target <id> [<url>]      # open with a target id
urlmenu copy <url>                      # copy the cleaned URL
```

`items` prints:

```json
{
  "version": 1,
  "rule": null,
  "url": "https://example.com/?id=1",
  "items": [
    {
      "id": "chromium/Default",
      "label": "Chromium [profile “Work”]",
      "icon": "chromium",
      "private": { "id": "chromium/Default+private", "label": "[Private] Chromium [profile “Work”]" }
    }
  ],
  "copy": true,
  "suggested": "chromium/Default"
}
```

- `url` is the cleaned link, `null` without one.
- `rule` is the target id a rule would open directly, or `null`.
- `private` is `null` for browsers without a private mode.
- `suggested` is the target to preselect, or `null`.
- Target ids are `<browser>[/<profile>][+private]`, with `/`, `+` and non-ASCII bytes percent-encoded. They stay stable across runs.

## Configuration

`~/.config/urlmenu/config.toml` (or `$URLMENU_CONFIG`). Every key is optional. [`config.sample.toml`](config.sample.toml) lists them all with their defaults.

```toml
launcher = "rofi"                        # rofi | fuzzel | wofi | tofi | walker | bemenu | dmenu
launcher_args = ["-theme", "my.rasi"]    # appended to the launcher command
# launcher = { command = ["bemenu", "-i", "-p", "{url}"] }   # {url} is replaced by the link

private_entries = true   # "[Private]" rows for launchers without custom keys (wofi, dmenu, commands)
hide_icons = false       # true leaves out browser icons in rofi and fuzzel
clipboard_entry = true   # offer to copy the link: a shortcut in rofi and fuzzel, a row elsewhere
remember = true          # suggest the most frequent and recent pick for each site
favorite_browser = "firefox-developer-edition"   # target or browser id suggested when a site has no history
clean = true
extra_params = ["si", "pk_*"]   # more query keys to strip; trailing * matches a prefix

excluded_browsers = ["chromium"]   # desktop entry ids to leave out

# Keys in rofi syntax, for rofi and fuzzel. Pick keys the launcher leaves free.
[shortcuts]
open_in_private_window = "Alt+Return"
copy_link = "Control+Shift+c"
show_hints = true   # reminder of these keys under the URL

# Override a detected browser, or add one (then `exec` is required).
[[browser]]
id = "qutebrowser"
name = "qutebrowser"
icon = "qutebrowser"
exec = "qutebrowser %u"
private_exec = "qutebrowser --target private-window %u"

# First matching rule wins. `host` and `url` both set means both must match.
[[rule]]
host = "*.example.org"          # does not match example.org itself
browser = "firefox-developer-edition"
profile = "work"                # profile name or directory; first profile when omitted

[[rule]]
url = '^https://meet\.google\.com/'
browser = "chromium"
private = true
```

Browser ids are desktop entry names without `.desktop`, as `urlmenu list` shows them.

A Firefox profile belongs to the browser installed in its `LastPlatformDir`, so Firefox and Firefox Developer Edition each list their own profiles. A browser with a single profile gets no profile lines.

## Packaging

Build with `cargo build --release --locked` (Rust 1.88 or later). `cargo test` needs neither network nor a home directory.

| Source | Destination |
|---|---|
| `target/release/urlmenu` | `/usr/bin/urlmenu` |
| `packaging/urlmenu.desktop` | `/usr/share/applications/urlmenu.desktop` |
| `packaging/urlmenu.svg` | `/usr/share/icons/hicolor/scalable/apps/urlmenu.svg` |
| `packaging/man/*.1` | `/usr/share/man/man1/` |
| `packaging/completions/urlmenu.bash` | `/usr/share/bash-completion/completions/urlmenu` |
| `packaging/completions/urlmenu.fish` | `/usr/share/fish/vendor_completions.d/urlmenu.fish` |
| `packaging/completions/_urlmenu` | `/usr/share/zsh/site-functions/_urlmenu` |
| `config.sample.toml` | `/usr/share/doc/urlmenu/config.sample.toml` |

`packaging/install.sh` installs these files; it honors `PREFIX` (default `/usr/local`), `DESTDIR` and `BIN`, and works from the source tree or from a release archive, which ships a static binary for x86_64 and aarch64. Each release also ships `.deb` and `.rpm` packages, built by `packaging/build-packages.sh` with [nfpm](https://nfpm.goreleaser.com/).

With the desktop entry installed system-wide, `urlmenu setup --handler` only makes it the default handler.

Runtime dependencies: `xdg-utils`, `wl-clipboard` (Wayland) or `xclip` (X11), and one supported launcher.

## License

Public domain, see [UNLICENSE](UNLICENSE).
