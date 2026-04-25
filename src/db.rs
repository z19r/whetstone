use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::cli::DbCommand;

const SCHEMA: &str = include_str!("../assets/db/schema.sql");

fn db_path() -> PathBuf {
    let claude_dir = std::env::current_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("db");
    claude_dir.join("memstack.db")
}

fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
    Ok(conn)
}

fn parse_json(raw: &str) -> Result<Value> {
    serde_json::from_str(raw).context("invalid JSON input")
}

fn require_fields(data: &Value, fields: &[&str]) -> Result<()> {
    for f in fields {
        if data.get(f).is_none() || data[f].is_null() {
            bail!("missing required field: {f}");
        }
    }
    Ok(())
}

fn str_field(data: &Value, key: &str) -> Option<String> {
    data.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn str_or_empty(data: &Value, key: &str) -> String {
    str_field(data, key).unwrap_or_default()
}

fn i64_field(data: &Value, key: &str) -> Option<i64> {
    data.get(key).and_then(|v| v.as_i64())
}

pub fn dispatch(cmd: DbCommand) -> Result<()> {
    let result = match cmd {
        DbCommand::Init => cmd_init(),
        DbCommand::AddSession { json } => cmd_add_session(&json),
        DbCommand::AddInsight { json } => cmd_add_insight(&json),
        DbCommand::Search { query, project, limit } => cmd_search(&query, project.as_deref(), limit),
        DbCommand::GetSessions { project, limit } => cmd_get_sessions(&project, limit),
        DbCommand::GetInsights { project } => cmd_get_insights(&project),
        DbCommand::GetContext { project } => cmd_get_context(&project),
        DbCommand::SetContext { json } => cmd_set_context(&json),
        DbCommand::AddPlanTask { json } => cmd_add_plan_task(&json),
        DbCommand::GetPlan { project } => cmd_get_plan(&project),
        DbCommand::UpdateTask { json } => cmd_update_task(&json),
        DbCommand::ExportMd { project } => cmd_export_md(&project),
        DbCommand::Stats => cmd_stats(),
    };

    if let Err(e) = &result {
        println!("{}", json!({"ok": false, "error": e.to_string()}));
    }
    result
}

fn cmd_init() -> Result<()> {
    let path = db_path();
    let conn = open(&path)?;
    conn.execute_batch(SCHEMA)?;
    conn.close().map_err(|(_, e)| e)?;
    println!("{}", json!({"ok": true, "db": path.display().to_string()}));
    Ok(())
}

fn cmd_add_session(raw: &str) -> Result<()> {
    let data = parse_json(raw)?;
    require_fields(&data, &["project"])?;

    let conn = open(&db_path())?;
    conn.execute(
        "INSERT INTO sessions (project, date, accomplished, files_changed, commits,
         decisions, problems, next_steps, duration, raw_markdown)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            str_or_empty(&data, "project"),
            str_or_empty(&data, "date"),
            str_or_empty(&data, "accomplished"),
            str_or_empty(&data, "files_changed"),
            str_or_empty(&data, "commits"),
            str_or_empty(&data, "decisions"),
            str_or_empty(&data, "problems"),
            str_or_empty(&data, "next_steps"),
            str_or_empty(&data, "duration"),
            str_or_empty(&data, "raw_markdown"),
        ],
    )?;
    let row_id: i64 = conn.query_row("SELECT last_insert_rowid()", [], |r| r.get(0))?;
    println!("{}", json!({"ok": true, "id": row_id}));
    Ok(())
}

fn cmd_add_insight(raw: &str) -> Result<()> {
    let data = parse_json(raw)?;
    require_fields(&data, &["content"])?;

    let conn = open(&db_path())?;
    conn.execute(
        "INSERT INTO insights (project, type, content, context, tags)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            str_field(&data, "project"),
            str_field(&data, "type").unwrap_or_else(|| "decision".into()),
            str_or_empty(&data, "content"),
            str_or_empty(&data, "context"),
            str_or_empty(&data, "tags"),
        ],
    )?;
    let row_id: i64 = conn.query_row("SELECT last_insert_rowid()", [], |r| r.get(0))?;
    println!("{}", json!({"ok": true, "id": row_id}));
    Ok(())
}

