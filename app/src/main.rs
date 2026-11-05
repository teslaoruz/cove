// Release builds on Windows are GUI apps (no console window).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod activity;
mod agents;
mod asr;
mod update;
mod chat;
mod desk;
mod info;
mod ipc;
mod mic;
mod orb;
mod launch;
mod live;
mod md;
mod platform;
mod router;
mod scan;
mod settings;
mod speech;

use launch::Mode;
use scan::Project;
use settings::{ScanDir, Settings};
use slint::{ComponentHandle, Image, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

slint::include_modules!();

const THEMES: [&str; 3] = ["system", "light", "dark"];
const GLASS: [&str; 4] = ["auto", "clear", "frosted", "solid"];

struct State {
    s: Settings,
    projects: Vec<Project>,
    colls: Vec<String>,
    vault_idx: Vec<(String, PathBuf)>,
    notes: HashMap<String, info::Notes>,
    art: serde_json::Value,
    images: HashMap<String, Option<Image>>,
    recent: Vec<scan::Session>,
    days: HashMap<String, u32>,
    agents: Vec<live::Agent>,
    status: Option<serde_json::Value>,
    slots: Vec<usize>,
    cur: Option<usize>,
    cur_actions: Vec<info::Action>,
    last_page: String,
    msgs: Vec<(String, bool)>,
    notice: Option<Box<dyn Fn()>>,
    choices: HashMap<&'static str, Vec<(String, String)>>, // setting key -> [(value, label)]
    sys_dark: bool,
    roots: Arc<Mutex<Vec<String>>>,
    act_started: bool, // the Activity page reads the logs once per run, on first visit
    prompts: HashMap<String, u64>, // prompts per day, once the activity scan is in
    chat_sid: Option<String>, // the hub's own Claude Code chat, while one is open
    brief: Option<desk::Brief>,
    desk: Option<Desk>,
    tween: slint::Timer, // eases the home scroll to a section
    mic: Option<mic::Mic>, // open only while listening
    speech: Option<speech::Speech>, // the piece of the reply being spoken
    clips: std::collections::VecDeque<speech::Clip>, // pieces synthesised and waiting their turn
    voice_until: Option<std::time::Instant>, // Handy pastes the transcript shortly after listening stops
    stopped: Option<std::time::Instant>, // when listening last stopped (Handy may still be transcribing)
    submit_timer: slint::Timer, // sends a pasted transcript once it stops changing
    key: Option<(std::time::Instant, bool)>, // listen key: last message and whether it is being held
    live: Option<asr::Live>, // live speech to text, while listening (Handy is the fallback)
    live_sent: usize, // sentences heard so far that were already sent (or skipped as the hub's own voice)
    live_base: String, // what the box held when listening started
    voice_msg: bool, // the next submit comes from speech
    talking: bool, // the last message was spoken, so its reply is read aloud
    chats_all: bool, // the sidebar lists every chat, not just the latest
    sys: Option<(f32, u64, u64)>, // CPU %, memory used, memory total
}

/// Desk sources read off the UI thread.
struct Desk {
    deadlines: Vec<desk::Deadline>,
    daily: Vec<String>,
    notifs: Vec<desk::Notice>,
    weather: Option<desk::Weather>,
    screen: Option<(u64, Vec<(String, u64)>)>,
    battery: Option<(u8, bool)>,
    disk: Option<(u64, u64)>, // (free, total) bytes of the disk holding the projects
}

fn desk_data(vault: &str, projects_dir: &str) -> Desk {
    let today = chrono::Local::now().date_naive();
    let disks = sysinfo::Disks::new_with_refreshed_list().ok();
    let disk = disks.iter().flat_map(|d| d.iter()).filter(|d| Path::new(projects_dir).starts_with(d.mount_point())).max_by_key(|d| d.mount_point().as_os_str().len()).map(|d| (d.available_space(), d.total_space()));
    Desk {
        deadlines: desk::deadlines(vault, today),
        daily: desk::daily_todos(vault, today),
        notifs: desk::notifications(6),
        weather: desk::weather(),
        screen: desk::screen_time(3),
        battery: desk::battery(),
        disk,
    }
}

fn dur(secs: u64) -> String {
    let (h, m) = (secs / 3600, secs % 3600 / 60);
    if h > 0 { format!("{h}h {m:02}m") } else { format!("{m}m") }
}

fn gb(b: u64) -> String {
    format!("{:.0} GB", b as f64 / 1e9)
}

/// System widget lines from the last CPU/memory poll and the desk's disk and battery.
fn show_stats(ui: &AppWindow) {
    with(|st| {
        let mut v = vec![];
        if let Some((cpu, used, total)) = st.sys {
            v.push(Stat { label: ss("CPU"), value: ss(format!("{cpu:.0}%")), frac: cpu / 100.0, warn: cpu > 85.0 });
            let f = used as f32 / total.max(1) as f32;
            v.push(Stat { label: ss("Memory"), value: ss(format!("{:.1} / {:.0} GB", used as f64 / 1e9, total as f64 / 1e9)), frac: f, warn: f > 0.9 });
        }
        if let Some(d) = &st.desk {
            if let Some((free, total)) = d.disk {
                let f = 1.0 - free as f32 / total.max(1) as f32;
                v.push(Stat { label: ss("Disk"), value: ss(format!("{} free", gb(free))), frac: f, warn: f > 0.92 });
            }
            if let Some((pct, charging)) = d.battery {
                v.push(Stat { label: ss(if charging { "Battery · charging" } else { "Battery" }), value: ss(format!("{pct}%")), frac: pct as f32 / 100.0, warn: pct < 15 && !charging });
            }
        }
        ui.set_sys_stats(model(v));
    });
}

thread_local! {
    static ST: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    ST.with(|s| f(s.borrow_mut().as_mut().expect("state set before the UI runs")))
}

fn ss(s: impl Into<SharedString>) -> SharedString {
    s.into()
}

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

// ---- building UI items ---------------------------------------------------

impl State {
    fn image(&mut self, root: &str) -> Option<Image> {
        if !self.images.contains_key(root) {
            let img = info::icon(root, &self.art).and_then(|p| Image::load_from_path(&p).ok());
            self.images.insert(root.to_string(), img);
        }
        self.images[root].clone()
    }

    fn notes(&mut self, root: &str) -> info::Notes {
        if !self.notes.contains_key(root) {
            let n = info::notes(root, &self.vault_idx);
            self.notes.insert(root.to_string(), n);
        }
        self.notes[root].clone()
    }

    fn hue(&self, p: &Project) -> i32 {
        self.colls.iter().position(|c| *c == p.collection).map(|i| if p.collection == "Home" { 8 } else if p.collection == "Other" { 7 } else { i as i32 }).unwrap_or(7)
    }

    fn state_of(&self, root: &str) -> String {
        self.agents.iter().find(|a| a.root == root).map(|a| a.state.clone()).unwrap_or_default()
    }

    fn item(&mut self, i: usize, slot: i32) -> ProjectItem {
        let p = self.projects[i].clone();
        let img = self.image(&p.root);
        ProjectItem {
            idx: i as i32,
            name: ss(&p.name),
            initials: ss(scan::initials(&p.name)),
            hue: self.hue(&p),
            has_icon: img.is_some(),
            icon: img.unwrap_or_default(),
            meta: ss(if p.sessions > 0 { format!("{} session{} · {}", p.sessions, if p.sessions == 1 { "" } else { "s" }, scan::ago(p.last)) } else { "No sessions yet".into() }),
            state: ss(self.state_of(&p.root)),
            slot,
            pinned: self.s.pinned.contains(&p.root),
        }
    }

    fn find(&self, root: &str) -> Option<usize> {
        self.projects.iter().position(|p| p.root == root)
    }
}

fn plural(n: usize, w: &str) -> String {
    format!("{n} {w}{}", if n == 1 { "" } else { "s" })
}

fn when(ts: f64) -> String {
    let t = chrono::DateTime::from_timestamp(ts as i64, 0).map(|t| t.with_timezone(&chrono::Local));
    let today = chrono::Local::now().date_naive();
    match t {
        Some(t) if t.date_naive() == today => t.format("%H:%M").to_string(),
        Some(t) => t.format("%a %H:%M").to_string(),
        None => String::new(),
    }
}

fn refresh_home(ui: &AppWindow) {
    refresh_head(ui);
    with(|st| {
        // facts (usage lives in the sidebar meters)
        let now = chrono::Local::now();
        let day = |d: i64| st.days.get(&(now - chrono::Duration::days(d)).format("%Y-%m-%d").to_string()).copied().unwrap_or(0);
        let week: u32 = (0..7).map(day).sum();
        let start = if day(0) > 0 { 0 } else { 1 }; // today may still be empty
        let streak = (start..3650).take_while(|d| day(*d) > 0).count();
        let facts = vec![
            Fact { title: ss(now.format("%A").to_string()), value: ss(now.format("%-d %B").to_string()), sub: ss(""), warn: false },
            Fact { title: ss("Today"), value: ss(plural(day(0) as usize, "session")), sub: ss(""), warn: false },
            Fact { title: ss("Last 7 days"), value: ss(plural(week as usize, "session")), sub: ss(""), warn: false },
            Fact { title: ss("Streak"), value: ss(if streak > 0 { plural(streak, "day") } else { "—".into() }), sub: ss(""), warn: false },
        ];
        ui.set_facts(model(facts));
        set_usage(ui, &st.status.clone().filter(|_| st.s.agent == "claude")); // the meters are Claude's limits
        let pin_idx: Vec<usize> = st.s.pinned.iter().filter_map(|r| st.find(r)).collect();
        let pins: Vec<ProjectItem> = pin_idx.into_iter().map(|i| st.item(i, 0)).collect();
        ui.set_pinned_items(model(pins));

        // recent: pinned first, then most recently used; Alt+1..8
        let mut order: Vec<usize> = st.s.pinned.iter().filter_map(|r| st.find(r)).collect();
        let mut used: Vec<usize> = (0..st.projects.len()).filter(|i| st.projects[*i].sessions > 0 && !order.contains(i)).collect();
        used.sort_by(|a, b| st.projects[*b].last.total_cmp(&st.projects[*a].last));
        order.extend(used);
        order.truncate(8);
        st.slots = order.clone();
        let recent: Vec<ProjectItem> = order.iter().enumerate().map(|(k, i)| st.item(*i, k as i32 + 1)).collect();
        ui.set_recent(model(recent));
        let mut groups = vec![];
        for c in st.colls.clone() {
            let items: Vec<ProjectItem> = (0..st.projects.len()).filter(|i| st.projects[*i].collection == c && !order.contains(i)).collect::<Vec<_>>().into_iter().map(|i| st.item(i, 0)).collect();
            if !items.is_empty() {
                groups.push(Group { title: ss(if c == "Home" { "Home folder".to_string() } else { c.clone() }), items: model(items) });
            }
        }
        ui.set_groups(model(groups));

        // pick up where you left off: the latest session of each of the last few projects
        let mut seen_roots = vec![];
        let mut chips = vec![];
        for sess in st.recent.clone() {
            if chips.len() >= 4 || seen_roots.contains(&sess.root) { continue; }
            if let Some(i) = st.find(&sess.root) {
                seen_roots.push(sess.root.clone());
                let t: String = sess.title.chars().take(48).collect();
                chips.push(RecentChip { item: st.item(i, 0), id: ss(&sess.id), title: ss(t), when: ss(scan::ago(sess.mtime)) });
            }
        }
        ui.set_recent_chips(model(chips));
        if let Some(sess) = st.recent.first().cloned() {
            if let Some(i) = st.find(&sess.root) {
                ui.set_has_cont(true);
                ui.set_cont(st.item(i, 0));
                let q: String = sess.title.chars().take(140).collect();
                ui.set_cont_quote(ss(format!("“{q}”")));
                ui.set_cont_when(ss(format!("{} · {}", scan::ago(sess.mtime), when(sess.mtime))));
            }
        } else {
            ui.set_has_cont(false);
        }

        // sidebar
        let colls: Vec<Coll> = st.colls.iter().filter_map(|c| {
            let n = st.projects.iter().filter(|p| p.collection == *c).count();
            (n > 0).then(|| Coll { name: ss(if c == "Home" { "Home folder" } else { c }), count: n as i32, hue: if c == "Home" { 8 } else if c == "Other" { 7 } else { st.colls.iter().position(|x| x == c).unwrap_or(7) as i32 } })
        }).collect();
        ui.set_colls(model(colls));
        ui.set_project_count(st.projects.len() as i32);
        ui.set_agent_label(ss(launch::AGENTS.iter().find(|a| a.0 == st.s.agent).map(|a| a.1).unwrap_or("Claude Code")));
        ui.set_agent_tint(agent_color(&st.s.agent));
    });
    refresh_agents(ui);
}

fn refresh_agents(ui: &AppWindow) {
    with(|st| {
        let items: Vec<AgentItem> = st.agents.clone().iter().filter_map(|a| {
            let i = st.find(&a.root)?;
            let it = st.item(i, 0);
            Some(AgentItem {
                idx: i as i32,
                name: it.name,
                initials: it.initials,
                hue: it.hue,
                icon: it.icon,
                has_icon: it.has_icon,
                tool: ss(launch::AGENTS.iter().find(|t| t.0 == a.tool).map(|t| t.1).unwrap_or(&a.tool)),
                tint: agent_color(&a.tool),
                state: ss(&a.state),
                wid: ss(a.window.clone().unwrap_or_default()),
            })
        }).collect();
        ui.set_agents(model(items));
    });
}

fn show_list(ui: &AppWindow, title: &str, filter: impl Fn(&Project) -> bool, grouped: bool) {
    with(|st| {
        let idx: Vec<usize> = (0..st.projects.len()).filter(|i| filter(&st.projects[*i])).collect();
        let mut groups = vec![];
        if grouped {
            for c in st.colls.clone() {
                let items: Vec<ProjectItem> = idx.iter().filter(|i| st.projects[**i].collection == c).copied().collect::<Vec<_>>().into_iter().map(|i| st.item(i, 0)).collect();
                if !items.is_empty() {
                    groups.push(Group { title: ss(if c == "Home" { "Home folder".into() } else { c.clone() }), items: model(items) });
                }
            }
        } else if !idx.is_empty() {
            let items = idx.into_iter().map(|i| st.item(i, 0)).collect();
            groups.push(Group { title: ss(""), items: model(items) });
        }
        ui.set_list_groups(model(groups));
        ui.set_list_title(ss(title));
    });
    ui.set_page(ss("list"));
}

fn search(ui: &AppWindow, q: &str) {
    if ui.get_mode() == "chat" {
        return; // chat mode: the box only talks
    }
    let q = q.trim().to_lowercase();
    if ui.get_page() == "home" && ui.get_dock_raw() < 1.0 {
        // the desk shows matches under its prompt instead of switching pages
        let rows: Vec<DeskRow> = matches(&q).into_iter().take(5).map(|i| with(|st| { let it = st.item(i, 0); row(it.name.as_str(), it.meta.as_str(), "", false, i as i32, "") })).collect();
        ui.set_suggest(model(rows));
        return;
    }
    if q.is_empty() {
        if ui.get_page() == "list" && ui.get_searching() {
            let back = with(|st| st.last_page.clone());
            go(ui, &back, "");
        }
        return;
    }
    ui.set_searching(true);
    ui.set_nav(ss(""));
    show_list(ui, "Results", |p| p.name.to_lowercase().contains(&q) || p.root.to_lowercase().contains(&q), false);
}

fn matches(q: &str) -> Vec<usize> {
    let q = q.trim().to_lowercase();
    with(|st| (0..st.projects.len()).filter(|i| !q.is_empty() && (st.projects[*i].name.to_lowercase().contains(&q) || st.projects[*i].root.to_lowercase().contains(&q))).collect())
}

fn go(ui: &AppWindow, page: &str, arg: &str) {
    let page = if page == "back" { with(|st| st.last_page.clone()) } else { page.to_string() };
    ui.set_searching(false);
    match page.as_str() {
        "list" => {
            show_list(ui, "All projects", |_| true, true);
            ui.set_nav(ss("list"));
        }
        "coll" => {
            let c = if arg == "Home folder" { "Home".to_string() } else { arg.to_string() };
            show_list(ui, arg, |p| p.collection == c, false);
            ui.set_nav(ss(format!("coll:{arg}")));
        }
        "activity" => {
            ui.set_page(ss("home"));
            ui.set_nav(ss("home"));
            ui.invoke_scroll_section(ss("activity"));
        }
        "chat" => show_chat(ui),
        "settings" => {
            refresh_settings(ui);
            ui.set_page(ss("settings"));
            ui.set_nav(ss("settings"));
        }
        _ => {
            ui.set_page(ss("home"));
            ui.set_nav(ss("home"));
        }
    }
    let p = if page == "coll" { format!("coll:{arg}") } else { page.clone() };
    with(|st| st.last_page = p.split(':').next().unwrap_or("home").to_string());
    if page == "coll" {
        with(|st| st.last_page = "home".into()); // Back from a project opened in a collection returns home
    }
}

fn show_project(ui: &AppWindow, i: usize) {
    let (root, item, notes, coll, facts, actions) = with(|st| {
        let p = st.projects[i].clone();
        st.cur = Some(i);
        st.cur_actions = info::actions(&p.root);
        let facts = if p.sessions > 0 { format!("{}, last {}", plural(p.sessions, "session"), scan::ago(p.last)) } else { "No sessions yet".into() };
        (p.root.clone(), st.item(i, 0), st.notes(&p.root), if p.collection == "Home" { "Home folder".into() } else { p.collection.clone() }, facts, st.cur_actions.iter().map(|a| ss(&a.label)).collect::<Vec<_>>())
    });
    ui.set_proj(item);
    ui.set_proj_coll(ss(coll));
    ui.set_proj_facts(ss(facts));
    ui.set_proj_git(ss(""));
    ui.set_proj_desc(ss(&notes.desc));
    ui.set_proj_actions(model(actions));
    ui.set_proj_todo_head(ss(format!("To do · {} open{}", notes.todo.len(), if notes.done > 0 { format!(", {} done", notes.done) } else { String::new() })));
    ui.set_proj_todos(model(notes.todo.iter().take(10).map(ss).collect()));
    ui.set_proj_github(launch::github_url(&root).is_some());
    ui.set_proj_notes(notes.dir.is_some());
    ui.set_proj_sessions(model(vec![]));
    ui.set_proj_loading(true);
    ui.set_page(ss("project"));
    ui.set_nav(ss(""));
    let p = with(|st| st.projects[i].clone());
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let sessions = scan::project_sessions(&p, 60);
        let git = info::git(&p.root);
        let rows: Vec<(String, String, String)> = sessions.iter().map(|s| (s.id.clone(), s.title.chars().take(160).collect(), format!("{} · {}", scan::ago(s.mtime), when(s.mtime)))).collect();
        let git = git.map(|g| format!("{} · {}{}", g.branch, if g.changed > 0 { format!("{} changed", g.changed) } else { "clean".into() }, if g.ahead > 0 { format!(", {} unpushed", g.ahead) } else { String::new() }));
        let root = p.root.clone();
        let _ = weak.upgrade_in_event_loop(move |ui| {
            if ui.get_page() != "project" || ui.get_proj().idx as usize != i || with(|st| st.projects.get(i).map(|p| p.root != root).unwrap_or(true)) {
                return;
            }
            ui.set_proj_git(ss(git.unwrap_or_default()));
            ui.set_proj_sessions(model(rows.into_iter().map(|(id, t, sub)| SessionItem { id: ss(id), title: ss(t), sub: ss(sub) }).collect()));
            ui.set_proj_loading(false);
        });
    });
}

