mod config;
mod detect;
mod exec;
mod history;
mod ini;
mod launch;
mod launcher;
mod locale;
mod menu;
mod paths;
mod rules;
mod setup;
mod target;
mod url;

use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::engine::{ArgValueCandidates, CompletionCandidate};
use serde::Serialize;

use crate::config::Config;
use crate::launcher::{Options, rofi};
use crate::locale::Messages;
use crate::menu::{Choice, Menu};
use crate::paths::Paths;
use crate::target::Target;

/// Pick a browser for each link, through your launcher.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Print what would run as JSON instead of running it.
    #[arg(long, global = true)]
    dry_run: bool,
    #[command(subcommand)]
    command: Option<Cmd>,
    /// URLs or files to open, with one pick for those no rule handles.
    /// Without any, pick a browser to start.
    urls: Vec<String>,
}

#[derive(Subcommand)]
enum Cmd {
    /// List detected targets with their ids and commands.
    List,
    /// Show how a URL is cleaned, which rule or picker handles it, and its
    /// history; a fragment such as `github` lists every matching site.
    Explain {
        #[arg(add = ArgValueCandidates::new(complete_sites))]
        url: String,
    },
    /// Print the menu as JSON, for an external picker.
    Items { url: Option<String> },
    /// Open URLs (or nothing) with a target id from `list` or `items`.
    Open {
        #[arg(long, add = ArgValueCandidates::new(complete_targets))]
        target: String,
        urls: Vec<String>,
    },
    /// Copy the cleaned URLs to the clipboard, one per line.
    Copy {
        #[arg(required = true)]
        urls: Vec<String>,
    },
    /// Reset the picks remembered for a site, or for every site.
    Forget {
        /// URL or host, e.g. `github.com`.
        #[arg(
            required_unless_present = "all",
            conflicts_with = "all",
            add = ArgValueCandidates::new(complete_sites)
        )]
        site: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Register urlmenu as the default handler for http, https and HTML, and
    /// install shell completion. Without a part flag, does both.
    Setup {
        /// Only the default handler: desktop entry and `xdg-mime default`.
        #[arg(long)]
        handler: bool,
        /// Only the completion scripts, for each of fish, bash and zsh in `PATH`.
        #[arg(long)]
        completion: bool,
        /// Remove what setup wrote and restore the previous default handler.
        #[arg(long)]
        undo: bool,
        /// Print the files and commands instead of applying them.
        #[arg(long)]
        print: bool,
    },
    /// rofi script-mode entry point.
    #[command(name = "__rofi", hide = true)]
    Rofi {
        /// Row text rofi passes on selection; the choice is read from `ROFI_INFO`.
        #[arg(allow_hyphen_values = true)]
        _selection: Option<String>,
    },
}

struct RuleMatch {
    rule: String,
    target: Result<usize, String>,
}

struct Ctx {
    paths: Paths,
    cfg: Config,
    msg: &'static Messages,
    targets: Vec<Target>,
}

impl Ctx {
    fn load() -> Result<Self, String> {
        let paths = Paths::from_env();
        let cfg = config::load(&config::path(&paths))?;
        let languages = locale::languages_from_env();
        let msg = locale::messages(&languages);
        let targets = target::build(&detect::browsers(&paths, &cfg, &languages), msg);
        Ok(Self {
            paths,
            cfg,
            msg,
            targets,
        })
    }

    fn url(&self, input: &str) -> Result<String, String> {
        let url = url::normalize(input.trim(), self.cfg.clean, &self.cfg.extra_params);
        if url.starts_with('-') {
            return Err(format!(
                "refusing argument that looks like an option: {url:?}"
            ));
        }
        Ok(url)
    }

    fn urls(&self, inputs: &[String]) -> Result<Vec<String>, String> {
        inputs.iter().map(|i| self.url(i)).collect()
    }

    fn options(&self) -> Options {
        Options::new(&self.cfg, self.msg)
    }