fn cmd_search(query: &str, project: Option<&str>, limit: usize) -> Result<()> {
    let conn = open(&db_path())?;
    let pattern = format!("%{query}%");
    let mut results: Vec<Value> = Vec::new();

    // Search sessions
    let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(proj) = project {
        (
            "SELECT id, project, date, accomplished, decisions FROM sessions
             WHERE (accomplished LIKE ?1 OR decisions LIKE ?2 OR commits LIKE ?3
                    OR next_steps LIKE ?4 OR problems LIKE ?5)
             AND project = ?6 ORDER BY date DESC LIMIT ?7".into(),
            vec![
                Box::new(pattern.clone()), Box::new(pattern.clone()),
                Box::new(pattern.clone()), Box::new(pattern.clone()),
                Box::new(pattern.clone()), Box::new(proj.to_string()),
                Box::new(limit as i64),
            ],
        )
    } else {
        (
            "SELECT id, project, date, accomplished, decisions FROM sessions
             WHERE (accomplished LIKE ?1 OR decisions LIKE ?2 OR commits LIKE ?3
                    OR next_steps LIKE ?4 OR problems LIKE ?5)
             ORDER BY date DESC LIMIT ?6".into(),
            vec![
                Box::new(pattern.clone()), Box::new(pattern.clone()),
                Box::new(pattern.clone()), Box::new(pattern.clone()),
                Box::new(pattern.clone()), Box::new(limit as i64),
            ],
        )
    };

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        let accomplished: Option<String> = row.get(3)?;
        let decisions: Option<String> = row.get(4)?;
        Ok(json!({
            "type": "session",
            "id": row.get::<_, i64>(0)?,
            "project": row.get::<_, String>(1)?,
            "date": row.get::<_, String>(2)?,
            "accomplished": accomplished.unwrap_or_default().chars().take(200).collect::<String>(),
            "decisions": decisions.unwrap_or_default().chars().take(200).collect::<String>(),
        }))
    })?;
    for row in rows {
        results.push(row?);
    }

    // Search insights
    let (sql2, params_vec2): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(proj) = project {
        (
            "SELECT id, project, type, content, tags FROM insights
             WHERE content LIKE ?1 AND project = ?2
             ORDER BY created_at DESC LIMIT ?3".into(),
            vec![Box::new(pattern.clone()), Box::new(proj.to_string()), Box::new(limit as i64)],
        )
    } else {
        (
            "SELECT id, project, type, content, tags FROM insights
             WHERE content LIKE ?1 ORDER BY created_at DESC LIMIT ?2".into(),
            vec![Box::new(pattern), Box::new(limit as i64)],
        )
    };

    let param_refs2: Vec<&dyn rusqlite::types::ToSql> = params_vec2.iter().map(|p| p.as_ref()).collect();
    let mut stmt2 = conn.prepare(&sql2)?;
    let rows2 = stmt2.query_map(param_refs2.as_slice(), |row| {
        let content: String = row.get(3)?;
        Ok(json!({
            "type": "insight",
            "id": row.get::<_, i64>(0)?,
            "project": row.get::<_, Option<String>>(1)?,
            "insight_type": row.get::<_, String>(2)?,
            "content": content.chars().take(200).collect::<String>(),
            "tags": row.get::<_, Option<String>>(4)?,
        }))
    })?;
    for row in rows2 {
        results.push(row?);
    }

    let count = results.len();
    println!("{}", json!({"results": results, "count": count}));
    Ok(())
}

fn cmd_get_sessions(project: &str, limit: usize) -> Result<()> {
    let conn = open(&db_path())?;
    let mut stmt = conn.prepare(
        "SELECT id, project, date, accomplished, files_changed, commits,
         decisions, problems, next_steps, duration
         FROM sessions WHERE project = ?1 ORDER BY date DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![project, limit as i64], |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "project": row.get::<_, String>(1)?,
            "date": row.get::<_, String>(2)?,
            "accomplished": row.get::<_, Option<String>>(3)?,
            "files_changed": row.get::<_, Option<String>>(4)?,
            "commits": row.get::<_, Option<String>>(5)?,
            "decisions": row.get::<_, Option<String>>(6)?,
            "problems": row.get::<_, Option<String>>(7)?,
            "next_steps": row.get::<_, Option<String>>(8)?,
            "duration": row.get::<_, Option<String>>(9)?,
        }))
    })?;
    let sessions: Vec<Value> = rows.filter_map(|r| r.ok()).collect();
    let count = sessions.len();
    println!("{}", json!({"sessions": sessions, "count": count}));
    Ok(())
}

fn cmd_get_insights(project: &str) -> Result<()> {
    let conn = open(&db_path())?;
    let mut stmt = conn.prepare(
        "SELECT id, project, type, content, context, tags, created_at
         FROM insights WHERE project = ?1 OR project IS NULL
         ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map(params![project], |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "project": row.get::<_, Option<String>>(1)?,
            "type": row.get::<_, String>(2)?,
            "content": row.get::<_, String>(3)?,
            "context": row.get::<_, Option<String>>(4)?,
            "tags": row.get::<_, Option<String>>(5)?,
            "created_at": row.get::<_, Option<String>>(6)?,
        }))
    })?;
    let insights: Vec<Value> = rows.filter_map(|r| r.ok()).collect();
    let count = insights.len();
    println!("{}", json!({"insights": insights, "count": count}));
    Ok(())
}