fn refresh_settings(ui: &AppWindow) {
    // Build everything inside the state borrow, set widgets after it: a widget callback must never find the state borrowed.
    let own = |v: &[(&str, &str)]| v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>();
    let s = with(|st| st.s.clone());
    let terms = launch::installed_terminals();
    let term_cur = if s.terminal.is_empty() { terms.first().map(|t| t.0).unwrap_or("").to_string() } else { s.terminal.clone() };
    let lists: Vec<(&'static str, Vec<(String, String)>, String)> = vec![
        // installed agents, plus the chosen one even when it is not
        ("agent", own(&launch::AGENTS.iter().copied().filter(|a| a.0 == s.agent || launch::which(a.0).is_some()).collect::<Vec<_>>()), s.agent.clone()),
        ("terminal", own(&terms), term_cur),
        ("editor", own(&launch::installed(launch::EDITORS)), s.editor.clone()),
        ("browser", own(&launch::installed(launch::BROWSERS)), s.browser.clone()),
        ("files", own(&launch::installed(launch::FILE_MANAGERS)), s.files.clone()),
        ("engine", engines(), s.voice_engine.clone()),
        ("voice", own(speech::KOKORO_VOICES), s.voice.clone()),
        ("speed", own(SPEEDS), SPEEDS.iter().min_by(|a, b| (a.0.parse::<f32>().unwrap_or(1.0) - s.voice_speed).abs().total_cmp(&(b.0.parse::<f32>().unwrap_or(1.0) - s.voice_speed).abs())).map(|x| x.0.to_string()).unwrap_or_default()),
    ];
    let mut out: Vec<(ModelRc<SharedString>, i32)> = vec![];
    with(|st| {
        for (key, mut list, cur) in lists {
            if !list.iter().any(|(v, _)| *v == cur) {
                list.push((cur.clone(), if cur.is_empty() { "System default".into() } else { cur.clone() }));
            }
            let idx = list.iter().position(|(v, _)| *v == cur).unwrap_or(0) as i32;
            out.push((model(list.iter().map(|(_, l)| ss(l)).collect()), idx));
            st.choices.insert(key, list);
        }
    });
    let mut it = out.into_iter();
    let (m, i) = it.next().unwrap(); ui.set_agent_names(m); ui.set_agent_index(i);
    ui.set_agent_colors(model(with(|st| st.choices.get("agent").map(|l| l.iter().map(|(k, _)| agent_color(k)).collect()).unwrap_or_default())));
    ui.set_local_model(ss(&s.local_model));
    let (m, i) = it.next().unwrap(); ui.set_term_names(m); ui.set_term_index(i);
    let (m, i) = it.next().unwrap(); ui.set_editor_names(m); ui.set_editor_index(i);
    let (m, i) = it.next().unwrap(); ui.set_browser_names(m); ui.set_browser_index(i);
    let (m, i) = it.next().unwrap(); ui.set_files_names(m); ui.set_files_index(i);
    let (m, i) = it.next().unwrap(); ui.set_engine_names(m); ui.set_engine_index(i);
    let (m, i) = it.next().unwrap(); ui.set_voice_names(m); ui.set_voice_index(i);
    let (m, i) = it.next().unwrap(); ui.set_speed_names(m); ui.set_speed_index(i);
    let kokoro = speech::kokoro().is_some();
    ui.set_voice_enabled(kokoro && (s.voice_engine == "kokoro" || s.voice_engine == "auto"));
    ui.set_talkback(s.talkback);
    ui.set_auto_send(s.auto_send);
    ui.set_auto_update(s.auto_update);
    ui.set_mail_on(s.mail);
    ui.set_update_note(ss(&if update::managed() {
        format!("Version {}. New versions come from GitHub and install in the background.", update::VERSION)
    } else {
        format!("Version {}. This copy was not installed with the installer, so it does not update itself.", update::VERSION)
    }));
    ui.set_chat_folder(ss(&s.chat_dir));
    ui.set_voice_note(ss(&voice_note(kokoro)));
    ui.set_theme_index(THEMES.iter().position(|t| *t == s.theme).unwrap_or(0) as i32);
    ui.set_glass_index(GLASS.iter().position(|t| *t == s.glass).unwrap_or(0) as i32);
    ui.set_vault(ss(&s.vault));
    ui.set_scan_dirs(model(s.scan_dirs.iter().map(|d| ScanItem { path: ss(&d.path), collection: ss(&d.collection) }).collect()));
    ui.set_hidden(model(s.hidden.iter().map(ss).collect()));
    ui.set_platform_note(ss(platform::glass_note()));
}