    /// Ranked: best-scored pick for the site of the first URL, then the
    /// favorite browser.
    fn menu(&self, urls: &[String]) -> Menu {
        let url = urls.first().cloned();
        let mut preferred = Vec::new();
        if self.cfg.remember {
            let key = history::key(url.as_deref());
            let history = history::History::load(&history::path(&self.paths));
            preferred.extend(history.best(&key, history::now()).map(String::from));
        }
        preferred.extend(self.favorite_target());
        let mut menu = Menu::build(&self.targets, url, self.cfg.clipboard_entry, &preferred);
        menu.more = urls.len().saturating_sub(1);
        menu
    }

    /// `favorite_browser` as a target id, or the first normal target of that browser.
    fn favorite_target(&self) -> Option<String> {
        let wanted = self.cfg.favorite_browser.as_deref()?;
        let normal = || self.targets.iter().filter(|t| !t.private);
        normal()
            .find(|t| t.id() == wanted)
            .or_else(|| normal().find(|t| t.browser_id == wanted))
            .map(Target::id)
    }

    /// Records the pick once for each site among `urls`.
    fn remember(&self, urls: &[String], id: &str) {
        if !self.cfg.remember {
            return;
        }
        let path = history::path(&self.paths);
        let mut history = history::History::load(&path);
        let mut keys: Vec<String> = urls.iter().map(|u| history::key(Some(u))).collect();
        if keys.is_empty() {
            keys.push(history::key(None));
        }
        keys.sort();
        keys.dedup();
        let now = history::now();
        for key in keys {
            history.record(key, id, now);
        }
        if let Err(e) = history.save(&path) {
            eprintln!("urlmenu: {e}");
        }
    }

    /// The first matching rule, described, and the target index it resolves to.
    fn rule_match(&self, url: &str) -> Result<Option<RuleMatch>, String> {
        let rules = rules::compile(&self.cfg.rules)?;
        Ok(rules::find(&rules, url).map(|rule| RuleMatch {
            rule: rule.describe(),
            target: rule.resolve(&self.targets),
        }))
    }

    /// Target index chosen by the first matching rule, if it resolves.
    fn rule_target(&self, url: &str) -> Result<Option<usize>, String> {
        Ok(self.rule_match(url)?.and_then(|m| {
            m.target
                .map_err(|e| eprintln!("urlmenu: {e}; falling back to the picker"))
                .ok()
        }))
    }

    fn open_target(&self, id: &str, urls: &[String], dry_run: bool) -> Result<(), String> {
        let i = target::find(&self.targets, id).ok_or_else(|| format!("unknown target {id:?}"))?;
        self.open_index(i, urls, dry_run)
    }

    fn open_index(&self, i: usize, urls: &[String], dry_run: bool) -> Result<(), String> {
        self.targets[i]
            .commands(urls)
            .iter()
            .try_for_each(|argv| launch::open(argv, dry_run))
    }
}

fn main() -> ExitCode {
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            launch::report(&e);
            ExitCode::FAILURE
        }
    }
}

/// Remembered sites, for `explain` and `forget`.
fn complete_sites() -> Vec<CompletionCandidate> {
    let paths = Paths::from_env();
    history::History::load(&history::path(&paths))
        .sites_matching("")
        .into_iter()
        .map(CompletionCandidate::new)
        .collect()
}

/// Target ids with their labels, for `open --target`.
fn complete_targets() -> Vec<CompletionCandidate> {
    Ctx::load()
        .map(|ctx| {
            ctx.targets
                .iter()
                .map(|t| CompletionCandidate::new(t.id()).help(Some(t.label.clone().into())))
                .collect()
        })
        .unwrap_or_default()
}

fn run(cli: Cli) -> Result<(), String> {
    let ctx = Ctx::load()?;
    let dry_run = cli.dry_run;
    match cli.command {
        Some(Cmd::List) => list(&ctx),
        Some(Cmd::Explain { url }) => explain(&ctx, &url),
        Some(Cmd::Items { url }) => items(&ctx, url.as_deref()),
        Some(Cmd::Open { target, urls }) => ctx.open_target(&target, &ctx.urls(&urls)?, dry_run),
        Some(Cmd::Copy { urls }) => launch::copy(&ctx.urls(&urls)?.join("\n"), dry_run),
        Some(Cmd::Forget { site, all }) => forget(&ctx, site.as_deref(), all),
        Some(Cmd::Setup {
            handler,
            completion,
            undo,
            print,
        }) => setup::run(
            &ctx.paths,
            setup::Parts {
                handler,
                completion,
            },
            undo,
            print,
        ),
        Some(Cmd::Rofi { .. }) => rofi_script(&ctx),
        None => open(&ctx, &cli.urls, dry_run),
    }
}

