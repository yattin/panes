use super::*;

const LEGACY_NATIVE_ENGINE_ID: &str = "claude-code-native";
const CLAURST_NATIVE_ENGINE_ID: &str = "claurst-native";
const CLAURST_NATIVE_LABEL: &str = "CueLight Agent";
const MIGRATION_METADATA_KEY: &str = "legacyNativeMigration";
const MIGRATION_NOTICE_KIND: &str = "legacy_native_migrated";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LegacyNativeMigration {
    pub metadata_changed: bool,
    pub notice: Option<LegacyNativeMigrationNotice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LegacyNativeMigrationNotice {
    pub kind: String,
    pub level: String,
    pub title: String,
    pub message: String,
}

impl LegacyNativeMigrationNotice {
    pub(super) fn into_engine_event(self) -> EngineEvent {
        EngineEvent::Notice {
            kind: self.kind,
            level: self.level,
            title: self.title,
            message: self.message,
        }
    }
}

pub(super) fn migrate_legacy_native_thread_metadata(
    thread: &mut ThreadDto,
) -> Option<LegacyNativeMigration> {
    if thread.engine_id != LEGACY_NATIVE_ENGINE_ID {
        return None;
    }

    let original_metadata = thread.engine_metadata.clone();
    let mut metadata = thread
        .engine_metadata
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));
    if !metadata.is_object() {
        metadata = serde_json::json!({});
    }

    let object = metadata
        .as_object_mut()
        .expect("metadata was normalized to object");
    let already_notified = object
        .get(MIGRATION_METADATA_KEY)
        .and_then(Value::as_object)
        .and_then(|migration| migration.get("noticeShown"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    object.remove("nativeHistoryTokens");
    object.remove("nativeContextTokens");
    object.remove("nativeContextMaxTokens");
    object.remove("nativeCompactionAvailable");
    object.remove("claudeCodeRustVersion");
    object.remove("claudeCodeRustVendorPath");

    object.insert(
        MIGRATION_METADATA_KEY.to_string(),
        serde_json::json!({
            "fromEngine": LEGACY_NATIVE_ENGINE_ID,
            "toEngine": CLAURST_NATIVE_ENGINE_ID,
            "runtimeLabel": CLAURST_NATIVE_LABEL,
            "noticeShown": true,
            "version": 1
        }),
    );

    thread.engine_metadata = Some(metadata);
    let metadata_changed = thread.engine_metadata != original_metadata;
    let notice = (!already_notified).then(|| LegacyNativeMigrationNotice {
        kind: MIGRATION_NOTICE_KIND.to_string(),
        level: "info".to_string(),
        title: "Runtime migrated".to_string(),
        message: "This legacy claude-code-native thread is now handled by CueLight Agent on the claurst-native runtime.".to_string(),
    });

    Some(LegacyNativeMigration {
        metadata_changed,
        notice,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_thread(metadata: Option<Value>) -> ThreadDto {
        ThreadDto {
            id: "thread-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            repo_id: None,
            engine_id: LEGACY_NATIVE_ENGINE_ID.to_string(),
            model_id: "claude-sonnet-4-5".to_string(),
            engine_thread_id: Some("engine-thread-1".to_string()),
            engine_metadata: metadata,
            title: "Legacy".to_string(),
            status: ThreadStatusDto::Idle,
            message_count: 0,
            total_tokens: 0,
            created_at: "2026-06-21T00:00:00Z".to_string(),
            last_activity_at: "2026-06-21T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn migrates_legacy_native_metadata_and_cleans_removed_runtime_fields() {
        let mut thread = legacy_thread(Some(serde_json::json!({
            "lastModelId": "claude-opus-4-1",
            "nativeHistoryTokens": 123,
            "nativeCompactionAvailable": true,
            "claudeCodeRustVendorPath": "vendor/claude-code-rust"
        })));

        let migration = migrate_legacy_native_thread_metadata(&mut thread)
            .expect("legacy thread should return migration outcome");
        let notice = migration
            .notice
            .expect("first migration should emit notice");
        let metadata = thread.engine_metadata.expect("metadata should be present");
        let migration_marker = metadata
            .get(MIGRATION_METADATA_KEY)
            .and_then(Value::as_object)
            .expect("migration marker should be present");

        assert_eq!(
            migration_marker.get("fromEngine").and_then(Value::as_str),
            Some(LEGACY_NATIVE_ENGINE_ID)
        );
        assert_eq!(
            migration_marker.get("toEngine").and_then(Value::as_str),
            Some(CLAURST_NATIVE_ENGINE_ID)
        );
        assert_eq!(
            migration_marker.get("noticeShown").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            metadata.get("lastModelId").and_then(Value::as_str),
            Some("claude-opus-4-1")
        );
        assert!(metadata.get("nativeHistoryTokens").is_none());
        assert!(metadata.get("nativeCompactionAvailable").is_none());
        assert!(metadata.get("claudeCodeRustVendorPath").is_none());
        assert!(migration.metadata_changed);
        assert_eq!(notice.kind, MIGRATION_NOTICE_KIND);
    }

    #[test]
    fn migration_notice_is_one_time_for_legacy_threads() {
        let mut thread = legacy_thread(Some(serde_json::json!({
            "legacyNativeMigration": {
                "fromEngine": "claude-code-native",
                "toEngine": "claurst-native",
                "runtimeLabel": "CueLight Agent",
                "noticeShown": true,
                "version": 1
            }
        })));

        let migration = migrate_legacy_native_thread_metadata(&mut thread)
            .expect("legacy thread should return migration outcome");

        assert!(migration.notice.is_none());
        assert!(!migration.metadata_changed);
    }

    #[test]
    fn cleans_stale_fields_even_when_notice_was_already_shown() {
        let mut thread = legacy_thread(Some(serde_json::json!({
            "nativeContextTokens": 88,
            "legacyNativeMigration": {
                "fromEngine": "claude-code-native",
                "toEngine": "claurst-native",
                "runtimeLabel": "CueLight Agent",
                "noticeShown": true,
                "version": 1
            }
        })));

        let migration = migrate_legacy_native_thread_metadata(&mut thread)
            .expect("legacy thread should return migration outcome");
        let metadata = thread.engine_metadata.expect("metadata should be present");

        assert!(migration.notice.is_none());
        assert!(migration.metadata_changed);
        assert!(metadata.get("nativeContextTokens").is_none());
    }

    #[test]
    fn ignores_claurst_native_threads() {
        let mut thread = legacy_thread(None);
        thread.engine_id = CLAURST_NATIVE_ENGINE_ID.to_string();

        assert!(migrate_legacy_native_thread_metadata(&mut thread).is_none());
        assert!(thread.engine_metadata.is_none());
    }
}