const SPEEDS: &[(&str, &str)] = &[("0.85", "Slower"), ("1", "Normal"), ("1.15", "Faster"), ("1.3", "Fastest")];

/// Voice engines for Settings, each marked when it is not installed (the choice then falls back to espeak-ng).
fn engines() -> Vec<(String, String)> {
    let mark = |ok: bool, l: &str| if ok { l.to_string() } else { format!("{l} (not installed)") };
    let piper = (launch::which("piper-tts").is_some() || launch::which("piper").is_some()) && speech::piper_voice().is_some();
    vec![
        ("auto".into(), "Best installed".into()),
        ("kokoro".into(), mark(speech::kokoro().is_some(), "Kokoro (natural)")),
        ("piper".into(), mark(piper, "Piper")),
        ("espeak".into(), mark(launch::which("espeak-ng").is_some(), "espeak-ng (robotic)")),
    ]
}

/// Which voice input and output this machine has, for the Settings caption.
fn voice_note(kokoro: bool) -> String {
    let model = dirs::data_dir()
        .and_then(|d| std::fs::read_to_string(d.join("com.pais.handy/settings_store.json")).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["settings"]["selected_model"].as_str().map(str::to_string));
    let input = match (launch::which("handy").is_some(), model) {
        _ if asr::installed().is_some() => "Voice: press the Copilot key with Cove in front to talk; words appear as you speak, each pause sends, replies are read aloud, and it keeps listening until you press the key again. When Cove is not in front, the key only brings it up. Speech to text: Parakeet in ~/.local/share/asr.".to_string(),
        (false, _) => "Voice input needs Handy (handy.computer).".to_string(),
        (true, m) => format!("Voice input: Handy{}. Press the Copilot key to talk; press it again to stop. It brings Cove to the front.", m.map(|m| format!(" with {m}")).unwrap_or_default()),
    };
    let output = if kokoro { " Kokoro lives in ~/.local/share/kokoro." } else { " For a natural voice, put sherpa-onnx and a Kokoro model in ~/.local/share/kokoro." };
    format!("{input}{output} Changing the chat folder starts a fresh chat list; older chats stay in the old folder.")
}

fn save(st: &State) {
    if let Err(e) = st.s.save() {
        eprintln!("settings not saved: {e}");
    }
}

// ---- theme -----------------------------------------------------------------

fn apply_theme(ui: &AppWindow) {
    let (pref, glass, sys_dark) = with(|st| (st.s.theme.clone(), st.s.glass.clone(), st.sys_dark));
    let dark = match pref.as_str() { "light" => false, "dark" => true, _ => sys_dark };
    let t = ui.global::<Theme>();
    t.set_dark(dark);
    t.set_alpha(platform::glass_alpha(&glass));
    let mut acc = platform::accent().unwrap_or((10, 132, 255));
    let ground = if dark { (36, 36, 42) } else { (244, 244, 248) };
    if !dark && platform::luminance(acc) > 0.25 {
        acc = platform::mix(acc, (0, 0, 0), 0.35);
    }
    let on = if platform::contrast(acc, (255, 255, 255)) >= platform::contrast(acc, (17, 17, 20)) { (255, 255, 255) } else { (17, 17, 20) };
    let mut text = acc;
    for _ in 0..8 {
        if platform::contrast(text, ground) >= 4.5 {
            break;
        }
        text = platform::mix(text, if dark { (255, 255, 255) } else { (0, 0, 0) }, 0.2);
    }
    let c = |(r, g, b): (u8, u8, u8)| slint::Color::from_rgb_u8(r, g, b);
    t.set_accent(c(acc));
    t.set_on_accent(c(on));
    t.set_accent_text(c(text));
    #[cfg(any(windows, target_os = "macos"))]
    apply_vibrancy(ui, dark, &glass);
}

#[cfg(any(windows, target_os = "macos"))]
fn apply_vibrancy(ui: &AppWindow, dark: bool, glass: &str) {
    use slint::winit_030::WinitWindowAccessor;
    if glass == "solid" {
        return;
    }
    ui.window().with_winit_window(|w| {
        #[cfg(windows)]
        let _ = window_vibrancy::apply_mica(w, Some(dark));
        #[cfg(target_os = "macos")]
        let _ = window_vibrancy::apply_vibrancy(w, window_vibrancy::NSVisualEffectMaterial::UnderWindowBackground, None, None);
        let _ = dark;
    });
}

// ---- actions ---------------------------------------------------------------

fn report(ui: &AppWindow, r: std::io::Result<()>, what: &str) {
    if let Err(e) = r {
        say(ui, &format!("Could not open {what}: {e}. Check Settings."));
    }
}

fn say(ui: &AppWindow, text: &str) {
    with(|st| {
        st.msgs.push((text.to_string(), false));
        let n = st.msgs.len();
        if n > 200 {
            st.msgs.drain(..n - 200);
        }
    });
    show_msgs(ui);
    ui.set_reply_open(true);
}

/// Last few messages for the desk and other pages; the whole conversation for the chat page, scrolled to its end.
fn show_msgs(ui: &AppWindow) {
    with(|st| {
        ui.set_msgs(model(st.msgs.iter().rev().take(8).rev().map(|(t, u)| msg(t, *u)).collect()));
        ui.set_chat_msgs(model(st.msgs.iter().map(|(t, u)| msg(t, *u)).collect()));
    });
    let w = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(30), move || { if let Some(ui) = w.upgrade() { ui.invoke_scroll_chat_end() } });
}

/// A message for the UI: replies split into markdown blocks; what the user typed stays plain.
fn msg(text: &str, user: bool) -> Msg {
    let block = |kind: &str, text: &str, rich: slint::StyledText| Block { kind: ss(kind), text: ss(text), rich };
    let blocks: Vec<Block> = if user {
        text.lines().map(|l| block("p", l, slint::StyledText::from_plain_text(l))).collect() // StyledText draws no \n in plain text
    } else {
        md::blocks(text)
            .into_iter()
            .map(|(k, t)| {
                let rich = match k {
                    md::Kind::Prose | md::Kind::Quote => md::rich(&t),
                    md::Kind::Heading => md::rich(&format!("**{t}**")),
                    _ => slint::StyledText::default(),
                };
                block(k.name(), &t, rich)
            })
            .collect()
    };
    Msg { text: ss(text), user, blocks: model(blocks) }
}

/// Handy's remote commands, in order, off the UI thread (each hands its message to the running Handy).
/// A command run while Handy is not running becomes Handy itself and never exits, so Handy is started
/// first, and no command is waited on for more than 5 s.
fn handy(cmds: &[&'static str]) {
    use std::process::{Command, Stdio};
    let cmds = cmds.to_vec();
    std::thread::spawn(move || {
        let running = sysinfo::System::new().ok().is_none_or(|mut sys| {
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            sys.processes().values().any(|p| p.name() == "handy")
        });
        if !running {
            let _ = Command::new("handy").arg("--start-hidden").stdout(Stdio::null()).stderr(Stdio::null()).spawn();
            std::thread::sleep(Duration::from_secs(3));
        }
        for c in cmds {
            let Ok(mut ch) = Command::new("handy").arg(c).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else { return };
            for _ in 0..50 {
                if !matches!(ch.try_wait(), Ok(None)) { break; }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
}

fn voice(st: &State) -> speech::Voice {
    speech::Voice { engine: st.s.voice_engine.clone(), voice: st.s.voice.clone(), speed: st.s.voice_speed }
}

fn say_user(text: &str) {
    with(|st| st.msgs.push((text.to_string(), true)));
}

/// The chat page: conversation, bar at the bottom.
fn show_chat(ui: &AppWindow) {
    ui.set_mode(ss("chat"));
    ui.set_page(ss("chat"));
    ui.set_nav(ss("chat"));
    show_msgs(ui);
    ui.invoke_focus_input();
}

fn launch_project(ui: &AppWindow, i: usize, mode: Mode) {
    let (s, root) = with(|st| (st.s.clone(), st.projects[i].root.clone()));
    report(ui, launch::agent(&s, &root, mode), "the terminal");
}

fn run_action(ui: &AppWindow, root: &str, a: &info::Action) {
    let s = with(|st| st.s.clone());
    let cwd = Path::new(root).join(&a.cwd);
    if let Some(f) = &a.file {
        return report(ui, launch::system_open(&cwd.join(f).to_string_lossy()), &a.label);
    }
    let missing: Vec<&String> = a.needs.iter().filter(|n| !Path::new(root).join(n).exists()).collect();
    if !missing.is_empty() {
        let prompt = format!("Restore what '{}' needs in this project (read REINSTALL.md; tell me the download size before downloading), then start it.", a.label);
        let root = root.to_string();
        ui.set_notice_title(ss(format!("{} isn't set up here", a.label)));
        ui.set_notice_text(ss("Your coding agent can install what it needs. It tells you the download size first."));
        ui.set_notice_action(ss("Set it up"));
        let weak = ui.as_weak();
        with(|st| st.notice = Some(Box::new(move || {
            if let Some(ui) = weak.upgrade() {
                let s = with(|st| st.s.clone());
                report(&ui, launch::agent(&s, &root, Mode::New(Some(&prompt))), "the terminal");
            }
        })));
        ui.set_notice_open(true);
        return;
    }
    let Some(cmd) = &a.cmd else { return };
    report(ui, launch::terminal(&s, &cwd.to_string_lossy(), Some(cmd)), &a.label);
    if let Some(url) = a.url.clone() {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            let _ = launch::browser(&s, &url);
        });
    }
}

fn ask(ui: &AppWindow, text: &str) {
    // quick capture into the vault inbox: "note: …", "todo: …"
    let lower = text.to_lowercase();
    for (pre, todo) in [("note:", false), ("todo:", true), ("idea:", false)] {
        if let Some(rest) = lower.strip_prefix(pre).map(|_| text[pre.len()..].trim()) {
            let vault = with(|st| st.s.vault.clone());
            if vault.is_empty() {
                return say(ui, "Set a notes vault in Settings to keep notes.");
            }
            match desk::capture(&vault, rest, todo) {
                Ok(_) => { say(ui, &format!("Saved to the vault inbox{}.", if todo { " as a to-do" } else { "" })); refresh_desk(ui); }
                Err(e) => say(ui, &format!("Could not save it: {e}")),
            }
            return;
        }
    }
    say_user(text);
    if ui.get_mode() == "chat" {
        return chat_send(ui, text);
    }
    let projects = with(|st| st.projects.clone());
    let chatting = with(|st| st.chat_sid.is_some());
    match router::route(text, &projects) {
        // mid-chat, "make this a project" takes the chat with it
        Some(router::Route::New { name }) if chatting => {
            open_new(ui, &name, "");
            ui.set_new_from_chat(true);
        }
        // mid-chat, naming a project is conversation unless it asks to go there
        Some(router::Route::Project { .. }) if chatting && !router::explicit_open(text) => chat_send(ui, text),
        // a project's name alone (or with a word or two) is not a task: talk instead of starting an agent there
        Some(router::Route::Project { run: false, status: false, .. }) if text.split_whitespace().count() <= 3 && !router::explicit_open(text) => chat_send(ui, text),
        Some(router::Route::New { name }) => {
            say(ui, "Where should it live?");
            open_new(ui, &name, text);
        }
        Some(router::Route::Project { idx, run, status }) => act_on(ui, idx, run, status, text),
        None if chatting => chat_send(ui, text),
        None => {
            say(ui, "Thinking…");
            let cat: Vec<(String, String)> = with(|st| st.projects.clone().iter().map(|p| (p.root.clone(), st.notes(&p.root).desc)).collect());
            let weak = ui.as_weak();
            let text = text.to_string();
            std::thread::spawn(move || {
                let j = router::llm_route(&text, &cat);
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    // model output is untrusted: only known projects, and a dialog for anything that creates
                    let j = j.unwrap_or_default();
                    let act = j["action"].as_str().unwrap_or("").to_string();
                    let prompt = j["prompt"].as_str().filter(|p| !p.is_empty()).unwrap_or(&text).to_string();
                    match act.as_str() {
                        "new" => {
                            say(&ui, "Where should it live?");
                            open_new(&ui, &j["name"].as_str().unwrap_or("").chars().take(60).collect::<String>(), &prompt);
                        }
                        "open" => match j["project"].as_str().and_then(|r| with(|st| st.find(r))) {
                            Some(i) => act_on(&ui, i, false, false, &prompt),
                            None => say(&ui, "Which project? Try its name."),
                        },
                        _ => { unthink(&ui); chat_send(&ui, &text) } // a question or an idea: talk it through
                    }
                });
            });
        }
    }
}

fn act_on(ui: &AppWindow, i: usize, run: bool, status: bool, text: &str) {
    let (p, notes, acts) = with(|st| { let p = st.projects[i].clone(); let n = st.notes(&p.root); (p.clone(), n, info::actions(&p.root)) });
    if run && !acts.is_empty() {
        let t = text.to_lowercase();
        let a = acts.iter().find(|a| a.label.to_lowercase().split_whitespace().any(|w| w.len() > 3 && t.contains(w))).unwrap_or(&acts[0]);
        say(ui, &format!("Starting {}…", a.label));
        return run_action(ui, &p.root, a);
    }
    if status && (!notes.todo.is_empty() || !notes.desc.is_empty()) {
        let mut m = format!("{}: {}", p.name, notes.desc.chars().take(200).collect::<String>());
        if !notes.todo.is_empty() {
            m += &format!("\n{} open, {} done. Next: {}", notes.todo.len(), notes.done, notes.todo.iter().take(3).cloned().collect::<Vec<_>>().join("; "));
        }
        say(ui, &m);
    }
    let t = text.to_lowercase();
    let cont = p.sessions > 0 && router::wants_continue(&t);
    say(ui, &format!("{} {}.", if cont { "Continuing" } else { "Opening" }, p.name));
    launch_project(ui, i, if cont { Mode::ContinueWith(text) } else { Mode::New(Some(text)) });
}

fn open_new(ui: &AppWindow, name: &str, prompt: &str) {
    ui.set_new_from_chat(false); // only "Make it a project" moves the chat; it sets this after
    let loc = with(|st| st.s.scan_dirs.first().map(|d| d.path.clone()).unwrap_or_else(scan::home));
    ui.set_new_name(ss(name.split_whitespace().collect::<Vec<_>>().join("-")));
    ui.set_new_location(ss(loc));
    ui.set_new_prompt(ss(prompt));
    ui.set_new_error(ss(""));
    ui.set_new_open(true);
}

fn create_project(ui: &AppWindow, name: &str, base: &str, prompt: &str) {
    let name = name.trim();
    if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
        return ui.set_new_error(ss("The name is empty or contains a slash."));
    }
    if !Path::new(base.trim()).is_absolute() {
        return ui.set_new_error(ss("The location must be a full path."));
    }
    let path = Path::new(base.trim()).join(name);
    if let Err(e) = std::fs::create_dir_all(&path) {
        return ui.set_new_error(ss(format!("Could not create it: {e}")));
    }
    let path = path.to_string_lossy().into_owned();
    if scan::place(&path, &with(|st| st.s.clone())).is_none() {
        let _ = scan::add_extra(&path);
    }
    ui.set_new_open(false);
    let s = with(|st| st.s.clone());
    if ui.get_new_from_chat() {
        if let Some(sid) = with(|st| st.chat_sid.take()) {
            // the idea chat moves into the new folder and continues there
            match chat::move_session(&sid, &path) {
                Ok(()) => {
                    report(ui, launch::terminal(&s, &path, Some(&launch::agent_cmd("claude", &Mode::Resume(&sid)))), "the terminal");
                    clear_chat(ui);
                    say(ui, &format!("Moved the chat into {name} and opened it there."));
                }
                Err(e) => { with(|st| st.chat_sid = Some(sid)); say(ui, &format!("Created {name}, but could not move the chat: {e}")); }
            }
            return rescan(ui);
        }
    }
    let p = prompt.trim();
    report(ui, launch::agent(&s, &path, Mode::New(if p.is_empty() { None } else { Some(p) })), "the terminal");
    rescan(ui);
}

