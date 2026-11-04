//! What the desktop offers: dark mode, accent colour, whether it blurs behind windows, the clipboard.
use std::process::{Command, Stdio};

fn out(cmd: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(cmd).args(args).stderr(Stdio::null()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The desktop's light/dark preference, None if unknown.
pub fn system_dark() -> Option<bool> {
    if cfg!(windows) {
        let s = out("reg", &["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "/v", "AppsUseLightTheme"])?;
        return Some(s.contains("0x0"));
    }
    if cfg!(target_os = "macos") {
        return Some(out("defaults", &["read", "-g", "AppleInterfaceStyle"]).is_some_and(|s| s.contains("Dark")));
    }
    // XDG desktop portal: 1 = prefer dark, 2 = prefer light, 0 = no preference
    if let Some(s) = out("gdbus", &["call", "--session", "--dest", "org.freedesktop.portal.Desktop", "--object-path", "/org/freedesktop/portal/desktop", "--method", "org.freedesktop.portal.Settings.ReadOne", "org.freedesktop.appearance", "color-scheme"]) {
        if s.contains("uint32 1") {
            return Some(true);
        }
        if s.contains("uint32 2") {
            return Some(false);
        }
    }
    let s = out("gsettings", &["get", "org.gnome.desktop.interface", "color-scheme"])?;
    Some(s.contains("dark"))
}

fn hex(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim().trim_start_matches('#');
    if s.len() < 6 {
        return None;
    }
    Some((u8::from_str_radix(&s[0..2], 16).ok()?, u8::from_str_radix(&s[2..4], 16).ok()?, u8::from_str_radix(&s[4..6], 16).ok()?))
}

/// The desktop accent colour: Noctalia's primary, the XDG portal's accent, or the Windows accent.
pub fn accent() -> Option<(u8, u8, u8)> {
    if let Some(home) = dirs::config_dir() {
        if let Ok(s) = std::fs::read_to_string(home.join("noctalia/colors.json")) {
            if let Some(c) = serde_json::from_str::<serde_json::Value>(&s).ok().and_then(|v| v["mPrimary"].as_str().and_then(hex)) {
                return Some(c);
            }
        }
    }
    if cfg!(windows) {
        let s = out("reg", &["query", r"HKCU\Software\Microsoft\Windows\DWM", "/v", "AccentColor"])?;
        let v = u32::from_str_radix(s.split("0x").nth(1)?.trim(), 16).ok()?; // ABGR
        return Some(((v & 0xff) as u8, (v >> 8 & 0xff) as u8, (v >> 16 & 0xff) as u8));
    }
    // portal: (<(dd d)>) as three doubles 0..1
    let s = out("gdbus", &["call", "--session", "--dest", "org.freedesktop.portal.Desktop", "--object-path", "/org/freedesktop/portal/desktop", "--method", "org.freedesktop.portal.Settings.ReadOne", "org.freedesktop.appearance", "accent-color"])?;
    let nums: Vec<f64> = s.split(|c: char| !(c.is_ascii_digit() || c == '.')).filter_map(|x| x.parse().ok()).collect();
    if nums.len() >= 3 && nums[..3].iter().all(|x| (0.0..=1.0).contains(x)) {
        return Some(((nums[0] * 255.0) as u8, (nums[1] * 255.0) as u8, (nums[2] * 255.0) as u8));
    }
    None
}

pub fn luminance((r, g, b): (u8, u8, u8)) -> f64 {
    let lin = |c: u8| {
        let c = c as f64 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

pub fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

pub fn mix(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let m = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// Name of a Linux compositor known to blur behind translucent windows, if one is running.
pub fn blur_compositor() -> Option<&'static str> {
    if std::env::var_os("NIRI_SOCKET").is_some() {
        return Some("niri");
    }
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return Some("Hyprland");
    }
    let de = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
    if de.contains("kde") {
        return Some("KDE Plasma");
    }
    None
}

/// Window tint opacity for a Glass setting.
pub fn glass_alpha(setting: &str) -> f32 {
    let blur = cfg!(any(windows, target_os = "macos")) || blur_compositor().is_some();
    match setting {
        "clear" => 0.46,
        "frosted" => 0.64,
        "solid" => 0.97,
        _ => if blur { 0.64 } else { 0.97 }, // without blur a see-through window makes text unreadable
    }
}

pub fn glass_note() -> String {
    if cfg!(windows) {
        "Glass uses Windows Mica.".into()
    } else if cfg!(target_os = "macos") {
        "Glass uses macOS vibrancy.".into()
    } else {
        match blur_compositor() {
            Some(c) => format!("{c} blurs the desktop behind this window. Clear shows more of it, Solid none."),
            None => "Glass needs a compositor that blurs behind windows (niri, KDE Plasma, Hyprland). Auto uses a solid window here.".into(),
        }
    }
}

pub fn copy(text: &str) -> bool {
    let cmds: &[(&str, &[&str])] = if cfg!(windows) {
        &[("clip", &[])]
    } else if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else {
        &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"]), ("xsel", &["-ib"])]
    };
    for (c, a) in cmds {
        if let Ok(mut ch) = Command::new(c).args(*a).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            use std::io::Write;
            if let Some(mut i) = ch.stdin.take() {
                let _ = i.write_all(text.as_bytes());
            }
            return ch.wait().is_ok_and(|s| s.success());
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contrast_of_black_on_white_is_21() {
        assert!((contrast((0, 0, 0), (255, 255, 255)) - 21.0).abs() < 0.01);
    }
}

/// The system's monospace family by name (code blocks and tables in chat replies).
pub fn mono_font() -> String {
    if cfg!(target_os = "macos") {
        return "Menlo".into();
    }
    if cfg!(windows) {
        return "Consolas".into();
    }
    std::process::Command::new("fc-match")
        .args(["-f", "%{family[0]}", "monospace"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .filter(|f| !f.trim().is_empty())
        .unwrap_or_else(|| "DejaVu Sans Mono".into())
}