/// Each URL a rule handles opens directly; the others share one pick.
fn open(ctx: &Ctx, inputs: &[String], dry_run: bool) -> Result<(), String> {
    let mut picked = Vec::new();
    for url in ctx.urls(inputs)? {
        match ctx.rule_target(&url)? {
            Some(i) => ctx.open_index(i, std::slice::from_ref(&url), dry_run)?,
            None => picked.push(url),
        }
    }
    if !inputs.is_empty() && picked.is_empty() {
        return Ok(());
    }
    if ctx.targets.is_empty() {
        return Err("no browser detected".into());
    }
    let menu = ctx.menu(&picked);
    let launcher = launcher::from_config(&ctx.cfg, &ctx.paths, ctx.msg)?;
    match launcher.pick(&menu, ctx.options())? {
        Choice::Open(id) => {
            ctx.open_target(&id, &picked, dry_run)?;
            if !dry_run {
                ctx.remember(&picked, &id);
            }
            Ok(())
        }
        Choice::Copy => launch::copy(&picked.join("\n"), dry_run),
        Choice::Cancel => Ok(()),
    }
}

fn rofi_script(ctx: &Ctx) -> Result<(), String> {
    let url = std::env::var(rofi::URL_ENV).ok().filter(|u| !u.is_empty());
    let mut menu = ctx.menu(url.as_slice());
    if menu.url.is_some() {
        menu.more = std::env::var(rofi::MORE_ENV)
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
    }
    let retv = std::env::var("ROFI_RETV")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let info = std::env::var("ROFI_INFO").ok();
    match rofi::script(
        &menu,
        ctx.options(),
        &ctx.cfg.shortcuts,
        retv,
        info.as_deref(),
    ) {
        rofi::Script::Rows(rows) => {
            print!("{rows}");
            Ok(())
        }
        rofi::Script::Done(choice) => {
            let path = std::env::var_os(rofi::RESULT_ENV)
                .ok_or_else(|| format!("{} is not set", rofi::RESULT_ENV))?;
            rofi::write_result(std::path::Path::new(&path), &choice)
        }
    }
}

#[derive(Serialize)]
struct ItemsOutput<'a> {
    version: u32,
    /// Target a rule would open directly.
    rule: Option<String>,
    #[serde(flatten)]
    menu: &'a Menu,
}