fn tool(ui: &AppWindow, kind: &str, i: usize) {
    let (s, p, notes) = with(|st| { let p = st.projects[i].clone(); let n = st.notes(&p.root); (st.s.clone(), p, n) });
    match kind {
        "editor" => report(ui, launch::editor(&s, &p.root), "the editor"),
        "terminal" => report(ui, launch::terminal(&s, &p.root, None), "the terminal"),
        "files" => report(ui, launch::files(&s, &p.root), "the file manager"),
        "github" => match launch::github_url(&p.root) {
            Some(u) => report(ui, launch::browser(&s, &u), "the browser"),
            None => say(ui, &format!("{} has no GitHub remote.", p.name)),
        },
        "notes" => if let Some(d) = notes.dir { report(ui, launch::editor(&s, &d.to_string_lossy()), "the notes") },
        "copy" => if !platform::copy(&p.root) { say(ui, "Could not reach the clipboard (install wl-clipboard or xclip).") },
        "pin" => {
            with(|st| {
                if let Some(k) = st.s.pinned.iter().position(|r| *r == p.root) { st.s.pinned.remove(k); } else { st.s.pinned.insert(0, p.root.clone()); }
                save(st);
            });
            refresh_home(ui);
        }
        "hide" => {
            with(|st| { st.s.hidden.push(p.root.clone()); save(st); });
            rescan(ui);
            say(ui, &format!("Hid {}. Show it again in Settings.", p.name));
        }
        _ => {}
    }
}

// ---- data loading ---------------------------------------------------------------

fn rescan(ui: &AppWindow) {
    let s = with(|st| st.s.clone());
    let projects = scan::collect(&s);
    let vault_idx = info::vault_index(&s);
    with(|st| {
        st.colls = scan::collections(&st.s);
        *st.roots.lock().unwrap() = projects.iter().map(|p| p.root.clone()).collect();
        st.projects = projects;
        st.vault_idx = vault_idx;
        st.notes.clear();
        st.art = info::art();
        st.images.clear();
    });
    refresh_home(ui);
    let page = ui.get_page().to_string();
    if page == "list" || page == "settings" {
        let nav = ui.get_nav().to_string();
        if page == "settings" { go(ui, "settings", ""); } else if let Some(c) = nav.strip_prefix("coll:") { go(ui, "coll", c); } else { go(ui, "list", ""); }
    }
    let weak = ui.as_weak();
    let projects = with(|st| st.projects.clone());
    std::thread::spawn(move || {
        let recent = scan::recent_sessions(&projects, 30);
        let days = scan::day_counts(&projects);
        let _ = weak.upgrade_in_event_loop(move |ui| {
            with(|st| { st.recent = recent; st.days = days; });
            refresh_home(&ui);
            ui.set_loaded(true);
            load_activity(&ui);
            refresh_desk(&ui);
            refresh_chats(&ui);
        });
    });
}

