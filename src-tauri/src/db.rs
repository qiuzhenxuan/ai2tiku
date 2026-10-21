use crate::models::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;

/// 本地存储：题库、记录、供应商、模型、设置
pub struct Db {
    conn: Mutex<Connection>,
}

/// 题干归一化：去掉空白、标点、题号标记，便于匹配
pub fn normalize(input: &str) -> String {
    const DROP: &str = "【】[]()（）{}.,，。、?？!！:：;；'\"“”‘’《》<>-_—+=*/\\|~`@#$%^&";
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_whitespace() || DROP.contains(ch) {
            continue;
        }
        out.extend(ch.to_lowercase());
    }
    // 去掉常见的题型前缀，避免同一道题因标记不同而无法命中
    for tag in ["单选题", "多选题", "判断题", "填空题", "简答题"] {
        if out.starts_with(tag) {
            out = out[tag.len()..].to_string();
        }
    }
    out
}

/// 归一化编辑距离相似度，范围 0..=1
pub fn similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let (x, y): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if x.is_empty() || y.is_empty() {
        return 0.0;
    }
    let (n, m) = (x.len(), y.len());
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for i in 1..=n {
        cur[0] = i;
        for j in 1..=m {
            let cost = if x[i - 1] == y[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    1.0 - prev[m] as f64 / n.max(m) as f64
}

/// 相似度 ≥ t 的必要条件：设查询串归一化长度为 n、库中题干为 m，
/// 则 t·n ≤ m ≤ n/t。用它做候选预筛，不会漏掉任何真正达标的行。
fn length_band(n: usize, t: f64) -> (i64, i64) {
    if t <= 0.0 {
        return (0, i64::MAX);
    }
    let lo = (t * n as f64).ceil() as i64;
    let hi = (n as f64 / t).floor() as i64;
    (lo.max(0), hi)
}

/// 去掉选项行首的编号（"A." / "A、xxx" / "(A) xxx" / "1. xxx" 等），返回正文。
/// 行首不是编号（例如 "ABC 开头的正文"）时原样返回。
fn strip_option_label(line: &str) -> &str {
    let s = line.trim();
    let body = s
        .strip_prefix('(')
        .or_else(|| s.strip_prefix('（'))
        .unwrap_or(s);
    let mut end = 0usize;
    for (i, c) in body.char_indices() {
        // 编号最多两个字符（A、1、10…），再往后就不是编号了
        if c.is_ascii_alphanumeric() && i < 2 {
            end = i + c.len_utf8();
            continue;
        }
        break;
    }
    if end == 0 {
        return s;
    }
    let rest = &body[end..];
    let mut it = rest.chars();
    match it.next() {
        Some(c) if matches!(c, '.' | '、' | ')' | '）' | ',' | '，' | ':' | '：') => it.as_str().trim(),
        Some(' ') | Some('\t') => rest.trim(),
        _ => s,
    }
}

/// 选项正文列表，按行顺序（已去编号、去标点空白、转小写）
fn option_texts(options: &str) -> Vec<String> {
    options
        .lines()
        .map(strip_option_label)
        .map(normalize)
        .filter(|s| !s.is_empty())
        .collect()
}

/// 选项签名：与顺序无关，用于判断两次请求是不是「同一组选项」（乱序也算同一组）
fn option_signature(options: &str) -> String {
    let mut texts = option_texts(options);
    texts.sort();
    texts.join("\u{1}")
}

/// 答案是否只是「选项字母」形式（"A"、"A#C"）；填空题、判断题的文本答案返回 false
fn is_letter_answer(answer: &str) -> bool {
    let mut any = false;
    for part in answer.split('#') {
        let p = part.trim();
        if p.is_empty() {
            return false;
        }
        match p.chars().next() {
            Some(c) if p.len() == 1 && c.is_ascii_alphabetic() => any = true,
            _ => return false,
        }
    }
    any
}

/// 把缓存里的字母答案换算成「当前这组选项」下的字母。
/// 选项乱序时同一道题的字母会变（北京从 A 挪到 B），必须按选项正文重新定位；
/// 选项内容/数量对不上则返回 None —— 表示这条缓存不能安全复用，宁可回退到 AI 也不能答错。
fn remap_letter_answer(stored_options: &str, current_options: &str, answer: &str) -> Option<String> {
    let stored = option_texts(stored_options);
    let current = option_texts(current_options);
    if stored.is_empty() || stored.len() != current.len() {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    for part in answer.split('#') {
        let c = part.trim().chars().next()?;
        if !c.is_ascii_alphabetic() {
            return None;
        }
        let text = stored.get((c.to_ascii_uppercase() as u8 - b'A') as usize)?;
        let idx = current.iter().position(|t| t == text)?;
        out.push(((b'A' + idx as u8) as char).to_string());
    }
    Some(out.join("#"))
}

/// 一条缓存能否用于当前这组选项；可以则返回该直接使用的答案
fn answer_for_options(q: &Question, current_options: &str) -> Option<String> {
    if !is_letter_answer(&q.answer) {
        // 填空题、判断题的答案是文本，与选项顺序无关
        return Some(q.answer.clone());
    }
    remap_letter_answer(&q.options, current_options, &q.answer)
}

pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

type R<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

impl Db {
    pub fn open(path: &std::path::Path) -> R<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(err)?;
        }
        let conn = Connection::open(path).map_err(err)?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS providers (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                name       TEXT NOT NULL,
                base_url   TEXT NOT NULL,
                api_key    TEXT NOT NULL DEFAULT '',
                enabled    INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS models (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_id  INTEGER NOT NULL,
                model        TEXT NOT NULL,
                display_name TEXT NOT NULL DEFAULT '',
                temperature  REAL NOT NULL DEFAULT 0.3,
                max_tokens   INTEGER NOT NULL DEFAULT 2048,
                enabled      INTEGER NOT NULL DEFAULT 1,
                is_default   INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS questions (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                title            TEXT NOT NULL,
                normalized_title TEXT NOT NULL,
                options          TEXT NOT NULL DEFAULT '',
                qtype            TEXT NOT NULL DEFAULT 'unknown',
                answer           TEXT NOT NULL,
                source           TEXT NOT NULL DEFAULT 'manual',
                provider         TEXT NOT NULL DEFAULT '',
                model            TEXT NOT NULL DEFAULT '',
                hits             INTEGER NOT NULL DEFAULT 0,
                created_at       TEXT NOT NULL,
                updated_at       TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_questions_norm ON questions(normalized_title);
            CREATE TABLE IF NOT EXISTS records (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                title             TEXT NOT NULL,
                answer            TEXT NOT NULL DEFAULT '',
                source            TEXT NOT NULL,
                provider          TEXT NOT NULL DEFAULT '',
                model             TEXT NOT NULL DEFAULT '',
                prompt_tokens     INTEGER NOT NULL DEFAULT 0,
                completion_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens      INTEGER NOT NULL DEFAULT 0,
                duration_ms       INTEGER NOT NULL DEFAULT 0,
                created_at        TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_records_created ON records(created_at);
            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )
        .map_err(err)?;

        // 一次性迁移：旧版默认 max_tokens 是 1024，模型偶尔先写一段分析就会撞上限被截断，
        // 导致整题漏答。新默认值改为 2048。
        // 只升级「仍等于旧默认值」的行，用户自己调过的数值不动；标记写进 settings，
        // 避免每次启动都覆盖用户之后手动改回 1024 的选择。
        let migrated: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'migrated_max_tokens_2048'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        if migrated.is_none() {
            conn.execute("UPDATE models SET max_tokens = 2048 WHERE max_tokens = 1024", [])
                .map_err(err)?;
            conn.execute(
                "INSERT OR REPLACE INTO settings(key, value) VALUES('migrated_max_tokens_2048', '1')",
                [],
            )
            .map_err(err)?;
        }

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> R<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|e| e.to_string())
    }

    // ---------- 设置 ----------

    pub fn settings(&self) -> R<Settings> {
        let conn = self.lock()?;
        let get = |key: &str| -> Option<String> {
            conn.query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()
        };
        let d = Settings::default();
        Ok(Settings {
            port: get("port")
                .and_then(|v| v.parse().ok())
                .unwrap_or(d.port),
            match_threshold: get("match_threshold")
                .and_then(|v| v.parse().ok())
                .unwrap_or(d.match_threshold),
            auto_cache: get("auto_cache")
                .map(|v| v != "0")
                .unwrap_or(d.auto_cache),
            ai_fallback: get("ai_fallback")
                .map(|v| v != "0")
                .unwrap_or(d.ai_fallback),
            system_prompt: get("system_prompt").unwrap_or(d.system_prompt),
            proxy_url: get("proxy_url").unwrap_or(d.proxy_url),
        })
    }

    pub fn save_settings(&self, s: &Settings) -> R<()> {
        let conn = self.lock()?;
        let pairs = [
            ("port", s.port.to_string()),
            ("match_threshold", s.match_threshold.to_string()),
            ("auto_cache", if s.auto_cache { "1" } else { "0" }.to_string()),
            ("ai_fallback", if s.ai_fallback { "1" } else { "0" }.to_string()),
            ("system_prompt", s.system_prompt.clone()),
            ("proxy_url", s.proxy_url.clone()),
        ];
        for (k, v) in pairs {
            conn.execute(
                "INSERT INTO settings(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![k, v],
            )
            .map_err(err)?;
        }
        Ok(())
    }

    // ---------- 界面偏好（主题色等） ----------
    // 存在 settings 表里，加 ui: 前缀，和业务设置共用一套落库逻辑。
    // 之所以不只放 localStorage：WebView 的 localStorage 按 origin 分开存，
    // 换一次运行方式（dev / 安装版）就取不到了，重启后主题色会丢。

    pub fn ui_pref(&self, key: &str) -> R<Option<String>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![format!("ui:{key}")],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(err)
    }

    pub fn set_ui_pref(&self, key: &str, value: &str) -> R<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![format!("ui:{key}"), value],
        )
        .map_err(err)?;
        Ok(())
    }

    // ---------- 供应商 ----------

    pub fn list_providers(&self) -> R<Vec<Provider>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT id, name, base_url, api_key, enabled, created_at FROM providers ORDER BY id")
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Provider {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    base_url: r.get(2)?,
                    api_key: r.get(3)?,
                    enabled: r.get(4)?,
                    created_at: r.get(5)?,
                })
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    pub fn save_provider(&self, input: &ProviderInput) -> R<i64> {
        let conn = self.lock()?;
        let enabled = input.enabled.unwrap_or(true) as i64;
        match input.id {
            Some(id) => {
                conn.execute(
                    "UPDATE providers SET name = ?1, base_url = ?2, api_key = ?3, enabled = ?4 WHERE id = ?5",
                    params![input.name, input.base_url, input.api_key, enabled, id],
                )
                .map_err(err)?;
                Ok(id)
            }
            None => {
                conn.execute(
                    "INSERT INTO providers(name, base_url, api_key, enabled, created_at) VALUES(?1,?2,?3,?4,?5)",
                    params![input.name, input.base_url, input.api_key, enabled, now()],
                )
                .map_err(err)?;
                Ok(conn.last_insert_rowid())
            }
        }
    }

    pub fn delete_provider(&self, id: i64) -> R<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM models WHERE provider_id = ?1", params![id])
            .map_err(err)?;
        conn.execute("DELETE FROM providers WHERE id = ?1", params![id])
            .map_err(err)?;
        Ok(())
    }

    // ---------- 模型 ----------

    pub fn list_models(&self) -> R<Vec<AiModel>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, provider_id, model, display_name, temperature, max_tokens, enabled, is_default
                 FROM models ORDER BY is_default DESC, id",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(AiModel {
                    id: r.get(0)?,
                    provider_id: r.get(1)?,
                    model: r.get(2)?,
                    display_name: r.get(3)?,
                    temperature: r.get(4)?,
                    max_tokens: r.get(5)?,
                    enabled: r.get(6)?,
                    is_default: r.get(7)?,
                })
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    pub fn save_model(&self, input: &ModelInput) -> R<i64> {
        let conn = self.lock()?;
        let display = input
            .display_name
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| input.model.clone());
        let temperature = input.temperature.unwrap_or(0.3);
        let max_tokens = input.max_tokens.unwrap_or(2048);
        let enabled = input.enabled.unwrap_or(true) as i64;
        let is_default = input.is_default.unwrap_or(false) as i64;
        let id = match input.id {
            Some(id) => {
                conn.execute(
                    "UPDATE models SET provider_id=?1, model=?2, display_name=?3, temperature=?4,
                        max_tokens=?5, enabled=?6, is_default=?7 WHERE id=?8",
                    params![
                        input.provider_id,
                        input.model,
                        display,
                        temperature,
                        max_tokens,
                        enabled,
                        is_default,
                        id
                    ],
                )
                .map_err(err)?;
                id
            }
            None => {
                conn.execute(
                    "INSERT INTO models(provider_id, model, display_name, temperature, max_tokens, enabled, is_default)
                     VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        input.provider_id,
                        input.model,
                        display,
                        temperature,
                        max_tokens,
                        enabled,
                        is_default
                    ],
                )
                .map_err(err)?;
                conn.last_insert_rowid()
            }
        };
        if is_default == 1 {
            conn.execute("UPDATE models SET is_default = 0 WHERE id <> ?1", params![id])
                .map_err(err)?;
        }
        Ok(id)
    }

    pub fn delete_model(&self, id: i64) -> R<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM models WHERE id = ?1", params![id])
            .map_err(err)?;
        Ok(())
    }

    /// 批量把供应商返回的模型写入题库；
    /// 已存在的跳过；若整张表还没有默认模型，把第一个新入库的设为默认。
    /// 返回本次新增的数量。
    pub fn upsert_models(&self, provider_id: i64, names: &[String]) -> R<usize> {
        if names.is_empty() {
            return Ok(0);
        }
        let conn = self.lock()?;
        let existing: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT model FROM models WHERE provider_id = ?1")
                .map_err(err)?;
            let rows = stmt
                .query_map(params![provider_id], |r| r.get::<_, String>(0))
                .map_err(err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err)?;
            rows
        };
        let has_default: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM models WHERE is_default = 1",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map_err(err)?
            > 0;

        let mut added = 0usize;
        let mut made_default = false;
        for name in names {
            if existing.iter().any(|e| e == name) {
                continue;
            }
            let is_default = if !has_default && !made_default {
                made_default = true;
                1
            } else {
                0
            };
            conn.execute(
                "INSERT INTO models(provider_id, model, display_name, temperature, max_tokens, enabled, is_default)
                 VALUES(?1,?2,?2,0.3,2048,1,?3)",
                params![provider_id, name, is_default],
            )
            .map_err(err)?;
            added += 1;
        }
        Ok(added)
    }

    /// 取默认启用的模型 + 其供应商
    pub fn active_model(&self) -> R<Option<(Provider, AiModel)>> {
        let conn = self.lock()?;
        let row = conn
            .query_row(
                "SELECT m.id, m.provider_id, m.model, m.display_name, m.temperature, m.max_tokens,
                        m.enabled, m.is_default,
                        p.id, p.name, p.base_url, p.api_key, p.enabled, p.created_at
                 FROM models m JOIN providers p ON p.id = m.provider_id
                 WHERE m.enabled = 1 AND p.enabled = 1
                 ORDER BY m.is_default DESC, m.id LIMIT 1",
                [],
                |r| {
                    Ok((
                        AiModel {
                            id: r.get(0)?,
                            provider_id: r.get(1)?,
                            model: r.get(2)?,
                            display_name: r.get(3)?,
                            temperature: r.get(4)?,
                            max_tokens: r.get(5)?,
                            enabled: r.get(6)?,
                            is_default: r.get(7)?,
                        },
                        Provider {
                            id: r.get(8)?,
                            name: r.get(9)?,
                            base_url: r.get(10)?,
                            api_key: r.get(11)?,
                            enabled: r.get(12)?,
                            created_at: r.get(13)?,
                        },
                    ))
                },
            )
            .optional()
            .map_err(err)?;
        Ok(row.map(|(m, p)| (p, m)))
    }

    // ---------- 题库 ----------

    fn row_to_question(r: &rusqlite::Row) -> rusqlite::Result<Question> {
        Ok(Question {
            id: r.get(0)?,
            title: r.get(1)?,
            options: r.get(2)?,
            qtype: r.get(3)?,
            answer: r.get(4)?,
            source: r.get(5)?,
            provider: r.get(6)?,
            model: r.get(7)?,
            hits: r.get(8)?,
            created_at: r.get(9)?,
            updated_at: r.get(10)?,
        })
    }

    const Q_COLS: &'static str = "id, title, options, qtype, answer, source, provider, model, hits, created_at, updated_at";

    /// 本地题库匹配：先精确命中，再模糊匹配取最高相似度。
    ///
    /// 选项参与判定（这一步很关键）：
    /// - 题干归一化后相同、但选项不同的题不算同一道题 —— 同一句「下列说法正确的是」
    ///   可以出现在很多道不同的题里，只看题干必然答错；
    /// - 命中时如果选项只是**顺序被换过**，答案里的字母按选项正文重新对新位置
    ///   （北京从 A 挪到 B，缓存里的 "A" 要变成 "B"）；
    /// - 选项内容对不上就放弃这条缓存，宁可回退 AI 也不能答错。
    ///
    /// 返回的 `Question.answer` 已经是「对应当前选项」的答案。
    pub fn find_local(&self, title: &str, options: &str, threshold: f64) -> R<Option<(Question, f64)>> {
        let key = normalize(title);
        if key.is_empty() {
            return Ok(None);
        }
        let conn = self.lock()?;

        // 1) 题干归一化后完全相同
        let sql_same = format!(
            "SELECT {} FROM questions WHERE normalized_title = ?1",
            Self::Q_COLS
        );
        let same = {
            let mut stmt = conn.prepare(&sql_same).map_err(err)?;
            let rows = stmt
                .query_map(params![key], |r| Self::row_to_question(r))
                .map_err(err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err)?;
            rows
        };
        for q in same {
            if let Some(answer) = answer_for_options(&q, options) {
                let mut hit = q;
                hit.answer = answer;
                return Ok(Some((hit, 1.0)));
            }
        }

        // 2) 模糊匹配：先按长度区间筛出「可能达到阈值」的候选（不漏真命中），再逐条算归一化编辑距离。
        //
        //    模糊这条通道只对**选择题**开放，且要求选项集合完全一致，原因是：
        //    「人生目的是人生观的核心…」与「人生态度是人生观的核心…」只差两个字，
        //    相似度 0.917 却是一对答案相反的题；判断题的选项永远是「对/错」，
        //    给不了任何区分信号。只看题干去容错，必然答错，所以宁可 miss 交给 AI。
        let query_sig = option_signature(options);
        let n = key.chars().count();
        let (lo, hi) = length_band(n, threshold);
        let sql_near = format!(
            "SELECT {} FROM questions
             WHERE length(normalized_title) BETWEEN ?1 AND ?2
             ORDER BY abs(length(normalized_title) - ?3)
             LIMIT 2000",
            Self::Q_COLS
        );
        let candidates = {
            let mut stmt = conn.prepare(&sql_near).map_err(err)?;
            let rows = stmt
                .query_map(params![lo, hi, n as i64], |r| Self::row_to_question(r))
                .map_err(err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err)?;
            rows
        };

        let mut best: Option<(Question, f64)> = None;
        for q in candidates {
            if !is_letter_answer(&q.answer) || query_sig.is_empty() {
                continue;
            }
            if option_signature(&q.options) != query_sig {
                continue;
            }
            let score = similarity(&key, &normalize(&q.title));
            if score < threshold {
                continue;
            }
            let Some(answer) = remap_letter_answer(&q.options, options, &q.answer) else {
                continue;
            };
            if best.as_ref().map(|(_, b)| score > *b).unwrap_or(true) {
                let mut hit = q;
                hit.answer = answer;
                best = Some((hit, score));
            }
        }
        Ok(best)
    }

    pub fn list_questions(
        &self,
        keyword: Option<String>,
        source: Option<String>,
        page: i64,
        page_size: i64,
    ) -> R<Page<Question>> {
        let conn = self.lock()?;
        let kw = keyword.unwrap_or_default();
        let src = source.unwrap_or_default();
        let like = format!("%{}%", kw);
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM questions
                 WHERE (?1 = '' OR title LIKE ?2) AND (?3 = '' OR source = ?3)",
                params![kw, like, src],
                |r| r.get(0),
            )
            .map_err(err)?;
        let sql = format!(
            "SELECT {} FROM questions
             WHERE (?1 = '' OR title LIKE ?2) AND (?3 = '' OR source = ?3)
             ORDER BY id DESC LIMIT ?4 OFFSET ?5",
            Self::Q_COLS
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let items = stmt
            .query_map(
                params![kw, like, src, page_size, (page - 1).max(0) * page_size],
                |r| Self::row_to_question(r),
            )
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(Page {
            items,
            total,
            page,
            page_size,
        })
    }

    pub fn save_question(&self, input: &QuestionInput) -> R<i64> {
        let conn = self.lock()?;
        let options = input.options.clone().unwrap_or_default();
        let qtype = input
            .qtype
            .clone()
            .unwrap_or_else(|| "unknown".into());
        let key = normalize(&input.title);
        match input.id {
            Some(id) => {
                conn.execute(
                    "UPDATE questions SET title=?1, normalized_title=?2, options=?3, qtype=?4,
                        answer=?5, source='manual', updated_at=?6 WHERE id=?7",
                    params![input.title, key, options, qtype, input.answer, now(), id],
                )
                .map_err(err)?;
                Ok(id)
            }
            None => {
                conn.execute(
                    "INSERT INTO questions(title, normalized_title, options, qtype, answer, source, provider, model, hits, created_at, updated_at)
                     VALUES(?1,?2,?3,?4,?5,'manual','','',0,?6,?6)",
                    params![input.title, key, options, qtype, input.answer, now()],
                )
                .map_err(err)?;
                Ok(conn.last_insert_rowid())
            }
        }
    }

    /// AI 答完题后写入缓存
    pub fn cache_question(
        &self,
        title: &str,
        options: &str,
        qtype: &str,
        answer: &str,
        provider: &str,
        model: &str,
    ) -> R<i64> {
        let conn = self.lock()?;
        let key = normalize(title);
        // 去重键必须是「题干 + 选项」：题干相同但选项不同的其实是另一道题，
        // 只按题干去重会把上一道题的答案覆盖掉（正是缓存答错的来源之一）。
        let sig = option_signature(options);
        let existing: Option<i64> = {
            let mut stmt = conn
                .prepare("SELECT id, options FROM questions WHERE normalized_title = ?1")
                .map_err(err)?;
            let rows = stmt
                .query_map(params![key], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(err)?;
            let mut found = None;
            for row in rows {
                let (id, opts) = row.map_err(err)?;
                if option_signature(&opts) == sig {
                    found = Some(id);
                    break;
                }
            }
            found
        };
        match existing {
            Some(id) => {
                // 选项一并写回：同一组选项只是顺序变了时，让库里存的顺序与答案字母保持一致
                conn.execute(
                    "UPDATE questions SET options=?1, answer=?2, provider=?3, model=?4, updated_at=?5 WHERE id=?6",
                    params![options, answer, provider, model, now(), id],
                )
                .map_err(err)?;
                Ok(id)
            }
            None => {
                conn.execute(
                    "INSERT INTO questions(title, normalized_title, options, qtype, answer, source, provider, model, hits, created_at, updated_at)
                     VALUES(?1,?2,?3,?4,?5,'ai',?6,?7,0,?8,?8)",
                    params![title, key, options, qtype, answer, provider, model, now()],
                )
                .map_err(err)?;
                Ok(conn.last_insert_rowid())
            }
        }
    }

    pub fn delete_question(&self, id: i64) -> R<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM questions WHERE id = ?1", params![id])
            .map_err(err)?;
        Ok(())
    }

    pub fn clear_questions(&self) -> R<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM questions", [])
            .map_err(err)?;
        Ok(())
    }

    pub fn bump_hits(&self, id: i64) -> R<()> {
        let conn = self.lock()?;
        conn.execute("UPDATE questions SET hits = hits + 1 WHERE id = ?1", params![id])
            .map_err(err)?;
        Ok(())
    }

    // ---------- 记录 ----------

    #[allow(clippy::too_many_arguments)]
    pub fn add_record(
        &self,
        title: &str,
        answer: &str,
        source: &str,
        provider: &str,
        model: &str,
        prompt_tokens: i64,
        completion_tokens: i64,
        duration_ms: i64,
    ) -> R<i64> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO records(title, answer, source, provider, model, prompt_tokens, completion_tokens, total_tokens, duration_ms, created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                title,
                answer,
                source,
                provider,
                model,
                prompt_tokens,
                completion_tokens,
                prompt_tokens + completion_tokens,
                duration_ms,
                now()
            ],
        )
        .map_err(err)?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_records(&self, page: i64, page_size: i64) -> R<Page<SearchRecord>> {
        let conn = self.lock()?;
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM records", [], |r| r.get(0))
            .map_err(err)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, answer, source, provider, model, prompt_tokens, completion_tokens,
                        total_tokens, duration_ms, created_at
                 FROM records ORDER BY id DESC LIMIT ?1 OFFSET ?2",
            )
            .map_err(err)?;
        let items = stmt
            .query_map(params![page_size, (page - 1).max(0) * page_size], |r| {
                Ok(SearchRecord {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    answer: r.get(2)?,
                    source: r.get(3)?,
                    provider: r.get(4)?,
                    model: r.get(5)?,
                    prompt_tokens: r.get(6)?,
                    completion_tokens: r.get(7)?,
                    total_tokens: r.get(8)?,
                    duration_ms: r.get(9)?,
                    created_at: r.get(10)?,
                })
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(Page {
            items,
            total,
            page,
            page_size,
        })
    }

    pub fn clear_records(&self) -> R<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM records", []).map_err(err)?;
        Ok(())
    }

    // ---------- 统计 ----------

    pub fn stats(&self) -> R<Stats> {
        let conn = self.lock()?;
        let question_total: i64 = conn
            .query_row("SELECT COUNT(*) FROM questions", [], |r| r.get(0))
            .map_err(err)?;
        let (cache_hits, ai_calls): (i64, i64) = conn
            .query_row(
                "SELECT COALESCE(SUM(source='cache'),0), COALESCE(SUM(source='ai'),0) FROM records",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(err)?;
        let (pt, ct, tt): (i64, i64, i64) = conn
            .query_row(
                "SELECT COALESCE(SUM(prompt_tokens),0), COALESCE(SUM(completion_tokens),0),
                        COALESCE(SUM(total_tokens),0) FROM records",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(err)?;
        let day = today();
        let today_like = format!("{}%", day);
        let (today_count, today_tokens): (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(total_tokens),0) FROM records WHERE created_at LIKE ?1",
                params![today_like],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(err)?;

        // 近 14 天趋势
        let from = (chrono::Local::now() - chrono::Duration::days(13))
            .format("%Y-%m-%d")
            .to_string();
        let mut stmt = conn
            .prepare(
                "SELECT substr(created_at,1,10) d, COUNT(*), COALESCE(SUM(total_tokens),0)
                 FROM records WHERE created_at >= ?1 GROUP BY d ORDER BY d",
            )
            .map_err(err)?;
        let found: Vec<TrendPoint> = stmt
            .query_map(params![from], |r| {
                Ok(TrendPoint {
                    date: r.get(0)?,
                    count: r.get(1)?,
                    tokens: r.get(2)?,
                })
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;

        // 补齐缺失日期，让折线连续
        let mut trend = Vec::with_capacity(14);
        for i in (0..14).rev() {
            let d = (chrono::Local::now() - chrono::Duration::days(i))
                .format("%Y-%m-%d")
                .to_string();
            match found.iter().find(|p| p.date == d) {
                Some(p) => trend.push(p.clone()),
                None => trend.push(TrendPoint {
                    date: d,
                    count: 0,
                    tokens: 0,
                }),
            }
        }

        let mut stmt = conn
            .prepare(
                "SELECT model, COUNT(*), COALESCE(SUM(total_tokens),0) FROM records
                 WHERE source = 'ai' AND model <> '' GROUP BY model ORDER BY 2 DESC LIMIT 10",
            )
            .map_err(err)?;
        let model_usage = stmt
            .query_map([], |r| {
                Ok(ModelUsage {
                    model: r.get(0)?,
                    count: r.get(1)?,
                    tokens: r.get(2)?,
                })
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;

        let calls = cache_hits + ai_calls;
        Ok(Stats {
            question_total,
            cache_hits,
            ai_calls,
            hit_rate: if calls > 0 {
                cache_hits as f64 / calls as f64
            } else {
                0.0
            },
            prompt_tokens: pt,
            completion_tokens: ct,
            total_tokens: tt,
            today_count,
            today_tokens,
            trend,
            model_usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(name: &str) -> Db {
        let path = std::env::temp_dir().join(format!("ai2tiku-test-{}.db", name));
        let _ = std::fs::remove_file(&path);
        Db::open(&path).expect("建库失败")
    }

    fn manual(db: &Db, title: &str, options: &str, answer: &str) {
        db.save_question(&QuestionInput {
            id: None,
            title: title.into(),
            options: Some(options.into()),
            qtype: Some("single".into()),
            answer: answer.into(),
        })
        .expect("写入失败");
    }

    fn cached(db: &Db, title: &str, options: &str, answer: &str) {
        db.cache_question(title, options, "single", answer, "t", "t")
            .expect("写入失败");
    }

    const OPT_A: &str = "A. 甲\nB. 乙\nC. 丙\nD. 丁";
    const OPT_B: &str = "A. 苹果\nB. 香蕉\nC. 橘子\nD. 葡萄";

    /// 同一句题干配不同选项的两道题，缓存不应互相覆盖、也不应错答
    #[test]
    fn same_stem_should_not_reuse_other_options_answer() {
        let db = temp_db("stem");
        manual(&db, "下列说法正确的是", OPT_A, "A");
        cached(&db, "下列说法正确的是", OPT_B, "C");
        let rows = db.list_questions(None, None, 1, 10).unwrap().items;
        for r in &rows {
            println!("库中 → options={:?} answer={:?}", r.options, r.answer);
        }
        assert_eq!(rows.len(), 2, "两道不同的题应当各占一行，不能被合并");
        let hit = db.find_local("下列说法正确的是", OPT_A, 0.85).unwrap();
        let got = hit.as_ref().map(|(q, _)| q.answer.clone());
        println!("按 OPT_A 的选项查询 → 命中答案 {:?}（应为 A）", got);
        assert_eq!(got.as_deref(), Some("A"));
    }

    /// 选项乱序：字母答案要按选项正文重新定位
    #[test]
    fn shuffled_options_should_remap_letters() {
        let db = temp_db("shuffle");
        let stored = "A. 甲\nB. 乙\nC. 丙\nD. 丁";
        let current = "A. 丙\nB. 甲\nC. 丁\nD. 乙";
        manual(&db, "下面说法正确的有", stored, "A#C");
        let hit = db.find_local("下面说法正确的有", current, 0.85).unwrap();
        let got = hit.map(|(q, _)| q.answer);
        println!("乱序后 → {:?}（甲挪到 B、丙挪到 A，应为 B#A）", got);
        assert_eq!(got.as_deref(), Some("B#A"));
    }

    /// 选项内容对不上：宁可不命中，也不能把字母答案原样答出去
    #[test]
    fn different_options_should_not_hit() {
        let db = temp_db("diff");
        manual(&db, "下列说法正确的是", "A. 甲\nB. 乙", "A");
        let hit = db.find_local("下列说法正确的是", "A. 苹果\nB. 香蕉", 0.85).unwrap();
        println!("选项完全不同 → {:?}（应为 None）", hit.as_ref().map(|(q, _)| q.answer.clone()));
        assert!(hit.is_none());
    }

    /// 选择题：题干只差一点点、选项集合一致时，模糊匹配应该命中
    #[test]
    fn fuzzy_should_match_choice_with_same_options() {
        let db = temp_db("fuzzy-choice");
        let options = "个体对自己生命存在的肯定\n个人的社会存在\n个人通过劳动和创造,对社会所作的贡献\n社会对个人的尊重和满足";
        manual(&db, "下列关于社会价值正确的说法是", options, "C");
        let hit = db
            .find_local("下列关于社会价值正确的说法为", options, 0.85)
            .unwrap();
        println!("近义题干 + 同选项 → {:?}", hit.as_ref().map(|(q, s)| (q.answer.clone(), *s)));
        assert_eq!(hit.map(|(q, _)| q.answer).as_deref(), Some("C"));
    }

    /// 判断题近邻：只差两个字的题干相似度能到 0.9，但答案是相反的，绝不能被模糊匹配吃掉
    #[test]
    fn near_neighbour_judgement_must_not_cross_match() {
        let db = temp_db("near");
        manual(&db, "人生目的是人生观的核心,在人生实践中具有重要的作用。", "对\n错", "对");
        manual(&db, "人生态度是人生观的核心,在人生实践中具有重要的作用。", "对\n错", "错");

        let exact = db
            .find_local("人生态度是人生观的核心,在人生实践中具有重要的作用。", "对\n错", 0.85)
            .unwrap();
        assert_eq!(exact.map(|(q, _)| q.answer).as_deref(), Some("错"), "逐字命中应取到本题答案");

        let near = db
            .find_local("人生态度是人生观的核心,在人生实践中具有非常重要的作用。", "对\n错", 0.85)
            .unwrap();
        println!("近邻题干 → {:?}（应为 None，交给 AI）", near.as_ref().map(|(q, _)| q.answer.clone()));
        assert!(near.is_none());
    }

    /// 判断题/填空题只有题干一个信号，模糊匹配一律不做（宁可 miss 交给 AI）
    #[test]
    fn text_answer_without_options_should_not_fuzzy_match() {
        let db = temp_db("fuzzy-text");
        let stored = "网络协议的三要素是语法、语义和时序";
        let query = "网络协议的三要素是语法、语义和顺序";
        manual(&db, stored, "", "对");
        let score = similarity(&normalize(query), &normalize(stored));
        println!("相似度 = {:.3} ≥ 阈值 0.85，但答案是文本、没有选项可交叉验证", score);
        assert!(score >= 0.85);
        let hit = db.find_local(query, "", 0.85).unwrap();
        println!("查询 → {:?}（应为 None）", hit.as_ref().map(|(q, _)| q.answer.clone()));
        assert!(hit.is_none());
    }

    /// 反方向验证：真正的同一道题重复缓存时必须原地更新，不能每答一次多一行
    #[test]
    fn caching_same_question_twice_should_update_in_place() {
        let db = temp_db("dedupe");
        let options = "A. 甲\nB. 乙\nC. 丙";
        let first = db.cache_question("下列说法", options, "single", "A", "t", "t").unwrap();
        let second = db.cache_question("下列说法", options, "single", "B", "t", "t").unwrap();
        let n = db.list_questions(None, None, 1, 10).unwrap().items.len();
        println!("同题同选项缓存两次 → 行数={}，id={}/{}", n, first, second);
        assert_eq!(n, 1, "同题同选项不应产生重复行");
        assert_eq!(first, second, "应更新原来那一行");
        let hit = db.find_local("下列说法", options, 0.85).unwrap().unwrap().0;
        assert_eq!(hit.answer, "B", "答案应被更新");
    }

    /// 同一组选项只是顺序变了：仍然算同一道题，且库里存的选项顺序要与答案字母保持一致
    #[test]
    fn shuffled_same_options_should_dedupe_and_keep_order_consistent() {
        let db = temp_db("dedupe-shuffle");
        let a = "A. 甲\nB. 乙\nC. 丙";
        let b = "A. 丙\nB. 甲\nC. 乙";
        let first = db.cache_question("下列说法", a, "single", "B", "t", "t").unwrap();
        let second = db.cache_question("下列说法", b, "single", "A", "t", "t").unwrap();
        println!("选项乱序后再缓存 → 行数={}", db.list_questions(None, None, 1, 10).unwrap().items.len());
        assert_eq!(first, second, "同一组选项只是顺序变了，应仍是同一行");
        assert_eq!(db.list_questions(None, None, 1, 10).unwrap().items.len(), 1);

        let same = db.find_local("下列说法", b, 0.85).unwrap().unwrap().0;
        assert_eq!(same.answer, "A");
        // 换回最早那个顺序查询：丙 从 A 位挪到了 C 位，字母应跟着走
        let back = db.find_local("下列说法", a, 0.85).unwrap().unwrap().0;
        println!("换回原选项顺序 → {}（丙 在 C 位，应为 C）", back.answer);
        assert_eq!(back.answer, "C");
    }
}
