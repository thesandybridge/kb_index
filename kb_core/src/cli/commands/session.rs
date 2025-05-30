use crate::{config, state::SessionManager};


pub fn handle_sessions(list: bool, clear: bool, switch: Option<String>) -> anyhow::Result<()> {
    let config_dir = config::get_config_dir()?;
    let mut session_manager = SessionManager::load(&config_dir)?;

    if clear {
        if let Some(active_id) = session_manager.active_session.clone() {
            session_manager.sessions.remove(&active_id);
            session_manager.active_session = None;
            session_manager.save(&config_dir)?;
            println!("🧹 Cleared session: {}", active_id);
        } else {
            println!("⚠️ No active session to clear");
        }
        return Ok(());
    }

    if let Some(id) = switch {
        session_manager.set_active_session(&id)?;
        println!("🔄 Switched to session: {}", id);
        session_manager.save(&config_dir)?;
        return Ok(());
    }

    if list || session_manager.sessions.is_empty() {
        println!("📋 Available Sessions:");
        for (id, session) in session_manager.list_sessions() {
            let active = if Some(id) == session_manager.active_session.as_ref() {
                "* "
            } else {
                "  "
            };

            let time = chrono::DateTime::<chrono::Utc>::from_timestamp(
                session.last_updated as i64, 0
            ).unwrap_or_default().format("%Y-%m-%d %H:%M");

            println!("{}{} - {} Q&A pairs, last updated: {}",
                active,
                &id[..8],
                session.queries.len(),
                time
            );
        }

        if session_manager.sessions.is_empty() {
            println!("  No sessions found. Create one with 'kb query --session new'");
        }
    }

    Ok(())
}