fn window_focused(ui: &AppWindow) -> bool {
    use slint::winit_030::WinitWindowAccessor;
    ui.window().with_winit_window(|w| w.has_focus()).unwrap_or(true)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let listen_now = args.iter().any(|a| a == "--listen");
    // a plain launch or the listen shortcut goes to the hub that is already open
    if (args.len() == 1 || (listen_now && args.len() == 2)) && ipc::send(if listen_now { "listen" } else { "show" }) {
        return;
    }
    if args.iter().any(|a| a == "--mic-test") {
        // print the microphone level for 4 s, to check the orb's voice input
        let Some(m) = mic::Mic::open() else { return eprintln!("no microphone") };
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(100));
            println!("{:.3}", m.level());
        }
        return;
    }
    let settings = Settings::load();
    chat::set_dir(&settings.chat_dir);
    chat::set_agent(&settings.agent, &settings.local_model);
    if std::env::args().any(|a| a == "--list") {
        // print the projects the app finds, for checking the scan settings
        for p in scan::collect(&settings) {
            println!("{}\t{}\t{}\t{}", p.collection, p.sessions, scan::ago(p.last), p.root);
        }
        return;
    }
    if std::env::args().any(|a| a == "--live") {
        // print running agents and the window each would jump to
        let roots: Vec<String> = scan::collect(&settings).into_iter().map(|p| p.root).collect();
        if let Some(mut w) = live::Watcher::new() {
            for a in w.scan(&roots, &scan::home()).0 {
                println!("{}\t{}\t{}\t{}\t{}", a.state, a.tool, a.pid, a.window.unwrap_or_default(), a.root);
            }
        }
        return;
    }
    // Draw on the integrated GPU (Mesa) when there is one: a launcher should not wake a discrete NVIDIA GPU
    // (battery) and the NVIDIA EGL driver costs ~35 MB more. Must be set before the window's EGL starts.
    #[cfg(all(unix, not(target_os = "macos")))]
    if std::env::var_os("__EGL_VENDOR_LIBRARY_FILENAMES").is_none() {
        let mesa = "/usr/share/glvnd/egl_vendor.d/50_mesa.json";
        if Path::new(mesa).exists() {
            unsafe { std::env::set_var("__EGL_VENDOR_LIBRARY_FILENAMES", mesa) }; // single-threaded here: no other thread reads env yet
        }
    }
    // app id = the .desktop file name, so launchers and docks match the window to its entry and icon
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use slint::winit_030::winit::platform::{wayland::WindowAttributesExtWayland, x11::WindowAttributesExtX11};
        let _ = slint::BackendSelector::new()
            .with_winit_window_attributes_hook(|a| {
                let a = WindowAttributesExtWayland::with_name(a, "cove", "");
                WindowAttributesExtX11::with_name(a, "cove", "cove")
            })
            .select();
    }
    let ui = AppWindow::new().expect("window");
    UI.with(|u| *u.borrow_mut() = Some(ui.as_weak()));
    let brief = if settings.mail { desk::cached_brief() } else { None };
    let roots = Arc::new(Mutex::new(vec![]));
    ST.with(|s| *s.borrow_mut() = Some(State {
        s: settings, projects: vec![], colls: vec![], vault_idx: vec![], notes: HashMap::new(), art: serde_json::Value::Null,
        images: HashMap::new(), recent: vec![], days: HashMap::new(), agents: vec![], status: None, slots: vec![], cur: None,
        cur_actions: vec![], last_page: "home".into(), msgs: vec![], notice: None, choices: HashMap::new(),
        sys_dark: platform::system_dark().unwrap_or(true), roots: roots.clone(), act_started: false, prompts: HashMap::new(), chat_sid: None, brief, desk: None, tween: slint::Timer::default(), mic: None, sys: None, speech: None, clips: Default::default(), voice_until: None, stopped: None, submit_timer: slint::Timer::default(), key: None, live: None, live_sent: 0, live_base: String::new(), voice_msg: false, talking: false, chats_all: false,
    }));
    apply_theme(&ui);
    ui.global::<Theme>().set_mono(ss(&platform::mono_font()));
    ui.set_side_collapsed(with(|st| st.s.sidebar_collapsed));
    ui.set_side_w(with(|st| st.s.sidebar_width).clamp(180.0, 420.0));
    show_mail(&ui);
    if with(|st| st.brief.as_ref().is_none_or(|b| chrono::Local::now().timestamp() - b.fetched > 3 * 3600)) { // the Refresh link fetches any time
        fetch_mail(&ui);
    }
    rescan(&ui);
    update::clean_up();
    let w_up = ui.as_weak();
    update::watch(
        || Settings::load().auto_update,
        move |tag| { let _ = w_up.upgrade_in_event_loop(move |ui| say(&ui, &format!("Cove {tag} is installed. Restart Cove to use it."))); },
    );
    refresh_settings(&ui); // fills the sidebar's agent picker too
    let w_ipc = ui.as_weak();
    ipc::serve(move |msg| {
        let _ = w_ipc.upgrade_in_event_loop(move |ui| match msg.as_str() {
            "listen" => listen_key(&ui),
            "show" => focus_self(),
            _ => {} // anything else (on Windows a local port, which a web page could also reach) is ignored
        });
    });

    // ---- callbacks
    let w = ui.as_weak();
    macro_rules! cb { ($name:ident, |$ui:ident $(, $a:ident : $t:ty)*| $body:expr) => {{ let w = w.clone(); ui.$name(move |$($a: $t),*| { if let Some($ui) = w.upgrade() { $body } }); }}; }
    cb!(on_open, |ui, i: i32| launch_project(&ui, i as usize, Mode::New(None)));
    cb!(on_continue_project, |ui, i: i32| launch_project(&ui, i as usize, Mode::Continue));
    cb!(on_show_project, |ui, i: i32| show_project(&ui, i as usize));
    cb!(on_resume, |ui, id: SharedString| if let Some(i) = with(|st| st.cur) { launch_project(&ui, i, Mode::Resume(&id)) });
    cb!(on_continue_last, |ui| {
        if let Some((i, id)) = with(|st| st.recent.first().cloned().and_then(|s| Some((st.find(&s.root)?, s.id)))) { launch_project(&ui, i, Mode::Resume(&id)) }
    });
    cb!(on_jump, |ui, k: i32| {
        let a = with(|st| st.agents.get(k as usize).cloned());
        if let Some(a) = a {
            let jumped = a.window.as_deref().is_some_and(live::focus);
            if !jumped { if let Some(i) = with(|st| st.find(&a.root)) { show_project(&ui, i) } }
        }
    });
    cb!(on_go, |ui, page: SharedString, arg: SharedString| go(&ui, &page, &arg));
    cb!(on_query_changed, |ui, q: SharedString| {
        search(&ui, &q);
        // a transcript Handy pasted after listening stopped: send it once it stops changing
        if with(|st| st.voice_until.is_some_and(|t| std::time::Instant::now() < t)) && !q.trim().is_empty() {
            let w = ui.as_weak();
            with(|st| st.submit_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(900), move || {
                if let Some(ui) = w.upgrade() {
                    with(|st| { st.voice_until = None; st.voice_msg = true; });
                    let q = ui.get_query();
                    ui.invoke_submit(q);
                }
            }));
        }
    });
    cb!(on_submit, |ui, q: SharedString| {
        with(|st| st.talking = std::mem::take(&mut st.voice_msg)); // typed messages are not read aloud
        let text = q.trim().to_string();
        if text.is_empty() {
            if let Some((i, id)) = with(|st| st.recent.first().cloned().and_then(|s| Some((st.find(&s.root)?, s.id)))) { launch_project(&ui, i, Mode::Resume(&id)) }
            return;
        }
        // Enter always sends; a project opens only when picked from the suggestions (click, or arrows + Enter)
        ui.set_query(ss(""));
        search(&ui, "");
        ask(&ui, &text);
    });
    cb!(on_open_slot, |ui, n: i32| {
        if let Some(i) = with(|st| st.slots.get(n as usize - 1).copied()) { show_project(&ui, i) }
    });
    cb!(on_tool, |ui, kind: SharedString, i: i32| tool(&ui, &kind, i as usize));
    cb!(on_run_action, |ui, k: i32| {
        let r = with(|st| Some((st.projects.get(st.cur?)?.root.clone(), st.cur_actions.get(k as usize)?.clone())));
        if let Some((root, a)) = r { run_action(&ui, &root, &a) }
    });
    cb!(on_set_choice, |ui, key: SharedString, k: i32| {
        let changed = with(|st| {
            let k = k as usize;
            match key.as_str() {
                "theme" => st.s.theme = THEMES.get(k).unwrap_or(&"system").to_string(),
                "glass" => st.s.glass = GLASS.get(k).unwrap_or(&"auto").to_string(),
                key => {
                    let Some(v) = st.choices.get(key).and_then(|l| l.get(k)).map(|x| x.0.clone()) else { return false };
                    match key {
                        "agent" => st.s.agent = v,
                        "terminal" => st.s.terminal = v,
                        "editor" => st.s.editor = v,
                        "browser" => st.s.browser = v,
                        "files" => st.s.files = v,
                        "engine" => st.s.voice_engine = v,
                        "voice" => st.s.voice = v,
                        "speed" => st.s.voice_speed = v.parse().unwrap_or(1.0),
                        _ => return false,
                    }
                }
            }
            save(st);
            true
        });
        if changed { apply_theme(&ui); refresh_home(&ui); refresh_settings(&ui); }
        if changed && key == "agent" {
            // sessions, activity, running agents and chats all come from the selected agent
            with(|st| { st.act_started = false; st.agents.clear(); chat::set_agent(&st.s.agent, &st.s.local_model); });
            clear_chat(&ui);
            rescan(&ui);
        }
    });
    cb!(on_side_width, |_ui, w: f32| with(|st| { st.s.sidebar_width = w; save(st); }));
    cb!(on_zoom, |ui, dir: i32| { let z = zoom_step(with(|st| st.s.zoom), dir); set_zoom(&ui, z); });
    {
        // Ctrl+wheel (and a Windows touchpad pinch, which arrives as one) zooms; so does a macOS pinch
        use slint::winit_030::{winit::event::{MouseScrollDelta, WindowEvent as W}, EventResult, WinitWindowAccessor};
        let w = ui.as_weak();
        let ctrl = std::cell::Cell::new(false);
        let pinch = std::cell::Cell::new(0.0f64);
        ui.window().on_winit_window_event(move |_, e| {
            let Some(ui) = w.upgrade() else { return EventResult::Propagate };
            match e {
                W::ModifiersChanged(m) => ctrl.set(m.state().control_key()),
                W::MouseWheel { delta, .. } if ctrl.get() => {
                    let dy = match delta { MouseScrollDelta::LineDelta(_, y) => *y as f64, MouseScrollDelta::PixelDelta(p) => p.y };
                    if dy != 0.0 { let z = zoom_step(with(|st| st.s.zoom), if dy > 0.0 { 1 } else { -1 }); set_zoom(&ui, z); }
                    return EventResult::PreventDefault;
                }
                W::PinchGesture { delta, .. } => {
                    pinch.set(pinch.get() + delta);
                    if pinch.get().abs() > 0.1 { let z = zoom_step(with(|st| st.s.zoom), pinch.get().signum() as i32); pinch.set(0.0); set_zoom(&ui, z); }
                    return EventResult::PreventDefault;
                }
                W::ScaleFactorChanged { .. } => { let w = ui.as_weak(); slint::Timer::single_shot(Duration::from_millis(50), move || { if let Some(ui) = w.upgrade() { apply_zoom(&ui) } }); }
                _ => {}
            }
            EventResult::Propagate
        });
    }
    cb!(on_set_local_model, |ui, v: SharedString| {
        with(|st| { st.s.local_model = v.trim().to_string(); save(st); chat::set_agent(&st.s.agent, &st.s.local_model); });
        refresh_settings(&ui);
    });
    cb!(on_set_vault, |ui, v: SharedString| {
        let ok = v.is_empty() || Path::new(v.as_str()).is_dir();
        if ok { with(|st| { st.s.vault = v.to_string(); save(st); }); rescan(&ui); }
    });
    cb!(on_add_scan, |ui, path: SharedString, coll: SharedString| {
        let path = path.trim().trim_end_matches(['/', '\\']).to_string();
        if !Path::new(&path).is_dir() { return say(&ui, &format!("{path} is not a folder.")); }
        let coll = if coll.trim().is_empty() { Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or("Projects".into()) } else { coll.trim().to_string() };
        with(|st| { if !st.s.scan_dirs.iter().any(|d| d.path == path) { st.s.scan_dirs.push(ScanDir { path, collection: coll, all: true }); save(st); } });
        rescan(&ui);
    });
    cb!(on_remove_scan, |ui, k: i32| { with(|st| { if (k as usize) < st.s.scan_dirs.len() { st.s.scan_dirs.remove(k as usize); save(st); } }); rescan(&ui); });
    cb!(on_unhide, |ui, k: i32| { with(|st| { if (k as usize) < st.s.hidden.len() { st.s.hidden.remove(k as usize); save(st); } }); rescan(&ui); });
    cb!(on_create_project, |ui, n: SharedString, l: SharedString, p: SharedString| create_project(&ui, &n, &l, &p));
    cb!(on_notice_go, |ui| { ui.set_notice_open(false); if let Some(f) = with(|st| st.notice.take()) { f() } });
    cb!(on_mic, |ui| {
        if asr::installed().is_some() {
            let on = !ui.get_listening();
            if on {
                let base = ui.get_query().to_string();
                with(|st| { hush(st); st.live = asr::Live::start(); st.live_sent = 0; st.live_base = base; st.mic = st.live.as_ref().and_then(|_| mic::Mic::open()); });
                ui.set_listening(with(|st| st.live.is_some()));
                ui.invoke_focus_input();
            } else {
                finish_live(&ui);
            }
            return;
        }
        if launch::which("handy").is_none() { return say(&ui, "Voice input needs Handy (handy.computer)."); }
        let on = !ui.get_listening();
        // Handy only has a toggle, so the hub can drift out of step with it (a press it queued while
        // transcribing, or listening given up here but not there). Starting cancels first, so the toggle always
        // starts a recording; except just after a stop, while Handy may still be typing the transcript.
        let fresh = with(|st| st.stopped.is_some_and(|t| t.elapsed() < Duration::from_secs(4)));
        handy(if on && !fresh { &["--cancel", "--toggle-transcription"] } else { &["--toggle-transcription"] }); // Handy types into the focused field
        // the orb listens along: a loudness meter only, open while Handy records
        ui.set_listening(on);
        with(|st| {
            st.mic = if on { mic::Mic::open() } else { None };
            hush(st); // talking over it: stop speaking
            st.voice_until = (!on && st.s.auto_send).then(|| std::time::Instant::now() + Duration::from_secs(12));
            if !on { st.stopped = Some(std::time::Instant::now()); }
        });
        if on { ui.invoke_focus_input(); }
    });
    ui.on_printable(|t| { let mut c = t.chars(); matches!((c.next(), c.next()), (Some(ch), None) if !ch.is_control() && !('\u{e000}'..='\u{f8ff}').contains(&ch)) });
    ui.on_quit(|| { let _ = slint::quit_event_loop(); });
    cb!(on_resume_in, |ui, i: i32, id: SharedString| launch_project(&ui, i as usize, Mode::Resume(&id)));
    cb!(on_desk_open, |ui, r: DeskRow| {
        if safe_url(&r.url) {
            let s = with(|st| st.s.clone());
            report(&ui, launch::browser(&s, &r.url), "the browser");
        } else if r.idx >= 0 {
            show_project(&ui, r.idx as usize);
        }
    });
    cb!(on_refresh_mail, |ui| fetch_mail(&ui));
    cb!(on_summon, |ui| {
        go(&ui, "home", "");
        ui.invoke_scroll_section(ss("home"));
        ui.invoke_focus_input();
        ui.invoke_mic();
    });
    cb!(on_make_project, |ui| {
        let first = with(|st| st.msgs.iter().find(|m| m.1).map(|m| m.0.clone()).unwrap_or_default());
        open_new(&ui, "", &first);
        ui.set_new_from_chat(true);
    });
    cb!(on_clear_chat, |ui| clear_chat(&ui));
    // a message's speaker button: read it aloud, or stop the reading
    cb!(on_read_aloud, |ui, t: SharedString| if with(|st| st.speech.is_some() || !st.clips.is_empty()) { with(hush) } else { speak(&ui, t.to_string()) });
    cb!(on_toggle_chats, |ui| { with(|st| st.chats_all = !st.chats_all); refresh_chats(&ui); });
    cb!(on_copy_text, |ui, t: SharedString| if !platform::copy(&t) { say(&ui, "Could not reach the clipboard (install wl-clipboard or xclip).") });
    cb!(on_open_link, |ui, u: SharedString| if safe_url(&u) { let s = with(|st| st.s.clone()); report(&ui, launch::browser(&s, &u), "the browser"); });
    cb!(on_rename_chat, |ui, id: SharedString, name: SharedString| {
        match chat::rename(&id, &name) { Ok(()) => refresh_chats(&ui), Err(e) => say(&ui, &format!("Could not rename the chat: {e}")) }
    });
    cb!(on_chat_menu, |ui, what: SharedString, id: SharedString| if what == "delete" {
        match chat::delete(&id) {
            Ok(()) => {
                if ui.get_chat_id() == id { clear_chat(&ui); }
                refresh_chats(&ui);
            }
            Err(e) => say(&ui, &format!("Could not delete the chat: {e}")),
        }
    });
    cb!(on_set_flag, |ui, key: SharedString, on: bool| {
        with(|st| { match key.as_str() { "talkback" => st.s.talkback = on, "auto-send" => st.s.auto_send = on, "auto-update" => st.s.auto_update = on, "mail" => { st.s.mail = on; if !on { st.brief = None; } } _ => return } save(st); });
        if !on && key == "talkback" { with(hush); }
        if key == "mail" { if on { fetch_mail(&ui); } else { show_desk(&ui); } }
        refresh_settings(&ui);
    });
    cb!(on_set_chat_dir, |ui, t: SharedString| {
        let t = t.trim();
        let path = match t.strip_prefix("~/") { Some(r) => dirs::home_dir().unwrap_or_default().join(r).to_string_lossy().into_owned(), None => t.to_string() };
        if !path.is_empty() && !Path::new(&path).is_absolute() { return say(&ui, "Give the chat folder as a full path, e.g. ~/Chats."); }
        if !path.is_empty() { if let Err(e) = std::fs::create_dir_all(&path) { return say(&ui, &format!("Could not use {path}: {e}")); } }
        with(|st| { st.s.chat_dir = path.clone(); save(st); });
        chat::set_dir(&path);
        clear_chat(&ui);
        refresh_chats(&ui);
        refresh_settings(&ui);
        rescan(&ui); // the chat folder is never listed as a project
    });
    cb!(on_test_voice, |ui| speak(&ui, "Hi. This is how I sound when I read your replies aloud.".into()));
    cb!(on_open_chat, |ui, id: SharedString| open_chat(&ui, &id));
    cb!(on_new_chat, |ui| { clear_chat(&ui); show_chat(&ui); });
    cb!(on_mode_changed, |ui, m: SharedString| {
        ui.set_query(ss(""));
        ui.set_suggest(model(vec![]));
        if m != "chat" { return go(&ui, "home", ""); }
        // Chat picks up the last conversation, so it is one click (or Ctrl+2) back to where you were
        let latest = (ui.get_chat_id().is_empty()).then(|| chat::list(1).into_iter().next()).flatten();
        match latest { Some((id, _, _)) => open_chat(&ui, &id), None => show_chat(&ui) }
    });
    cb!(on_scroll_to, |ui, y: f32| scroll_to(&ui, y));
    cb!(on_toggle_side, |ui| {
        let c = with(|st| { st.s.sidebar_collapsed = !st.s.sidebar_collapsed; let _ = st.s.save(); st.s.sidebar_collapsed });
        ui.set_side_collapsed(c);
    });

    // ---- live agents, usage and theme every 5 s, off the UI thread
    let w2 = ui.as_weak();
    let home = scan::home();
    std::thread::spawn(move || {
        let mut watcher = live::Watcher::new();
        let mut sys = sysinfo::System::new().ok();
        loop {
            let load = sys.as_mut().map(|s| {
                s.refresh_cpu_usage();
                s.refresh_memory();
                (s.global_cpu_usage(), s.used_memory(), s.total_memory())
            });
            let roots_now = roots.lock().unwrap().clone();
            let (agents, status) = watcher.as_mut().map(|w| w.scan(&roots_now, &home)).unwrap_or_else(|| (vec![], live::bloom_status()));
            let dark = platform::system_dark();
            let status = status.map(|s| s.to_string());
            if w2.upgrade_in_event_loop(move |ui| {
                let status: Option<serde_json::Value> = status.and_then(|s| serde_json::from_str(&s).ok());
                let (changed, theme_changed) = with(|st| {
                    let agents: Vec<live::Agent> = agents.into_iter().filter(|a| a.tool == st.s.agent).collect(); // the selected agent only
                    let ch = st.agents != agents;
                    let tc = dark.is_some_and(|d| d != st.sys_dark);
                    if let Some(d) = dark { st.sys_dark = d; }
                    st.agents = agents;
                    st.status = status;
                    st.sys = load;
                    (ch, tc)
                });
                show_stats(&ui);
                if theme_changed { apply_theme(&ui); }
                if changed { refresh_home(&ui); } else { refresh_home_facts(&ui); }
            }).is_err() { break; }
            std::thread::sleep(Duration::from_secs(5));
        }
    });

    // ---- the live-mark clock: 20 fps, only while an agent works or waits and the window has focus
    // ---- the desk clock: 60 fps while the orb is on screen and the window has focus; otherwise nothing runs
    let anim = slint::Timer::default();
    let w3 = ui.as_weak();
    // Frames are drawn only while the desk is on screen and the window has focus: 30 fps while the pointer
    // moves, the mic is open or the blob is still easing; 20 fps of slow drift for 12 s after the last
    // interaction; then it rests on its last frame and costs nothing.
    let mut globe = orb::Orb::new(2400);
    let mut last = std::time::Instant::now();
    let mut poke = last;
    let mut quiet_since = last;
    let (mut tick, mut busy, mut seen, mut ptr) = (0u32, false, (false, -1.0f32, SharedString::new()), (0.0f32, 0.0f32));
    let draw = |ui: &AppWindow, globe: &orb::Orb| ui.set_orb_frame(globe.frame((420.0 * ui.window().scale_factor()).round() as u32));
    draw(&ui, &globe); // a still first frame, even before the window has focus
    anim.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || {
        let Some(ui) = w3.upgrade() else { return };
        key_released(&ui);
        live_tick(&ui);
        let now = std::time::Instant::now();
        tick = tick.wrapping_add(1);
        let focused = window_focused(&ui);
        let state = (focused, ui.get_dock_raw(), ui.get_query());
        if state != seen { poke = now; seen = state; } // focus, scroll or typing wakes it
        let listening = ui.get_listening();
        let (voice, speaking) = with(|st| {
            if st.speech.as_mut().is_some_and(|s| s.done()) { st.speech = None; }
            if st.speech.is_none() { if let Some(c) = st.clips.pop_front() { st.speech = speech::play(c); } }
            let talk = st.speech.as_ref().map(|s| s.level());
            (st.mic.as_ref().map(|m| m.level()).unwrap_or(0.0).max(talk.unwrap_or(0.0)), talk.is_some())
        });
        if listening {
            // the meter closes the mic itself after 2 min of quiet (Handy may have been stopped elsewhere)
            if voice > 0.05 { quiet_since = now; }
            if now.duration_since(quiet_since).as_secs() >= 120 {
                if with(|st| st.live.is_some()) { finish_live(&ui) } else { ui.set_listening(false); with(|st| st.mic = None); handy(&["--cancel"]); }
            }
        }
        if (ui.get_dock_raw() >= 1.0 && ui.get_page() != "chat") || !focused { last = now; return; }
        let hover = ui.get_orb_hover();
        let p = (ui.get_orb_mx(), ui.get_orb_my());
        let moving = (hover && p != ptr) || ui.get_orb_down();
        ptr = p;
        if moving || listening || speaking { poke = now; }
        let idle = now.duration_since(poke).as_secs_f32();
        let active = listening || speaking || busy || idle < 0.3;
        let due = (active && tick % 2 == 0) || (idle < 12.0 && tick % 3 == 0); // 30 fps active, 20 fps drift
        if !due { return; }
        let dt = now.duration_since(last).as_secs_f32().min(0.1);
        last = now;
        let th = ui.global::<Theme>();
        th.set_phase(th.get_phase() + dt);
        let down = ui.get_orb_down();
        let (r, g, b) = launch::agent_color(&with(|st| st.s.agent.clone()));
        globe.tint = [r, g, b].map(|c| 0.55 + 0.45 * c as f32 / 255.0); // mostly white, leaning to the agent's colour
        busy = globe.step(&orb::Input { dt, pointer: hover.then_some(p), voice, down });
        draw(&ui, &globe);
    });
    // notifications, weather, deadlines: every minute
    let desk_timer = slint::Timer::default();
    let w4 = ui.as_weak();
    desk_timer.start(slint::TimerMode::Repeated, Duration::from_secs(60), move || { if let Some(ui) = w4.upgrade() { refresh_desk(&ui) } });

    // --page settings | --project <root> | --search <text>: open straight on a page (for checks)
    for k in 1..args.len().saturating_sub(1) {
        match args[k].as_str() {
            "--page" => go(&ui, &args[k + 1], ""),
            "--project" => if let Some(i) = with(|st| st.find(&args[k + 1])) { show_project(&ui, i) },
            "--search" => { ui.set_query(ss(&args[k + 1])); search(&ui, &args[k + 1]); }
            "--say" => say(&ui, &args[k + 1]),
            "--new" => open_new(&ui, &args[k + 1], ""),
            _ => {}
        }
    }

    {
        // the window exists once the event loop runs: apply the saved zoom then
        let w = ui.as_weak();
        slint::Timer::single_shot(Duration::from_millis(30), move || { if let Some(ui) = w.upgrade() { set_zoom(&ui, with(|st| st.s.zoom)) } });
    }
    if listen_now {
        // launched by the listen key: only open and focus; the next press listens
        slint::Timer::single_shot(Duration::from_millis(600), focus_self);
    }
    ui.run().expect("event loop");
}

