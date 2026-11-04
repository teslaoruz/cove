//! Activity charts: prompts per hour/day from agent session logs (cached per file), GitHub contributions via `gh`.
use crate::scan::{self, Project};
use crate::settings::cache_dir;
use chrono::{Datelike, Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

/// What one session file adds up to. Cached by path, size and mtime: the logs total hundreds of MB.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct FileStat {
    size: u64,
    mtime: f64,
    hours: HashMap<String, u32>, // "YYYY-MM-DD HH" local -> typed prompts
    tokens: HashMap<String, u64>, // "YYYY-MM-DD" local -> output tokens
}

fn local(ts: &str) -> Option<chrono::DateTime<Local>> {
    chrono::DateTime::parse_from_rfc3339(ts).ok().map(|t| t.with_timezone(&Local))
}

fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let i = line.find(key)? + key.len();
    let rest = &line[i..];
    Some(&rest[..rest.find('"')?])
}

fn read_file(path: &Path) -> FileStat {
    let mut st = FileStat::default();
    let mut seen = HashSet::new(); // an assistant message is logged once per content block with the same usage
    let Ok(fh) = fs::File::open(path) else { return st };
    for line in BufReader::new(fh).lines().map_while(Result::ok) {
        if line.contains("\"type\":\"user\"") && !line.contains("\"tool_use_id\"") && !line.contains("\"isSidechain\":true") {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
            if scan::user_text(&v).is_none() {
                continue;
            }
            if let Some(t) = v["timestamp"].as_str().and_then(local) {
                *st.hours.entry(t.format("%Y-%m-%d %H").to_string()).or_insert(0) += 1;
            }
        } else if line.contains("\"type\":\"assistant\"") {
            let (Some(id), Some(ts)) = (field(&line, "\"id\":\"msg_"), field(&line, "\"timestamp\":\"")) else { continue };
            if !seen.insert(id.to_string()) {
                continue;
            }
            let n: u64 = line.find("\"output_tokens\":").map(|i| line[i + 16..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>()).and_then(|s| s.parse().ok()).unwrap_or(0);
            if let Some(t) = local(ts) {
                *st.tokens.entry(t.format("%Y-%m-%d").to_string()).or_insert(0) += n;
            }
        }
    }
    st
}

#[derive(Default, Clone)]
pub struct Activity {
    pub hours: HashMap<String, u32>, // "YYYY-MM-DD HH" -> prompts, all projects
    pub days: HashMap<String, u64>,  // "YYYY-MM-DD" -> prompts
    pub tokens: HashMap<String, u64>,
    pub projects: Vec<(String, u64, u64)>, // (root, prompts, output tokens) over the last 90 days, busiest first
}

/// Read every session log, reusing the cache for files that have not changed.
// shortcut: hours are cached in the time zone they were read in; delete ~/.cache/cove/activity.json after moving zones.
pub fn load(projects: &[Project]) -> Activity {
    let cache_path = cache_dir().join("activity.json");
    let old: HashMap<String, FileStat> = fs::read_to_string(&cache_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let mut new = HashMap::new();
    let mut a = Activity::default();
    let since = (Local::now() - Duration::days(90)).format("%Y-%m-%d").to_string();
    for p in projects {
        let (mut prompts, mut toks) = (0u64, 0u64);
        for d in &p.dirs {
            for f in scan::jsonl_files(d) {
                let key = f.to_string_lossy().into_owned();
                let size = fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
                let mtime = scan::mtime(&f);
                let st = match old.get(&key) {
                    Some(s) if s.size == size && s.mtime == mtime => s.clone(),
                    _ => FileStat { size, mtime, ..read_file(&f) },
                };
                for (h, n) in &st.hours {
                    *a.hours.entry(h.clone()).or_insert(0) += n;
                    *a.days.entry(h[..10].to_string()).or_insert(0) += *n as u64;
                    if h[..10] >= *since {
                        prompts += *n as u64;
                    }
                }
                for (d, n) in &st.tokens {
                    *a.tokens.entry(d.clone()).or_insert(0) += n;
                    if *d >= since {
                        toks += n;
                    }
                }
                new.insert(key, st);
            }
        }
        if prompts > 0 {
            a.projects.push((p.root.clone(), prompts, toks));
        }
    }
    a.projects.sort_by(|x, y| y.1.cmp(&x.1));
    let _ = fs::create_dir_all(cache_dir());
    let _ = fs::write(&cache_path, serde_json::to_string(&new).unwrap_or_default());
    a
}

// ---- GitHub -----------------------------------------------------------------

const GH_QUERY: &str = "query{viewer{login contributionsCollection{contributionCalendar{totalContributions weeks{contributionDays{date contributionCount}}}}}}";

/// (login, contributions per day) from the GitHub CLI; cached for an hour. Err is a sentence for the user.
pub fn github() -> Result<(String, HashMap<String, u64>), String> {
    let path = cache_dir().join("github.json");
    let fresh = fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|e| e.as_secs() < 3600);
    let raw = match fs::read_to_string(&path) {
        Ok(s) if fresh => s,
        cached => {
            if crate::launch::which("gh").is_none() {
                return Err("Install the GitHub CLI (gh) to show your GitHub contributions.".into());
            }
            let o = Command::new("gh").args(["api", "graphql", "-f", &format!("query={GH_QUERY}")]).stdin(Stdio::null()).stderr(Stdio::null()).output();
            match o {
                Ok(o) if o.status.success() => {
                    let s = String::from_utf8_lossy(&o.stdout).into_owned();
                    let _ = fs::create_dir_all(cache_dir());
                    let _ = fs::write(&path, &s);
                    s
                }
                _ => cached.map_err(|_| "Run gh auth login in a terminal to show your GitHub contributions.".to_string())?, // offline: keep the old copy
            }
        }
    };
    parse_github(&raw).ok_or_else(|| "GitHub sent an answer this app cannot read.".into())
}

fn parse_github(raw: &str) -> Option<(String, HashMap<String, u64>)> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let viewer = &v["data"]["viewer"];
    let mut days = HashMap::new();
    for w in viewer["contributionsCollection"]["contributionCalendar"]["weeks"].as_array()? {
        for d in w["contributionDays"].as_array()? {
            days.insert(d["date"].as_str()?.to_string(), d["contributionCount"].as_u64()?);
        }
    }
    Some((viewer["login"].as_str().unwrap_or("").to_string(), days))
}