fn cmd_get_context(project: &str) -> Result<()> {
    let conn = open(&db_path())?;
    let mut stmt = conn.prepare("SELECT * FROM project_context WHERE project = ?1")?;
    let row = stmt.query_row(params![project], |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "project": row.get::<_, String>(1)?,
            "status": row.get::<_, String>(2)?,
            "current_branch": row.get::<_, Option<String>>(3)?,
            "last_session_date": row.get::<_, Option<String>>(4)?,
            "architecture_decisions": row.get::<_, Option<String>>(5)?,
            "known_issues": row.get::<_, Option<String>>(6)?,
            "backlog": row.get::<_, Option<String>>(7)?,
            "updated_at": row.get::<_, Option<String>>(8)?,
        }))
    });
    match row {
        Ok(v) => println!("{v}"),
        Err(_) => println!("{}", json!({"project": project, "status": "no context saved"})),
    }
    Ok(())
}

fn cmd_set_context(raw: &str) -> Result<()> {
    let data = parse_json(raw)?;
    require_fields(&data, &["project"])?;

    let conn = open(&db_path())?;
    conn.execute(
        "INSERT INTO project_context (project, status, current_branch, last_session_date,
         architecture_decisions, known_issues, backlog, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
         ON CONFLICT(project) DO UPDATE SET
         status = COALESCE(?2, status),
         current_branch = COALESCE(?3, current_branch),
         last_session_date = COALESCE(?4, last_session_date),
         architecture_decisions = COALESCE(?5, architecture_decisions),
         known_issues = COALESCE(?6, known_issues),
         backlog = COALESCE(?7, backlog),
         updated_at = datetime('now')",
        params![
            str_or_empty(&data, "project"),
            str_field(&data, "status"),
            str_field(&data, "current_branch"),
            str_field(&data, "last_session_date"),
            str_field(&data, "architecture_decisions"),
            str_field(&data, "known_issues"),
            str_field(&data, "backlog"),
        ],
    )?;
    let project = str_or_empty(&data, "project");
    println!("{}", json!({"ok": true, "project": project}));
    Ok(())
}

fn cmd_add_plan_task(raw: &str) -> Result<()> {
    let data = parse_json(raw)?;
    require_fields(&data, &["project", "task_number", "description"])?;

    let conn = open(&db_path())?;
    conn.execute(
        "INSERT INTO plans (project, task_number, description, status, blocked_reason)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(project, task_number) DO UPDATE SET
         description = ?3, status = ?4,
         blocked_reason = ?5, updated_at = datetime('now')",
        params![
            str_or_empty(&data, "project"),
            i64_field(&data, "task_number"),
            str_or_empty(&data, "description"),
            str_field(&data, "status").unwrap_or_else(|| "pending".into()),
            str_field(&data, "blocked_reason"),
        ],
    )?;
    println!("{}", json!({"ok": true}));
    Ok(())
}

fn cmd_get_plan(project: &str) -> Result<()> {
    let conn = open(&db_path())?;
    let mut stmt = conn.prepare(
        "SELECT id, project, task_number, description, status, blocked_reason, created_at, updated_at
         FROM plans WHERE project = ?1 ORDER BY task_number",
    )?;
    let rows = stmt.query_map(params![project], |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "project": row.get::<_, String>(1)?,
            "task_number": row.get::<_, i64>(2)?,
            "description": row.get::<_, String>(3)?,
            "status": row.get::<_, String>(4)?,
            "blocked_reason": row.get::<_, Option<String>>(5)?,
            "created_at": row.get::<_, Option<String>>(6)?,
            "updated_at": row.get::<_, Option<String>>(7)?,
        }))
    })?;
    let tasks: Vec<Value> = rows.filter_map(|r| r.ok()).collect();
    let done = tasks.iter().filter(|t| t["status"] == "completed").count();
    let total = tasks.len();
    println!("{}", json!({"project": project, "tasks": tasks, "done": done, "total": total}));
    Ok(())
}

