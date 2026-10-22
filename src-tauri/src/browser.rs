use crate::models::BrowserInfo;
use std::path::PathBuf;

fn edge_executable() -> Option<PathBuf> {
    let mut paths = vec![
        PathBuf::from(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"),
        PathBuf::from(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"),
    ];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        paths.push(PathBuf::from(local).join(r"Microsoft\Edge\Application\msedge.exe"));
    }
    for root in ["HKCU", "HKLM"] {
        if let Some(path) = reg_value(
            &format!(r"{root}\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe"),
            "",
        ) {
            paths.push(PathBuf::from(path.trim_matches('"')));
        }
    }
    paths.into_iter().find(|p| p.is_file())
}

pub fn info() -> BrowserInfo {
    let prog_id = reg_value(
        r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice",
        "ProgId",
    )
    .unwrap_or_default();
    let edge_default = prog_id.starts_with("MSEdge");
    let default_browser = if edge_default {
        "Edge"
    } else if prog_id.starts_with("ChromeHTML") {
        "Chrome"
    } else if prog_id.starts_with("Firefox") {
        "Firefox"
    } else if prog_id.is_empty() {
        "未知"
    } else {
        "其他浏览器"
    };
    BrowserInfo {
        edge_installed: edge_executable().is_some(),
        edge_default,
        default_browser: default_browser.into(),
    }
}

fn validate_url(url: &str) -> Result<(), String> {
    if url == "edge://extensions/" {
        return Ok(());
    }
    let parsed = reqwest::Url::parse(url).map_err(|_| "链接格式无效".to_string())?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err("部署页仅支持 HTTPS 网页和 Edge 扩展管理页".into());
    }
    Ok(())
}

/// 全部部署步骤明确使用 Edge，不依赖系统默认浏览器，不修改浏览器设置。
pub fn open_url(url: &str) -> Result<(), String> {
    validate_url(url)?;
    let executable = edge_executable().ok_or("未检测到 Edge，请先安装或修复 Edge 后重试")?;
    let mut command = std::process::Command::new(executable);
    command.arg(url);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
        .spawn()
        .map_err(|e| format!("打开 Edge 失败：{e}"))?;
    Ok(())
}

#[cfg(windows)]
fn reg_value(path: &str, value: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new("reg");
    cmd.args(["query", path]);
    if value.is_empty() {
        cmd.arg("/ve");
    } else {
        cmd.args(["/v", value]);
    }
    let output = cmd.creation_flags(0x0800_0000).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            line.split_once("REG_SZ")
                .map(|(_, value)| value.trim().to_string())
        })
}

#[cfg(not(windows))]
fn reg_value(_path: &str, _value: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deployment_urls_are_passed_as_urls_not_shell_commands() {
        for url in [
            "https://docs.scriptcat.org/docs/use/open-dev/?browser=edge",
            "https://github.com/ocsjs/ocsjs/releases/latest",
            "edge://extensions/",
        ] {
            assert!(validate_url(url).is_ok());
        }
        for url in [
            "edge://settings/",
            "file:///C:/Windows/cmd.exe",
            "javascript:alert(1)",
            "--remote-debugging-port=1234",
            "not a URL",
        ] {
            assert!(validate_url(url).is_err());
        }
    }
}