// ---- chart data -------------------------------------------------------------

/// Levels 0–4 by rank among the non-zero values, like GitHub: one huge day does not flatten the rest.
pub fn levels(values: &[u64]) -> Vec<i32> {
    let mut nz: Vec<u64> = values.iter().copied().filter(|v| *v > 0).collect();
    nz.sort_unstable();
    let n = nz.len() as f64;
    values.iter().map(|&v| if v == 0 { 0 } else { (4.0 * nz.partition_point(|x| *x <= v) as f64 / n).ceil() as i32 }).collect()
}

pub struct Grid {
    pub levels: Vec<i32>, // column-major, 7 rows; -1 = after today
    pub tips: Vec<String>,
    pub months: Vec<(i32, String)>, // (column, label)
}

/// A year of days as 53 week columns, Sunday on top, ending with the current week.
pub fn year_grid(days: &HashMap<String, u64>, unit: &str, today: NaiveDate) -> Grid {
    let start = today - Duration::days(today.weekday().num_days_from_sunday() as i64 + 52 * 7);
    let dates: Vec<NaiveDate> = (0..53 * 7).map(|i| start + Duration::days(i)).collect();
    let vals: Vec<u64> = dates.iter().map(|d| days.get(&d.format("%Y-%m-%d").to_string()).copied().unwrap_or(0)).collect();
    let mut lv = levels(&vals);
    let mut tips = vec![];
    for (i, d) in dates.iter().enumerate() {
        if *d > today {
            lv[i] = -1;
        }
        let n = vals[i];
        let what = if n == 0 { format!("No {unit}s") } else if n == 1 { format!("1 {unit}") } else { format!("{n} {unit}s") };
        tips.push(format!("{what} · {}", d.format("%a %-d %b %Y")));
    }
    let mut months: Vec<(i32, String)> = vec![];
    for c in 0..53 {
        let m = dates[c * 7].month();
        if c == 0 || m != dates[(c - 1) * 7].month() {
            months.push((c as i32, dates[c * 7].format("%b").to_string()));
        }
    }
    if months.len() > 1 && months[1].0 - months[0].0 < 3 {
        months.remove(0); // a partial first month would collide with the next label
    }
    Grid { levels: lv, tips, months }
}