fn cmd_update_task(raw: &str) -> Result<()> {
    let data = parse_json(raw)?;
    require_fields(&data, &["project", "task_number", "status"])?;

    let conn = open(&db_path())?;
    let changed = conn.execute(
        "UPDATE plans SET status = ?1, blocked_reason = ?2, updated_at = datetime('now')
         WHERE project = ?3 AND task_number = ?4",
        params![
            str_or_empty(&data, "status"),
            str_field(&data, "blocked_reason"),
            str_or_empty(&data, "project"),
            i64_field(&data, "task_number"),
        ],
    )?;
    if changed == 0 {
        let proj = str_or_empty(&data, "project");
        let num = i64_field(&data, "task_number").unwrap_or(0);
        println!("{}", json!({"ok": false, "error": format!("Task not found: {proj} #{num}")}));
    } else {
        println!("{}", json!({"ok": true}));
    }
    Ok(())
}

fn cmd_export_md(project: &str) -> Result<()> {
    let conn = open(&db_path())?;
    let mut lines = vec![format!("# Memory Export — {project}\n")];

    // Sessions
    let mut stmt = conn.prepare(
        "SELECT date, accomplished, commits, decisions, next_steps
         FROM sessions WHERE project = ?1 ORDER BY date DESC",
    )?;
    type SessionRow = (String, Option<String>, Option<String>, Option<String>, Option<String>);
    let sessions: Vec<SessionRow> = stmt
        .query_map(params![project], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    if !sessions.is_empty() {
        lines.push("## Sessions\n".into());
        for (date, accomplished, commits, decisions, next_steps) in &sessions {
            lines.push(format!("### {date}"));
            if let Some(a) = accomplished { if !a.is_empty() { lines.push(format!("**Accomplished:**\n{a}")); } }
            if let Some(c) = commits { if !c.is_empty() { lines.push(format!("**Commits:**\n{c}")); } }
            if let Some(d) = decisions { if !d.is_empty() { lines.push(format!("**Decisions:**\n{d}")); } }
            if let Some(n) = next_steps { if !n.is_empty() { lines.push(format!("**Next Steps:**\n{n}")); } }
            lines.push(String::new());
        }
    }

    // Insights
    let mut stmt = conn.prepare(
        "SELECT type, content FROM insights WHERE project = ?1 ORDER BY created_at DESC",
    )?;
    let insights: Vec<(String, String)> = stmt.query_map(params![project], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?.filter_map(|r| r.ok()).collect();

    if !insights.is_empty() {
        lines.push("## Insights\n".into());
        for (typ, content) in &insights {
            lines.push(format!("- **[{typ}]** {content}"));
        }
        lines.push(String::new());
    }

    // Context
    let ctx = conn.query_row(
        "SELECT status, current_branch, architecture_decisions, known_issues, backlog
         FROM project_context WHERE project = ?1",
        params![project],
        |row| Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        )),
    );
    if let Ok((status, branch, arch, issues, backlog)) = ctx {
        lines.push("## Project Context\n".into());
        lines.push(format!("- **Status:** {status}"));
        if let Some(b) = branch { if !b.is_empty() { lines.push(format!("- **Current Branch:** {b}")); } }
        if let Some(a) = arch { if !a.is_empty() { lines.push(format!("- **Architecture Decisions:**\n{a}")); } }
        if let Some(i) = issues { if !i.is_empty() { lines.push(format!("- **Known Issues:**\n{i}")); } }
        if let Some(bl) = backlog { if !bl.is_empty() { lines.push(format!("- **Backlog:**\n{bl}")); } }
    }

    println!("{}", lines.join("\n"));
    Ok(())
}

fn cmd_stats() -> Result<()> {
    let path = db_path();
    let conn = open(&path)?;

    let sessions: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;
    let insights: i64 = conn.query_row("SELECT COUNT(*) FROM insights", [], |r| r.get(0))?;
    let projects: i64 = conn.query_row("SELECT COUNT(*) FROM project_context", [], |r| r.get(0))?;
    let plan_tasks: i64 = conn.query_row("SELECT COUNT(*) FROM plans", [], |r| r.get(0))?;

    let mut stmt = conn.prepare(
        "SELECT project, COUNT(*) as cnt FROM sessions GROUP BY project ORDER BY cnt DESC",
    )?;
    let by_project: Value = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
        .filter_map(|r| r.ok())
        .fold(json!({}), |mut acc, (proj, cnt)| {
            acc[proj] = json!(cnt);
            acc
        });

    let size_kb = path
        .metadata()
        .map(|m| (m.len() as f64 / 1024.0 * 10.0).round() / 10.0)
        .unwrap_or(0.0);

    println!("{}", json!({
        "sessions": sessions,
        "insights": insights,
        "projects": projects,
        "plan_tasks": plan_tasks,
        "sessions_by_project": by_project,
        "db_path": path.display().to_string(),
        "db_size_kb": size_kb,
    }));
    Ok(())
}
