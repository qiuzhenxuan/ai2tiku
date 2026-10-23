//! Question-only, versioned JSON backups. Credentials and request records never enter this format.
use crate::{
    db::{self, Db},
    models::*,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_QUESTIONS: usize = 100_000;
type R<T> = Result<T, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionBackup {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub questions: Vec<Question>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub new_count: usize,
    pub duplicates: usize,
    pub conflicts: usize,
    pub file_duplicates: usize,
    pub file_conflicts: usize,
    pub existing: usize,
    pub errors: Vec<String>,
    pub snapshot: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub total: usize,
    pub backup_path: String,
}

fn question_key(question: &Question) -> (String, String, String) {
    (
        db::normalize(&question.title),
        question.qtype.clone(),
        db::option_signature(&question.options),
    )
}

fn same_answer(stored: &Question, incoming: &Question) -> bool {
    if stored.options == incoming.options && stored.answer == incoming.answer {
        return true;
    }
    if stored.qtype == "unknown" {
        return stored.answer.trim() == incoming.answer.trim();
    }
    let canonical = |question: &Question| -> Option<Vec<String>> {
        let answer = db::answer_for_options(question, &incoming.options, &incoming.qtype)?;
        if !matches!(incoming.qtype.as_str(), "single" | "multiple") {
            return Some(vec![answer]);
        }
        let options = db::option_texts(&incoming.options);
        let mut texts = if let Some(letters) = crate::answers::validate(
            &serde_json::Value::String(answer.clone()),
            &incoming.qtype,
            &incoming.options,
        ) {
            letters
                .split('#')
                .map(|letter| options.get((letter.as_bytes()[0] - b'A') as usize).cloned())
                .collect::<Option<Vec<_>>>()?
        } else {
            vec![db::normalize(&answer)]
        };
        texts.sort();
        Some(texts)
    };
    canonical(stored)
        .zip(canonical(incoming))
        .is_some_and(|(left, right)| left == right)
}

fn snapshot(questions: &[Question]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for q in questions {
        (
            q.id,
            &q.title,
            &q.options,
            &q.qtype,
            &q.answer,
            &q.updated_at,
        )
            .hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

fn read_questions(conn: &rusqlite::Connection) -> R<Vec<Question>> {
    let mut stmt = conn.prepare("SELECT id,title,options,qtype,answer,source,provider,model,hits,created_at,updated_at FROM questions ORDER BY id").map_err(|e|e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
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
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn serialize(questions: Vec<Question>) -> R<String> {
    if questions.len() > MAX_QUESTIONS {
        return Err("题库超过单个备份支持的 100000 道题上限".into());
    }
    let content = serde_json::to_string_pretty(&QuestionBackup {
        format: "ai2tiku.questions".into(),
        version: 1,
        exported_at: db::now(),
        questions,
    })
    .map_err(|e| e.to_string())?;
    if content.len() as u64 > MAX_FILE_BYTES {
        return Err("题库超过单个备份支持的 50MB 上限".into());
    }
    Ok(content)
}

struct Parsed {
    total: usize,
    questions: Vec<Question>,
    invalid: usize,
    file_duplicates: usize,
    file_conflicts: usize,
    errors: Vec<String>,
}

fn parse(content: &str) -> R<Parsed> {
    if content.len() as u64 > MAX_FILE_BYTES {
        return Err("文件超过 50MB 上限".into());
    }
    let value: serde_json::Value = serde_json::from_str(content.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("JSON 格式错误（第 {} 行，第 {} 列）", e.line(), e.column()))?;
    let own_backup = !value.is_array();
    let rows = if let Some(rows) = value.as_array() {
        rows
    } else {
        if value.get("format").and_then(|v| v.as_str()) != Some("ai2tiku.questions")
            || value.get("version").and_then(|v| v.as_u64()) != Some(1)
        {
            return Err("不支持的题库格式或版本，请使用本软件导出的 JSON 或题目数组".into());
        }
        value
            .get("questions")
            .and_then(|v| v.as_array())
            .ok_or("缺少 questions 数组")?
    };
    if rows.len() > MAX_QUESTIONS {
        return Err("单次最多导入 100000 道题".into());
    }
    let mut parsed = Parsed {
        total: rows.len(),
        questions: Vec::new(),
        invalid: 0,
        file_duplicates: 0,
        file_conflicts: 0,
        errors: Vec::new(),
    };
    let mut seen = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        let result: R<Question> = (|| {
            let text = |key| {
                row.get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let title = text("title");
            let qtype = row
                .get("qtype")
                .or_else(|| row.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            if !matches!(
                qtype.as_str(),
                "single" | "multiple" | "judgement" | "completion" | "unknown"
            ) {
                return Err("不支持的题型".into());
            }
            let options = match row.get("options") {
                None | Some(serde_json::Value::Null) => String::new(),
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(serde_json::Value::Array(values)) if values.iter().all(|v| v.is_string()) => {
                    values
                        .iter()
                        .map(|v| v.as_str().unwrap())
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                _ => return Err("选项须为字符串或字符串数组".into()),
            };
            let value = row.get("answer").ok_or("缺少答案")?;
            let answer = if own_backup || qtype == "unknown" {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or("备份答案须为字符串")?
            } else {
                let decoded = value
                    .as_str()
                    .and_then(crate::answers::extract_json)
                    .filter(|v| v.is_array() || v.is_string());
                crate::answers::validate(decoded.as_ref().unwrap_or(value), &qtype, &options)
                    .or_else(|| {
                        // Text answers to choice questions are accepted only when uniquely present.
                        value
                            .as_str()
                            .filter(|answer| {
                                matches!(qtype.as_str(), "single" | "multiple")
                                    && db::option_texts(&options)
                                        .iter()
                                        .filter(|option| **option == db::normalize(answer))
                                        .count()
                                        == 1
                            })
                            .map(str::to_string)
                    })
                    .ok_or("答案格式或选项范围无效")?
            };
            if title.trim().is_empty() || answer.trim().is_empty() {
                return Err("题干与答案不能为空".into());
            }
            if title.len() > 200_000 || options.len() > 500_000 || answer.len() > 200_000 {
                return Err("题目内容过长".into());
            }
            Ok(Question {
                id: 0,
                title,
                options,
                qtype,
                answer,
                source: if text("source") == "ai" {
                    "ai"
                } else {
                    "manual"
                }
                .into(),
                provider: text("provider"),
                model: text("model"),
                hits: row.get("hits").and_then(|v| v.as_i64()).unwrap_or(0).max(0),
                created_at: if text("createdAt").is_empty() {
                    db::now()
                } else {
                    text("createdAt")
                },
                updated_at: if text("updatedAt").is_empty() {
                    db::now()
                } else {
                    text("updatedAt")
                },
            })
        })();
        match result {
            Ok(question) => {
                let key = question_key(&question);
                if let Some(&previous) = seen.get(&key) {
                    if same_answer(&parsed.questions[previous], &question) {
                        parsed.file_duplicates += 1;
                    } else {
                        parsed.file_conflicts += 1;
                        if !own_backup {
                            parsed.invalid += 1;
                            if parsed.errors.len() < 20 {
                                parsed
                                    .errors
                                    .push(format!("第 {} 条：同文件内同题存在冲突答案", index + 1));
                            }
                        }
                    }
                    // A versioned backup must be able to restore even legacy duplicate rows.
                    if own_backup {
                        parsed.questions.push(question);
                    }
                } else {
                    seen.insert(key, parsed.questions.len());
                    parsed.questions.push(question);
                }
            }
            Err(error) => {
                parsed.invalid += 1;
                if parsed.errors.len() < 20 {
                    parsed.errors.push(format!("第 {} 条：{error}", index + 1));
                }
            }
        }
    }
    Ok(parsed)
}

impl Db {
    pub fn export_questions(&self) -> R<String> {
        serialize(read_questions(&*self.lock()?)?)
    }

    pub fn preview_questions(&self, content: &str) -> R<ImportPreview> {
        let parsed = parse(content)?;
        let existing = read_questions(&*self.lock()?)?;
        let mut by_key = HashMap::<_, Vec<_>>::new();
        for question in &existing {
            by_key
                .entry(question_key(question))
                .or_default()
                .push(question);
        }
        let (mut new_count, mut duplicates, mut conflicts) = (0, 0, 0);
        let mut visited = std::collections::HashSet::new();
        for question in &parsed.questions {
            if !visited.insert(question_key(question)) {
                continue;
            }
            match by_key.get(&question_key(question)) {
                None => new_count += 1,
                Some(stored) if stored.iter().all(|stored| same_answer(stored, question)) => {
                    duplicates += 1
                }
                Some(_) => conflicts += 1,
            }
        }
        Ok(ImportPreview {
            total: parsed.total,
            valid: parsed.questions.len(),
            invalid: parsed.invalid,
            new_count,
            duplicates,
            conflicts,
            file_duplicates: parsed.file_duplicates,
            file_conflicts: parsed.file_conflicts,
            existing: existing.len(),
            errors: parsed.errors,
            snapshot: snapshot(&existing),
        })
    }

    pub fn import_questions(
        &self,
        content: &str,
        mode: &str,
        expected_snapshot: &str,
    ) -> R<ImportResult> {
        if !matches!(mode, "merge" | "replace") {
            return Err("无效的导入方式".into());
        }
        let parsed = parse(content)?;
        if parsed.invalid > 0 {
            return Err("文件含无效或冲突题目，请修正后重新预览；当前题库未改变".into());
        }
        if mode == "replace" && parsed.questions.is_empty() {
            return Err("不能使用空文件恢复题库；当前题库未改变".into());
        }
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let existing = read_questions(&tx)?;
        if snapshot(&existing) != expected_snapshot {
            return Err("题库在预览后发生变化，请重新预览再操作".into());
        }
        // Back up under the same DB lock before any mutation. A failed backup aborts restoration.
        let backup_path = if mode == "replace" {
            let dir = self
                .path
                .parent()
                .ok_or("无法定位备份目录")?
                .join("backups");
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let path = dir.join(format!(
                "before-restore-{}.json",
                chrono::Local::now().format("%Y%m%d-%H%M%S-%f")
            ));
            atomic_write(&path, &serialize(existing.clone())?)?;
            tx.execute("DELETE FROM questions", [])
                .map_err(|e| e.to_string())?;
            path.display().to_string()
        } else {
            String::new()
        };
        let mut keys: std::collections::HashSet<_> = existing.iter().map(question_key).collect();
        let mut imported = 0;
        for question in &parsed.questions {
            if mode == "merge" && keys.contains(&question_key(question)) {
                continue;
            }
            tx.execute("INSERT INTO questions(title,normalized_title,options,qtype,answer,source,provider,model,hits,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![question.title,db::normalize(&question.title),question.options,question.qtype,question.answer,question.source,question.provider,question.model,question.hits,question.created_at,question.updated_at]).map_err(|e|e.to_string())?;
            imported += 1;
            keys.insert(question_key(question));
        }
        let total: usize = tx
            .query_row("SELECT COUNT(*) FROM questions", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        self.changed();
        Ok(ImportResult {
            imported,
            skipped: parsed.total - imported,
            total,
            backup_path,
        })
    }
}

pub fn atomic_write(path: &Path, content: &str) -> R<()> {
    let temporary = path.with_file_name(format!(
        ".ai2tiku-{}.tmp",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

pub fn read_file(path: &Path) -> R<String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut content = String::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_string(&mut content)
        .map_err(|_| "文件须为 UTF-8 JSON".to_string())?;
    if content.len() as u64 > MAX_FILE_BYTES {
        return Err("文件超过 50MB 上限".into());
    }
    Ok(content)
}

#[cfg(feature = "webview")]
pub async fn save_dialog(app: tauri::AppHandle, content: String) -> R<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        let file = app
            .dialog()
            .file()
            .add_filter("题库 JSON", &["json"])
            .set_file_name(format!(
                "ai2tiku-questions-{}.json",
                chrono::Local::now().format("%Y%m%d-%H%M%S")
            ))
            .blocking_save_file();
        let Some(file) = file else { return Ok(None) };
        let path = file.into_path().map_err(|e| e.to_string())?;
        atomic_write(&path, &content)?;
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(feature = "webview")]
pub async fn open_dialog(app: tauri::AppHandle) -> R<Option<(String, String)>> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("题库 JSON", &["json"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path: PathBuf = file.into_path().map_err(|e| e.to_string())?;
        Ok(Some((
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            read_file(&path)?,
        )))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{configure, question, state};
    fn data(title: &str, answer: &str) -> String {
        serde_json::json!([{ "title":title,"options":"A. 甲\nB. 乙","qtype":"single","answer":answer }]).to_string()
    }

    #[test]
    fn round_trip_restores_metadata_and_never_exports_credentials_or_records() {
        let state = state();
        configure(&state, "http://127.0.0.1:1");
        state.db.save_question(&question("原题", "A")).unwrap();
        state.db.bump_hits(1).unwrap();
        state
            .db
            .record(&SearchRecord {
                title: "不应导出的请求".into(),
                status: "failure".into(),
                ..Default::default()
            })
            .unwrap();
        let backup = state.db.export_questions().unwrap();
        assert!(!backup.contains("secret-test-key") && !backup.contains("不应导出的请求"));
        state.db.clear_questions().unwrap();
        let preview = state.db.preview_questions(&backup).unwrap();
        let result = state
            .db
            .import_questions(&backup, "replace", &preview.snapshot)
            .unwrap();
        assert_eq!(result.imported, 1);
        assert_eq!(
            state.db.list_questions(None, None, 1, 10).unwrap().items[0].hits,
            1
        );
        assert!(Path::new(&result.backup_path).is_file());
        assert_eq!(state.db.list_records(1, 10).unwrap().total, 1);
        assert_eq!(
            state.db.list_providers().unwrap()[0].api_key,
            "secret-test-key"
        );
    }
    #[test]
    fn merge_previews_shuffled_duplicates_conflicts_and_keeps_existing_answers() {
        let state = state();
        state.db.save_question(&question("重复题", "A")).unwrap();
        let content = serde_json::json!([
            {"title":"重复题","options":"A. 乙\nB. 甲","qtype":"single","answer":"B"},
            {"title":"新题","options":"A. 甲\nB. 乙","qtype":"single","answer":"B"}
        ])
        .to_string();
        let preview = state.db.preview_questions(&content).unwrap();
        assert_eq!(
            (preview.new_count, preview.duplicates, preview.conflicts),
            (1, 1, 0)
        );
        let result = state
            .db
            .import_questions(&content, "merge", &preview.snapshot)
            .unwrap();
        assert_eq!((result.imported, result.skipped, result.total), (1, 1, 2));
        let conflict = data("重复题", "B");
        let preview = state.db.preview_questions(&conflict).unwrap();
        assert_eq!(preview.conflicts, 1);
        assert_eq!(
            state
                .db
                .import_questions(&conflict, "merge", &preview.snapshot)
                .unwrap()
                .imported,
            0
        );
        assert_eq!(
            state
                .db
                .find_local("重复题", "A. 甲\nB. 乙", "single", 1.0)
                .unwrap()
                .unwrap()
                .0
                .answer,
            "A"
        );
    }

    #[test]
    fn shuffled_multiple_answers_and_text_choices_compare_by_selected_content() {
        let state = state();
        state
            .db
            .save_question(&QuestionInput {
                id: None,
                title: "多选".into(),
                options: Some("A. 甲\nB. 乙\nC. 丙".into()),
                qtype: Some("multiple".into()),
                answer: "A#B".into(),
            })
            .unwrap();
        state.db.save_question(&question("文字答案", "甲")).unwrap();
        let content = serde_json::json!([
            {"title":"多选","options":"A. 乙\nB. 甲\nC. 丙","qtype":"multiple","answer":["A","B"]},
            {"title":"文字答案","options":"A. 乙\nB. 甲","qtype":"single","answer":"B"}
        ])
        .to_string();
        let preview = state.db.preview_questions(&content).unwrap();
        assert_eq!((preview.duplicates, preview.conflicts), (2, 0));
    }
    #[test]
    fn invalid_rows_and_unsupported_versions_never_partially_import() {
        let state = state();
        let content = serde_json::json!([
            {"title":"正常","options":"A. 甲\nB. 乙","qtype":"single","answer":"A"},
            {"title":"越界","options":"A. 甲\nB. 乙","qtype":"single","answer":"Z"}
        ])
        .to_string();
        let preview = state.db.preview_questions(&content).unwrap();
        assert_eq!(preview.invalid, 1);
        assert!(state
            .db
            .import_questions(&content, "merge", &preview.snapshot)
            .is_err());
        assert_eq!(state.db.stats().unwrap().question_total, 0);
        assert!(state
            .db
            .preview_questions(r#"{"format":"ai2tiku.questions","version":99,"questions":[]}"#)
            .is_err());
        assert!(state.db.preview_questions("not json").is_err());
    }
    #[test]
    fn stale_previews_are_rejected_and_restore_creates_recoverable_preimage() {
        let state = state();
        let incoming = data("新题", "B");
        let preview = state.db.preview_questions(&incoming).unwrap();
        state.db.save_question(&question("原题", "A")).unwrap();
        assert!(state
            .db
            .import_questions(&incoming, "replace", &preview.snapshot)
            .unwrap_err()
            .contains("重新预览"));
        let preview = state.db.preview_questions(&incoming).unwrap();
        let result = state
            .db
            .import_questions(&incoming, "replace", &preview.snapshot)
            .unwrap();
        let previous = read_file(Path::new(&result.backup_path)).unwrap();
        assert!(previous.contains("原题"));
        assert_eq!(state.db.stats().unwrap().question_total, 1);
        let preview = state.db.preview_questions(&previous).unwrap();
        state
            .db
            .import_questions(&previous, "replace", &preview.snapshot)
            .unwrap();
        assert_eq!(
            state.db.list_questions(None, None, 1, 10).unwrap().items[0].title,
            "原题"
        );
    }
    #[test]
    fn failed_restore_rolls_back_delete_and_inserts() {
        let state = state();
        state.db.save_question(&question("保留", "A")).unwrap();
        state.db.lock().unwrap().execute_batch("CREATE TRIGGER reject_import BEFORE INSERT ON questions BEGIN SELECT RAISE(ABORT,'simulated write failure'); END;").unwrap();
        let content = data("新题", "B");
        let preview = state.db.preview_questions(&content).unwrap();
        assert!(state
            .db
            .import_questions(&content, "replace", &preview.snapshot)
            .is_err());
        assert_eq!(
            state.db.list_questions(None, None, 1, 10).unwrap().items[0].title,
            "保留"
        );
    }
    #[test]
    fn failed_backup_blocks_restore_and_empty_files_cannot_replace_bank() {
        let state = state();
        state.db.save_question(&question("保留", "A")).unwrap();
        std::fs::write(
            state.db.path.parent().unwrap().join("backups"),
            "阻止创建备份目录",
        )
        .unwrap();
        let content = data("新题", "B");
        let preview = state.db.preview_questions(&content).unwrap();
        assert!(state
            .db
            .import_questions(&content, "replace", &preview.snapshot)
            .is_err());
        assert_eq!(state.db.stats().unwrap().question_total, 1);
        let preview = state.db.preview_questions("[]").unwrap();
        assert!(state
            .db
            .import_questions("[]", "replace", &preview.snapshot)
            .is_err());
    }
    #[test]
    fn versioned_backups_preserve_legacy_duplicates_and_noncanonical_answers() {
        let state = state();
        state.db.save_question(&question("旧题", "A")).unwrap();
        state.db.save_question(&question("旧题", "B")).unwrap();
        let backup = state.db.export_questions().unwrap();
        let preview = state.db.preview_questions(&backup).unwrap();
        assert_eq!(
            (preview.valid, preview.invalid, preview.file_conflicts),
            (2, 0, 1)
        );
        state
            .db
            .import_questions(&backup, "replace", &preview.snapshot)
            .unwrap();
        assert_eq!(state.db.stats().unwrap().question_total, 2);
    }
}
