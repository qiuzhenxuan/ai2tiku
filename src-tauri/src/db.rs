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
                max_tokens   INTEGER NOT NULL DEFAULT 1024,
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
        let max_tokens = input.max_tokens.unwrap_or(1024);
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
                 VALUES(?1,?2,?2,0.3,1024,1,?3)",
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

    /// 本地题库匹配：先精确命中，再模糊匹配取最高相似度
    pub fn find_local(&self, title: &str, threshold: f64) -> R<Option<(Question, f64)>> {
        let key = normalize(title);
        if key.is_empty() {
            return Ok(None);
        }
        let conn = self.lock()?;
        let sql_exact = format!(
            "SELECT {} FROM questions WHERE normalized_title = ?1 LIMIT 1",
            Self::Q_COLS
        );
        if let Some(q) = conn
            .query_row(&sql_exact, params![key], |r| Self::row_to_question(r))
            .optional()
            .map_err(err)?
        {
            return Ok(Some((q, 1.0)));
        }
        let sql_like = format!(
            "SELECT {} FROM questions
             WHERE normalized_title LIKE '%' || ?1 || '%' OR ?1 LIKE '%' || normalized_title || '%'
             LIMIT 300",
            Self::Q_COLS
        );
        let mut stmt = conn.prepare(&sql_like).map_err(err)?;
        let candidates = stmt
            .query_map(params![key], |r| Self::row_to_question(r))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(candidates
            .into_iter()
            .map(|q| {
                let s = similarity(&key, &normalize(&q.title));
                (q, s)
            })
            .filter(|(_, s)| *s >= threshold)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)))
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
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM questions WHERE normalized_title = ?1 LIMIT 1",
                params![key],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        match existing {
            Some(id) => {
                conn.execute(
                    "UPDATE questions SET answer=?1, provider=?2, model=?3, updated_at=?4 WHERE id=?5",
                    params![answer, provider, model, now(), id],
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