/// Sidebar usage meters from Bloom's limits, hidden when Bloom is not running.
fn set_usage(ui: &AppWindow, status: &Option<serde_json::Value>) {
    let lim = status.as_ref().map(|s| &s["limits"]).filter(|l| l.is_object());
    ui.set_has_usage(lim.is_some());
    let Some(lim) = lim else { return };
    let part = |k: &str| (lim[k]["used"].as_f64().unwrap_or(0.0) as f32 / 100.0, lim[k]["resets_at"].as_f64().map(|t| format!("Resets {}", when(t))).unwrap_or_default());
    let (f, fnote) = part("five_hour");
    let (w, wnote) = part("seven_day");
    ui.set_usage_five(f);
    ui.set_usage_five_note(ss(fnote));
    ui.set_usage_week(w);
    ui.set_usage_week_note(ss(wnote));
    ui.set_usage_model(ss(lim["model"].as_str().unwrap_or("")));
}

/// Usage numbers move every poll; only the sidebar meters change.
fn refresh_home_facts(ui: &AppWindow) {
    refresh_head(ui);
    let status = with(|st| st.status.clone().filter(|_| st.s.agent == "claude"));
    set_usage(ui, &status);
}

// ---- activity ---------------------------------------------------------------

fn heat(g: activity::Grid, idle: String) -> Heat {
    Heat {
        levels: model(g.levels),
        tips: model(g.tips.into_iter().map(ss).collect()),
        months: model(g.months.into_iter().map(|(col, text)| Month { col, text: ss(text) }).collect()),
        idle: ss(idle),
    }
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn short(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

/// Fill the Activity page. Session logs and GitHub load on their own threads; each fills its part when ready.
fn load_activity(ui: &AppWindow) {
    if with(|st| std::mem::replace(&mut st.act_started, true)) {
        return;
    }
    let projects = with(|st| st.projects.clone());
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let a = activity::load(&projects);
        let _ = weak.upgrade_in_event_loop(move |ui| show_activity(&ui, a));
    });
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let r = activity::github();
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let today = chrono::Local::now().date_naive();
            match r {
                Ok((login, days)) => {
                    let s = activity::summary(&days, today);
                    ui.set_gh_title(ss(if login.is_empty() { "GitHub".into() } else { format!("GitHub · {login}") }));
                    ui.set_gh_sub(ss(format!("{} contributions in the last year", thousands(s.total))));
                    let idle = match s.best { Some((d, n)) => format!("Busiest: {} · {n}", d.format("%a %-d %b")), None => "No contributions yet".into() };
                    ui.set_gh_heat(heat(activity::year_grid(&days, "contribution", today), idle));
                    ui.set_gh_note(ss(""));
                }
                Err(e) => ui.set_gh_note(ss(e)),
            }
        });
    });
}