/// Weekday x hour over the last year: 24 columns, Sunday on top.
pub fn punch_card(hours: &HashMap<String, u32>, today: NaiveDate) -> Grid {
    let since = (today - Duration::days(365)).format("%Y-%m-%d").to_string();
    let mut m = [[0u64; 24]; 7];
    for (k, n) in hours {
        if k[..10] < *since {
            continue;
        }
        if let (Ok(d), Ok(h)) = (NaiveDate::parse_from_str(&k[..10], "%Y-%m-%d"), k[11..].parse::<usize>()) {
            m[d.weekday().num_days_from_sunday() as usize][h.min(23)] += *n as u64;
        }
    }
    let vals: Vec<u64> = (0..24 * 7).map(|i| m[i % 7][i / 7]).collect();
    const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    let tips = (0..24 * 7).map(|i| format!("{} prompts · {} {:02}:00–{:02}:00", vals[i], DAYS[i % 7], i / 7, (i / 7 + 1) % 24)).collect();
    let months = [0, 6, 12, 18].iter().map(|h| (*h, format!("{h:02}:00"))).collect();
    Grid { levels: levels(&vals), tips, months }
}

/// Prompts per hour of day over the last year.
pub fn by_hour(hours: &HashMap<String, u32>, today: NaiveDate) -> [u64; 24] {
    let since = (today - Duration::days(365)).format("%Y-%m-%d").to_string();
    let mut out = [0u64; 24];
    for (k, n) in hours {
        if k[..10] >= *since {
            if let Ok(h) = k[11..].parse::<usize>() {
                out[h.min(23)] += *n as u64;
            }
        }
    }
    out
}

pub struct Summary {
    pub total: u64,
    pub active: usize,
    pub streak: usize,
    pub longest: usize,
    pub best: Option<(NaiveDate, u64)>,
}

pub fn summary(days: &HashMap<String, u64>, today: NaiveDate) -> Summary {
    let get = |d: NaiveDate| days.get(&d.format("%Y-%m-%d").to_string()).copied().unwrap_or(0);
    let year: Vec<NaiveDate> = (0..365).rev().map(|i| today - Duration::days(i)).collect();
    let (mut run, mut longest) = (0, 0);
    let mut best: Option<(NaiveDate, u64)> = None;
    for d in &year {
        let n = get(*d);
        run = if n > 0 { run + 1 } else { 0 };
        longest = longest.max(run);
        if n > 0 && best.is_none_or(|b| n > b.1) {
            best = Some((*d, n));
        }
    }
    let start = if get(today) > 0 { 0 } else { 1 }; // today may still be empty
    let streak = (start..365).take_while(|i| get(today - Duration::days(*i)) > 0).count();
    Summary { total: year.iter().map(|d| get(*d)).sum(), active: year.iter().filter(|d| get(**d) > 0).count(), streak, longest, best }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn levels_use_quartiles_of_nonzero_days() {
        assert_eq!(levels(&[0, 1, 2, 3, 4, 100]), vec![0, 1, 2, 3, 4, 4]);
        assert_eq!(levels(&[0, 0]), vec![0, 0]);
    }
    #[test]
    fn year_grid_ends_this_week_and_blanks_the_future() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap(); // a Monday
        let mut days = HashMap::new();
        days.insert("2026-11-02".to_string(), 5);
        let g = year_grid(&days, "prompt", today);
        assert_eq!(g.levels.len(), 371);
        let i = 52 * 7 + 1; // last column, Monday row
        assert_eq!(g.levels[i], 4);
        assert_eq!(g.levels[i + 1], -1);
        assert!(g.tips[i].starts_with("5 prompts · Mon 2 Nov"));
    }
    #[test]
    fn summary_counts_streaks() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        let days: HashMap<String, u64> = [("2026-11-01", 2), ("2026-10-31", 9), ("2026-10-20", 1)].iter().map(|(d, n)| (d.to_string(), *n)).collect();
        let s = summary(&days, today);
        assert_eq!((s.total, s.active, s.streak, s.longest), (12, 3, 2, 2));
        assert_eq!(s.best.unwrap().1, 9);
    }
    #[test]
    fn github_answer_parses() {
        let raw = r#"{"data":{"viewer":{"login":"me","contributionsCollection":{"contributionCalendar":{"totalContributions":3,"weeks":[{"contributionDays":[{"date":"2026-11-01","contributionCount":3}]}]}}}}}"#;
        let (login, d) = parse_github(raw).unwrap();
        assert_eq!((login.as_str(), d["2026-11-01"]), ("me", 3));
    }
}
