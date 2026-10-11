//! Conversation persistence

use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

use super::{get_db_connection, Conversation, ConversationMessage, ConversationWithMessages};

/// Create a new conversation
#[tauri::command(async)]
#[specta::specta]
pub fn create_conversation(title: Option<String>) -> Result<Conversation, String> {
    let conn = get_db_connection()?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let conversation_title = title.unwrap_or_else(|| "New Chat".to_string());

    conn.execute(
        "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, conversation_title, now, now],
    )
    .map_err(|e| format!("Failed to create conversation: {}", e))?;

    Ok(Conversation {
        id,
        title: conversation_title,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// List all conversations
#[tauri::command(async)]
#[specta::specta]
pub fn list_conversations(limit: Option<usize>) -> Result<Vec<Conversation>, String> {
    let conn = get_db_connection()?;
    let limit_val = limit.unwrap_or(50) as i64;

    // Only show conversations that have at least one message (excludes empty "New Chat" stubs)
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.title, c.created_at, c.updated_at
             FROM conversations c
             WHERE EXISTS (SELECT 1 FROM conversation_messages m WHERE m.conversation_id = c.id)
             ORDER BY c.updated_at DESC
             LIMIT ?1",
        )
        .map_err(|e| format!("Failed to prepare query: {}", e))?;

    let rows = stmt
        .query_map(params![limit_val], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })
        .map_err(|e| format!("Failed to execute query: {}", e))?;

    let mut conversations = Vec::new();
    for row in rows {
        conversations.push(row.map_err(|e| format!("Failed to read row: {}", e))?);
    }

    Ok(conversations)
}

/// Get a conversation with its messages
#[tauri::command(async)]
#[specta::specta]
pub fn get_conversation(conversation_id: String) -> Result<ConversationWithMessages, String> {
    let conn = get_db_connection()?;

    // Get conversation
    let conversation: Conversation = conn
        .query_row(
            "SELECT id, title, created_at, updated_at FROM conversations WHERE id = ?1",
            params![conversation_id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .map_err(|e| format!("Conversation not found: {}", e))?;

    // Get messages
    let mut stmt = conn
        .prepare(
            "SELECT id, conversation_id, role, content, created_at
             FROM conversation_messages
             WHERE conversation_id = ?1
             ORDER BY rowid ASC",
        )
        .map_err(|e| format!("Failed to prepare query: {}", e))?;

    let rows = stmt
        .query_map(params![conversation_id], |row| {
            Ok(ConversationMessage {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| format!("Failed to execute query: {}", e))?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(|e| format!("Failed to read row: {}", e))?);
    }

    Ok(ConversationWithMessages {
        conversation,
        messages,
    })
}

/// Save messages to a conversation (replaces existing messages)
#[tauri::command(async)]
#[specta::specta]
pub fn save_conversation_messages(
    conversation_id: String,
    messages: Vec<ConversationMessage>,
) -> Result<(), String> {
    let mut conn = get_db_connection()?;
    let now = Utc::now().to_rfc3339();

    // Replace the whole history atomically; rows are read back in insertion
    // (rowid) order, so the saved order is the conversation order.
    let tx = conn
        .transaction()
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    tx.execute(
        "DELETE FROM conversation_messages WHERE conversation_id = ?1",
        params![conversation_id],
    )
    .map_err(|e| format!("Failed to delete old messages: {}", e))?;

    for msg in messages {
        tx.execute(
            "INSERT INTO conversation_messages (id, conversation_id, role, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                msg.id,
                conversation_id,
                msg.role,
                msg.content,
                msg.created_at
            ],
        )
        .map_err(|e| format!("Failed to insert message: {}", e))?;
    }

    tx.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        params![now, conversation_id],
    )
    .map_err(|e| format!("Failed to update conversation: {}", e))?;

    tx.commit()
        .map_err(|e| format!("Failed to save conversation: {}", e))
}

/// Update conversation title
#[tauri::command(async)]
#[specta::specta]
pub fn update_conversation_title(conversation_id: String, title: String) -> Result<(), String> {
    let conn = get_db_connection()?;
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![title, now, conversation_id],
    )
    .map_err(|e| format!("Failed to update conversation title: {}", e))?;

    Ok(())
}

/// Delete a conversation and its messages
#[tauri::command(async)]
#[specta::specta]
pub fn delete_conversation(conversation_id: String) -> Result<(), String> {
    let conn = get_db_connection()?;

    // Delete messages first (in case foreign key cascade doesn't work)
    conn.execute(
        "DELETE FROM conversation_messages WHERE conversation_id = ?1",
        params![conversation_id],
    )
    .map_err(|e| format!("Failed to delete messages: {}", e))?;

    // Delete conversation
    conn.execute(
        "DELETE FROM conversations WHERE id = ?1",
        params![conversation_id],
    )
    .map_err(|e| format!("Failed to delete conversation: {}", e))?;

    Ok(())
}