fn show_activity(ui: &AppWindow, a: activity::Activity) {
    with(|st| st.prompts = a.days.clone());
    let today = chrono::Local::now().date_naive();
    let s = activity::summary(&a.days, today);
    let best = s.best.map(|(d, n)| format!("{} · {n}", d.format("%-d %b"))).unwrap_or("—".into());
    ui.set_act_facts(model(vec![
        Fact { title: ss("Prompts, last year"), value: ss(thousands(s.total)), sub: ss(""), warn: false },
        Fact { title: ss("Active days"), value: ss(format!("{} of 365", s.active)), sub: ss(""), warn: false },
        Fact { title: ss("Streak"), value: ss(if s.streak > 0 { plural(s.streak, "day") } else { "—".into() }), sub: ss(""), warn: false },
        Fact { title: ss("Longest streak"), value: ss(plural(s.longest, "day")), sub: ss(""), warn: false },
        Fact { title: ss("Busiest day"), value: ss(best), sub: ss(""), warn: false },
    ]));
    let tok_year: u64 = a.tokens.values().sum();
    ui.set_claude_sub(ss(format!("{} prompts · {} output tokens", thousands(s.total), short(tok_year))));
    let idle = match s.best { Some((d, n)) => format!("Busiest: {} · {n} prompts", d.format("%a %-d %b %Y")), None => "No prompts yet".into() };
    ui.set_claude_heat(heat(activity::year_grid(&a.days, "prompt", today), idle));
    ui.set_punch(heat(activity::punch_card(&a.hours, today), "Darker squares are the hours you prompt most.".into()));

    // last 30 days, one bar a day; a label each Monday
    let days: Vec<chrono::NaiveDate> = (0..30).rev().map(|i| today - chrono::Duration::days(i)).collect();
    let vals: Vec<u64> = days.iter().map(|d| a.days.get(&d.format("%Y-%m-%d").to_string()).copied().unwrap_or(0)).collect();
    let max = vals.iter().copied().max().unwrap_or(0).max(1) as f32;
    ui.set_month_bars(model(days.iter().zip(&vals).map(|(d, n)| {
        let tok = a.tokens.get(&d.format("%Y-%m-%d").to_string()).copied().unwrap_or(0);
        use chrono::Datelike;
        Bar { label: ss(if d.weekday() == chrono::Weekday::Mon { d.format("%-d").to_string() } else { String::new() }), value: *n as f32 / max, tip: ss(format!("{} · {} prompts · {} tokens", d.format("%a %-d %b"), n, short(tok))), hue: 0, idx: 0 }
    }).collect()));

    let hours = activity::by_hour(&a.hours, today);
    let max = hours.iter().copied().max().unwrap_or(0).max(1) as f32;
    ui.set_hour_bars(model(hours.iter().enumerate().map(|(h, n)| Bar { label: ss(if h % 6 == 0 { format!("{h:02}") } else { String::new() }), value: *n as f32 / max, tip: ss(format!("{h:02}:00–{:02}:00 · {} prompts", (h + 1) % 24, thousands(*n))), hue: 0, idx: 0 }).collect()));

    let top = with(|st| {
        let max = a.projects.first().map(|p| p.1).unwrap_or(1).max(1) as f32;
        a.projects.iter().take(8).filter_map(|(root, n, tok)| {
            let i = st.find(root)?;
            let it = st.item(i, 0);
            Some(Bar { label: it.name.clone(), value: *n as f32 / max, tip: ss(format!("{} prompts · {} tokens", thousands(*n), short(*tok))), hue: it.hue, idx: i as i32 })
        }).collect::<Vec<_>>()
    });
    ui.set_top_bars(model(top));
    ui.set_act_loaded(true);
    refresh_head(ui);
}

// ---- desk -------------------------------------------------------------------

/// Links in mail/calendar rows come from a model reading email text: open only Google's own pages.
fn safe_url(u: &str) -> bool {
    ["https://mail.google.com/", "https://calendar.google.com/", "https://www.google.com/calendar/"].iter().any(|p| u.starts_with(p))
}

fn row(title: &str, sub: &str, tag: &str, warn: bool, idx: i32, url: &str) -> DeskRow {
    DeskRow { title: ss(title), sub: ss(sub), tag: ss(tag), warn, idx, url: ss(url) }
}

fn day_tag(d: chrono::NaiveDate, today: chrono::NaiveDate) -> (String, bool) {
    let n = (d - today).num_days();
    match n {
        ..=-2 => (format!("{} days late", -n), true),
        -1 => ("Yesterday".into(), true),
        0 => ("Today".into(), true),
        1 => ("Tomorrow".into(), true),
        2..=6 => (d.format("%a").to_string(), false),
        _ => (d.format("%-d %b").to_string(), false),
    }
}

thread_local! {
    static START: u64 = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() as u64).unwrap_or(0);
}

/// A short line for the desk, picked fresh each time the hub opens and suited to the hour.
fn greeting(hour: u32, seed: u64) -> &'static str {
    let lines: &[&str] = match hour {
        5..=11 => &["Rise and compile.", "Fresh day, clean build.", "Coffee first, then code.", "The bugs are still asleep.", "Morning. Let's ship something.", "New day, new commits."],
        12..=17 => &["Back at it.", "Halfway there. Keep going.", "Let's make it work.", "Afternoon. What's next?", "Ship o'clock.", "Plenty of day left."],
        18..=23 => &["Evening shift.", "One more commit?", "Quiet hours, good code.", "Night mode engaged.", "Still building. Respect.", "Let's wrap something up."],
        _ => &["Burning the midnight oil?", "It's late. One more?", "The stars are out. So are you.", "Night owl mode."],
    };
    lines[(seed % lines.len() as u64) as usize]
}

/// Header line, greeting and today's numbers; cheap, runs with every home refresh.
fn refresh_head(ui: &AppWindow) {
    let now = chrono::Local::now();
    use chrono::Timelike;
    ui.set_greeting(ss(greeting(now.hour(), START.with(|s| *s))));
    ui.set_desk_date(ss(now.format("%A, %-d %B").to_string()));
    with(|st| {
        let working = st.agents.iter().filter(|a| a.state == "working").count();
        let waiting = st.agents.iter().filter(|a| a.state == "waiting").count();
        let mut parts = vec![];
        if working > 0 { parts.push(format!("{} working", plural(working, "agent"))); }
        if waiting > 0 { parts.push(format!("{waiting} need{} you", if waiting == 1 { "s" } else { "" })); }
        ui.set_desk_sub(ss(if parts.is_empty() { "What are we working on?".to_string() } else { parts.join(" · ") }));
        // prompts once the activity scan is in, sessions before that
        let (src, unit) = if st.prompts.is_empty() { (st.days.iter().map(|(k, v)| (k.clone(), *v as u64)).collect(), "session") } else { (st.prompts.clone(), "prompt") };
        let day = |d: i64| src.get(&(now - chrono::Duration::days(d)).format("%Y-%m-%d").to_string()).copied().unwrap_or(0);
        let start = if day(0) > 0 { 0 } else { 1 };
        let streak = (start..3650).take_while(|d| day(*d) > 0).count();
        let mut line = vec![plural(day(0) as usize, unit) + " today"];
        if streak > 1 { line.push(format!("{streak}-day streak")); }
        ui.set_today_line(ss(line.join(" · ")));
    });
}

/// Read desk sources on a thread, then fill the widgets.
fn refresh_desk(ui: &AppWindow) {
    let (vault, dir) = with(|st| (st.s.vault.clone(), st.s.scan_dirs.first().map(|d| d.path.clone()).unwrap_or_else(scan::home)));
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let d = desk_data(&vault, &dir);
        let _ = weak.upgrade_in_event_loop(move |ui| { with(|st| st.desk = Some(d)); show_desk(&ui); });
    });
}

fn show_desk(ui: &AppWindow) {
    let today = chrono::Local::now().date_naive();
    refresh_head(ui);
    with(|st| {
        let Some(d) = st.desk.take() else { return };
        ui.set_wx(match &d.weather {
            Some(w) => WeatherView {
                has: true,
                temp: ss(format!("{:.0}°", w.temp)),
                text: ss(w.text),
                place: ss(&w.place),
                hilo: ss(if w.hi.is_nan() { String::new() } else { format!("H {:.0}°  ·  L {:.0}°  ·  Wind {:.0} km/h", w.hi, w.lo, w.wind) }),
                sun: ss(if w.sunrise.is_empty() { String::new() } else { format!("Sunrise {}  ·  Sunset {}", w.sunrise, w.sunset) }),
                kind: ss(std::env::var("PH_WEATHER").unwrap_or_else(|_| w.kind.to_string())), // PH_WEATHER=storm etc.: preview the art
                day: w.day || std::env::var("PH_WEATHER").is_ok_and(|k| k == "sun" || k == "partly"),
                windy: w.wind >= 25.0 || std::env::var_os("PH_WINDY").is_some(),
                hours: model(w.hours.iter().map(|(t, k, c, _)| Hour { time: ss(t), kind: ss(*k), temp: ss(format!("{c:.0}°")) }).collect()),
            },
            None => WeatherView::default(),
        });
        match &d.screen {
            Some((total, apps)) if *total > 0 => {
                ui.set_screen_total(ss(dur(*total)));
                ui.set_screen_apps(model(apps.iter().map(|(n, s)| row(n, "", &dur(*s), false, -1, "")).collect()));
            }
            _ => ui.set_screen_total(ss("")),
        }
        // coming up: dated vault tasks and calendar events, soonest first
        let mut up: Vec<(chrono::NaiveDate, String, DeskRow)> = d.deadlines.iter().map(|x| {
            let (tag, warn) = day_tag(x.date, today);
            let idx = st.projects.iter().position(|p| scan::encode(&p.name.to_lowercase()) == scan::encode(&x.source.to_lowercase())).map(|i| i as i32).unwrap_or(-1);
            (x.date, String::new(), row(&x.text, &x.source, &tag, warn, idx, ""))
        }).collect();
        for e in st.brief.as_ref().map(|b| b.events.clone()).unwrap_or_default() {
            let t = chrono::DateTime::parse_from_rfc3339(&e.start).map(|t| t.with_timezone(&chrono::Local)).ok();
            let date = t.map(|t| t.date_naive()).or_else(|| chrono::NaiveDate::parse_from_str(e.start.get(..10).unwrap_or(""), "%Y-%m-%d").ok());
            let Some(date) = date else { continue };
            let (mut tag, warn) = day_tag(date, today);
            if let Some(t) = t { tag = format!("{tag} {}", t.format("%H:%M")); }
            up.push((date, e.start.clone(), row(&e.title, "Calendar", &tag, warn, -1, if safe_url(&e.url) { &e.url } else { "" })));
        }
        up.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        ui.set_upcoming(model(up.into_iter().map(|x| x.2).collect()));
        // to do: today's note, then the next item of each recently used project
        let mut todos: Vec<DeskRow> = d.daily.iter().map(|t| row(t, "Today's note", "", false, -1, "")).collect();
        let mut order: Vec<usize> = (0..st.projects.len()).filter(|i| st.projects[*i].sessions > 0).collect();
        order.sort_by(|a, b| st.projects[*b].last.total_cmp(&st.projects[*a].last));
        for i in order.into_iter().take(12) {
            if todos.len() >= 6 { break; }
            let root = st.projects[i].root.clone();
            if let Some(t) = st.notes(&root).todo.first().cloned() {
                todos.push(row(&t, &st.projects[i].name, "", false, i as i32, ""));
            }
        }
        ui.set_todos(model(todos));
        let mut seen = std::collections::HashSet::new();
        ui.set_notifs(model(d.notifs.iter().filter(|n| seen.insert((n.app.clone(), n.summary.clone()))).map(|n| row(&n.summary, &n.app, &when(n.ms as f64 / 1000.0), false, -1, "")).collect()));
        st.desk = Some(d);
    });
    show_stats(ui);
}

fn show_mail(ui: &AppWindow) {
    with(|st| match &st.brief {
        Some(b) => {
            ui.set_mail(model(b.mail.iter().map(|m| {
                let when = chrono::DateTime::parse_from_rfc3339(&m.when).map(|t| scan::ago(t.timestamp() as f64)).unwrap_or_else(|_| m.when.chars().take(10).collect());
                row(&m.subject, &m.from, &when, false, -1, if safe_url(&m.url) { &m.url } else { "" })
            }).collect()));
            ui.set_mail_note(ss(if b.mail.is_empty() { "No unread mail this week." } else { "" }));
        }
        None => ui.set_mail_note(ss("Loading…")),
    });
}