fn items(ctx: &Ctx, input: Option<&str>) -> Result<(), String> {
    let url = input.map(|i| ctx.url(i)).transpose()?;
    let rule = match url.as_deref() {
        Some(u) => ctx.rule_target(u)?.map(|i| ctx.targets[i].id()),
        None => None,
    };
    let menu = ctx.menu(url.as_slice());
    let out = ItemsOutput {
        version: 1,
        rule,
        menu: &menu,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn list(ctx: &Ctx) -> Result<(), String> {
    let width = |f: fn(&Target) -> String| {
        ctx.targets
            .iter()
            .map(|t| f(t).chars().count())
            .max()
            .unwrap_or(0)
    };
    let (lw, iw) = (width(|t| t.label.clone()), width(Target::id));
    for t in &ctx.targets {
        println!(
            "{:lw$}  {:iw$}  {}",
            t.label,
            t.id(),
            shell_words(&t.command(Some("<url>")))
        );
    }
    Ok(())
}

/// A URL shows its cleaning, rule and history; a fragment such as `github`
/// shows every remembered site that contains it.
fn explain(ctx: &Ctx, input: &str) -> Result<(), String> {
    if !input.contains("://") {
        return explain_sites(ctx, input);
    }
    let url = ctx.url(input)?;
    println!("input:      {input}");
    println!("normalized: {url}");
    explain_rule(ctx, &url)?;
    if ctx.cfg.remember {
        let history = history::History::load(&history::path(&ctx.paths));
        print_history(&history, &history::key(Some(&url)));
    }
    Ok(())
}

fn explain_sites(ctx: &Ctx, fragment: &str) -> Result<(), String> {
    let history = history::History::load(&history::path(&ctx.paths));
    let sites = history.sites_matching(fragment);
    if sites.is_empty() {
        println!("no remembered site matches {fragment:?}");
    }
    for (i, site) in sites.iter().enumerate() {
        if i > 0 {
            println!();
        }
        println!("site:       {site}");
        explain_rule(ctx, &format!("https://{site}/"))?;
        print_history(&history, site);
    }
    Ok(())
}

fn explain_rule(ctx: &Ctx, url: &str) -> Result<(), String> {
    match ctx.rule_match(url)? {
        Some(m) => {
            println!("matched:    {}", m.rule);
            match m.target {
                Ok(i) => {
                    println!(
                        "target:     {} ({})",
                        ctx.targets[i].label,
                        ctx.targets[i].id()
                    );
                    println!(
                        "command:    {}",
                        shell_words(&ctx.targets[i].command(Some(url)))
                    );
                }
                Err(e) => println!("error:      {e}; the picker opens instead"),
            }
        }
        None => println!("matched:    no rule, the picker opens"),
    }
    Ok(())
}

fn print_history(history: &history::History, key: &str) {
    let now = history::now();
    let scores = history.scores(key, now);
    if scores.is_empty() {
        println!("history:    none");
    }
    for (i, (id, stat)) in scores.iter().enumerate() {
        println!(
            "{:12}{id}  score {:.1}  rank {:.1}",
            if i == 0 { "history:" } else { "" },
            stat.score(now),
            stat.rank
        );
    }
}

fn forget(ctx: &Ctx, site: Option<&str>, all: bool) -> Result<(), String> {
    let path = history::path(&ctx.paths);
    let mut history = history::History::load(&path);
    match site {
        Some(site) if !all => {
            let key = history::key_of(site);
            if !history.forget(&key) {
                println!("nothing remembered for {key:?}");
                return Ok(());
            }
            println!("forgot {key}");
        }
        _ => {
            history.clear();
            println!("forgot every site");
        }
    }
    history.save(&path)
}

/// Display-only quoting for `list` and `explain`; never executed.
fn shell_words(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if !a.is_empty()
                && a.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+,<>".contains(c))
            {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', r"'\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `packaging/man` is generated from the CLI; `URLMENU_BLESS=1 cargo test`
    /// rewrites it after a CLI change.
    #[test]
    fn packaged_man_pages_are_current() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packaging/man");
        let out = std::env::temp_dir().join(format!("urlmenu-man-{}", std::process::id()));
        std::fs::create_dir_all(&out).unwrap();
        clap_mangen::generate_to(Cli::command(), &out).unwrap();
        if std::env::var_os("URLMENU_BLESS").is_some() {
            std::fs::create_dir_all(&dir).unwrap();
            for entry in std::fs::read_dir(&dir).unwrap() {
                std::fs::remove_file(entry.unwrap().path()).unwrap();
            }
            for entry in std::fs::read_dir(&out).unwrap() {
                let path = entry.unwrap().path();
                std::fs::copy(&path, dir.join(path.file_name().unwrap())).unwrap();
            }
        }
        let names = |d: &std::path::Path| {
            let mut v: Vec<_> = std::fs::read_dir(d)
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            v.sort();
            v
        };
        assert_eq!(names(&out), names(&dir), "run URLMENU_BLESS=1 cargo test");
        for name in names(&out) {
            assert_eq!(
                std::fs::read(out.join(&name)).unwrap(),
                std::fs::read(dir.join(&name)).unwrap(),
                "{name:?} is stale; run URLMENU_BLESS=1 cargo test"
            );
        }
        std::fs::remove_dir_all(&out).unwrap();
    }
}