/// Unread mail and the week's events via Claude Code's connectors; ~30 s, cached for an hour.
fn fetch_mail(ui: &AppWindow) {
    if ui.get_mail_busy() || !with(|st| st.s.mail) { return; }
    ui.set_mail_busy(true);
    if with(|st| st.brief.is_none()) { ui.set_mail_note(ss("Checking mail…")); }
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let r = desk::fetch_brief();
        let _ = weak.upgrade_in_event_loop(move |ui| {
            ui.set_mail_busy(false);
            match r {
                Ok(b) => { with(|st| st.brief = Some(b)); show_mail(&ui); show_desk(&ui); }
                Err(e) => if with(|st| st.brief.is_none()) { ui.set_mail_note(ss(e)) },
            }
        });
    });
}

// ---- chat ---------------------------------------------------------------------

/// Drop a trailing "Thinking…" placeholder.
fn unthink(ui: &AppWindow) {
    with(|st| if st.msgs.last().is_some_and(|m| !m.1 && m.0 == "Thinking…") { st.msgs.pop(); });
    show_msgs(ui);
}

fn chat_send(ui: &AppWindow, text: &str) {
    if ui.get_mode() == "chat" && ui.get_page() != "chat" { show_chat(ui); }
    say(ui, "Thinking…");
    let sid = with(|st| st.chat_sid.clone());
    let weak = ui.as_weak();
    let text = text.to_string();
    std::thread::spawn(move || {
        let r = chat::send(sid.as_deref(), &text);
        let _ = weak.upgrade_in_event_loop(move |ui| {
            unthink(&ui);
            match r {
                Ok((sid, reply)) => {
                    ui.set_chat_id(ss(&sid));
                    with(|st| st.chat_sid = Some(sid));
                    ui.set_chat_active(true);
                    refresh_chats(&ui);
                    say(&ui, if reply.is_empty() { "(no answer)" } else { &reply });
                    if !reply.is_empty() && with(|st| st.talking && st.s.talkback) { speak(&ui, reply); }
                }
                Err(e) => say(&ui, &e),
            }
        });
    });
}

fn refresh_chats(ui: &AppWindow) {
    const FEW: usize = 5; // the sidebar keeps the latest chats; the rest stay behind "Show all"
    let mut all = chat::list(usize::MAX);
    let (open, every) = (ui.get_chat_id().to_string(), with(|st| st.chats_all));
    let total = all.len();
    if !every && total > FEW {
        let tail = all.split_off(FEW);
        all.extend(tail.into_iter().filter(|c| c.0 == open)); // the open chat stays listed
    }
    ui.set_chats_more((total - all.len()) as i32);
    ui.set_chats_all(every && total > FEW);
    ui.set_chats(model(all.into_iter().map(|(id, t, mt)| ChatItem { id: ss(id), title: ss(t.chars().take(40).collect::<String>()), when: ss(scan::ago(mt)) }).collect()));
}

/// Open a past chat on the desk, ready to continue.
fn open_chat(ui: &AppWindow, id: &str) {
    let mut h = chat::history(id);
    let n = h.len();
    h.drain(..n.saturating_sub(30));
    with(|st| { st.msgs = h; st.chat_sid = Some(id.to_string()); });
    ui.set_chat_active(true);
    ui.set_chat_id(ss(id));
    show_chat(ui);
}

fn clear_chat(ui: &AppWindow) {
    ui.set_chat_id(ss(""));
    with(|st| { st.msgs.clear(); st.chat_sid = None; });
    show_msgs(ui);
    ui.set_chat_active(false);
    ui.set_reply_open(false);
}

/// Ease the desk scroll to `y` (content offset) over ~0.4 s.
fn scroll_to(ui: &AppWindow, y: f32) {
    let from = ui.get_home_y();
    let to = -y;
    let start = std::time::Instant::now();
    let weak = ui.as_weak();
    with(|st| st.tween.start(slint::TimerMode::Repeated, Duration::from_millis(12), move || {
        let Some(ui) = weak.upgrade() else { return };
        let t = (start.elapsed().as_secs_f32() / 0.4).min(1.0);
        let e = if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 };
        ui.set_home_y(from + (to - from) * e);
        if t >= 1.0 { with(|st| st.tween.stop()); }
    }));
}

/// Stop speaking, and drop what was still being synthesised.
fn hush(st: &mut State) {
    st.speech = None;
    st.clips.clear();
    speech::hush();
}

/// Say `text` aloud (synthesis off the UI thread, piece by piece; the frame loop plays them in turn).
fn speak(ui: &AppWindow, text: String) {
    let v = with(|st| { hush(st); voice(st) });
    let generation = speech::generation();
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        speech::render(&text, &v, generation, |clip| {
            let _ = weak.upgrade_in_event_loop(move |_| with(|st| if clip.generation == speech::generation() { st.clips.push_back(clip) }));
        });
    });
}

fn join_words(parts: &[&str]) -> String {
    parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ")
}

fn send_voice(ui: &AppWindow, text: String) {
    with(|st| st.voice_msg = true);
    ui.invoke_submit(ss(text));
}

/// Every frame while listening live: words appear in the box as they are spoken; with auto-send each finished
/// sentence is sent, and listening goes on, so a conversation runs back and forth until the key stops it.
fn live_tick(ui: &AppWindow) {
    let Some((h, speaking, sent, base, auto)) = with(|st| {
        let speaking = st.speech.is_some() || !st.clips.is_empty();
        st.live.as_ref().map(|l| (l.heard(), speaking, st.live_sent, st.live_base.clone(), st.s.auto_send))
    }) else { return };
    if h.ended {
        eprintln!("speech to text stopped (microphone or model failed)");
        with(|st| { st.live = None; st.mic = None; });
        return ui.set_listening(false);
    }
    if ui.get_listen_ready() != h.ready { ui.set_listen_ready(h.ready); }
    if speaking {
        // the hub's own voice is not the user talking: what is heard meanwhile is skipped
        return with(|st| st.live_sent = h.done.len() + usize::from(!h.now.is_empty()));
    }
    let spoken = join_words(&h.done.iter().skip(sent).map(String::as_str).collect::<Vec<_>>());
    let now = if sent <= h.done.len() { h.now.as_str() } else { "" }; // else: the tail of the hub's own voice
    if auto && !spoken.is_empty() {
        with(|st| { st.live_sent = h.done.len(); st.live_base.clear(); });
        send_voice(ui, join_words(&[&base, &spoken]));
        ui.set_query(ss(now));
    } else {
        let shown = join_words(&[&base, &spoken, now]);
        if ui.get_query() != shown.as_str() { ui.set_query(ss(shown)); }
    }
}

/// Stop listening live; what was still being said is sent (auto-send) or left in the box.
fn finish_live(ui: &AppWindow) {
    let Some((h, sent, base)) = with(|st| { st.mic = None; st.live.take().map(|l| (l.heard(), st.live_sent, std::mem::take(&mut st.live_base))) }) else { return };
    ui.set_listening(false);
    ui.set_listen_ready(true);
    let rest = join_words(&h.done.iter().skip(sent).map(String::as_str).chain((sent <= h.done.len()).then_some(h.now.as_str())).collect::<Vec<_>>());
    let text = join_words(&[&base, &rest]);
    if with(|st| st.s.auto_send) && !rest.is_empty() { send_voice(ui, text) } else { ui.set_query(ss(text)) }
}

/// Bring this window to the front (compositors that allow it: niri, Hyprland, sway).
/// Browser zoom steps.
const ZOOMS: &[f32] = &[0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

/// The next zoom step up (+1), down (-1), or back to 100 % (0).
fn zoom_step(z: f32, dir: i32) -> f32 {
    match dir {
        0 => 1.0,
        d if d > 0 => ZOOMS.iter().copied().find(|s| *s > z + 0.001).unwrap_or(2.0),
        _ => ZOOMS.iter().rev().copied().find(|s| *s < z - 0.001).unwrap_or(0.5),
    }
}

/// Scale the whole window: the desktop's own scale times the user's zoom.
fn apply_zoom(ui: &AppWindow) {
    use slint::winit_030::WinitWindowAccessor;
    let zoom = with(|st| st.s.zoom).clamp(0.5, 2.0);
    let Some(base) = ui.window().with_winit_window(|w| w.scale_factor() as f32) else { return };
    let sf = base * zoom;
    let w = ui.window();
    if (w.scale_factor() - sf).abs() < 0.001 {
        return;
    }
    w.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: sf });
    w.dispatch_event(slint::platform::WindowEvent::Resized { size: w.size().to_logical(sf) });
}

fn set_zoom(ui: &AppWindow, z: f32) {
    with(|st| { st.s.zoom = z.clamp(0.5, 2.0); save(st); });
    apply_zoom(ui);
    ui.set_zoom_label(ss(format!("{:.0}%", with(|st| st.s.zoom) * 100.0)));
}

fn agent_color(agent: &str) -> slint::Color {
    let (r, g, b) = launch::agent_color(agent);
    slint::Color::from_rgb_u8(r, g, b)
}

thread_local! {
    static UI: RefCell<Option<slint::Weak<AppWindow>>> = const { RefCell::new(None) };
}

/// Bring Cove to the front: through the compositor where it allows that (niri, Hyprland, sway), else through the
/// windowing system (Windows, X11; other Wayland desktops may only mark the window as wanting attention).
fn focus_self() {
    if let Some(w) = live::windows().get(&std::process::id()) {
        if live::focus(w) {
            return;
        }
    }
    use slint::winit_030::WinitWindowAccessor;
    UI.with(|u| {
        if let Some(ui) = u.borrow().as_ref().and_then(|w| w.upgrade()) {
            ui.window().with_winit_window(|w| {
                w.set_minimized(false);
                w.focus_window();
            });
        }
    });
}

/// The listen key, hold or toggle. niri cannot report a key release, but a held key repeats, so each
/// repeat arrives as another "listen": a lone message toggles; a stream of them is a hold, and listening
/// stops when the stream ends (see `key_released`).
fn listen_key(ui: &AppWindow) {
    let now = std::time::Instant::now();
    let repeat = with(|st| match &mut st.key {
        Some((last, held)) if now.duration_since(*last) < Duration::from_millis(700) => { *last = now; *held = true; true }
        _ => false,
    });
    if repeat {
        return;
    }
    if !window_focused(ui) {
        // another window was active: only bring the hub up; the next press, with the hub in front, listens
        with(|st| st.key = Some((now, false)));
        return focus_self();
    }
    if ui.get_listening() {
        with(|st| st.key = None);
        ui.invoke_mic(); // second tap: stop
    } else {
        with(|st| st.key = Some((now, false)));
        listen(ui);
    }
}

/// Called every frame: ends a hold once the key's repeats stop; forgets a tap after a moment.
fn key_released(ui: &AppWindow) {
    let now = std::time::Instant::now();
    let (release, forget) = with(|st| match st.key {
        Some((last, true)) if now.duration_since(last) > Duration::from_millis(220) => (true, true),
        Some((last, false)) if now.duration_since(last) > Duration::from_millis(700) => (false, true),
        _ => (false, false),
    });
    if forget { with(|st| st.key = None); }
    if release && ui.get_listening() { ui.invoke_mic(); }
}

/// The listen shortcut: show the desk and start or stop listening.
fn listen(ui: &AppWindow) {
    focus_self();
    if ui.get_page() != "chat" {
        go(ui, "home", "");
        ui.invoke_scroll_section(ss("home"));
    }
    ui.invoke_focus_input();
    ui.invoke_mic();
}

#[cfg(test)]
mod zoom_tests {
    #[test]
    fn zoom_steps_like_a_browser() {
        assert_eq!(super::zoom_step(1.0, 1), 1.1);
        assert_eq!(super::zoom_step(1.0, -1), 0.9);
        assert_eq!(super::zoom_step(2.0, 1), 2.0);
        assert_eq!(super::zoom_step(0.5, -1), 0.5);
        assert_eq!(super::zoom_step(1.33, 0), 1.0);
    }
}
