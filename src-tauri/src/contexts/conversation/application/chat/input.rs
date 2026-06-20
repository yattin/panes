use super::*;

pub(super) fn build_user_blocks(
    message: &str,
    input_items: &[TurnInputItem],
    attachments: &[TurnAttachment],
    plan_mode: bool,
    is_steer: bool,
) -> Vec<ContentBlock> {
    let mut user_blocks = Vec::with_capacity(
        input_items
            .len()
            .saturating_add(attachments.len())
            .saturating_add(1),
    );

    let mut structured_text_parts: Vec<String> = Vec::new();

    for item in input_items {
        match item {
            TurnInputItem::Skill { name, path } => {
                user_blocks.push(ContentBlock::Skill {
                    name: name.clone(),
                    path: path.clone(),
                });
            }
            TurnInputItem::Mention { name, path } => {
                user_blocks.push(ContentBlock::Mention {
                    name: name.clone(),
                    path: path.clone(),
                });
            }
            TurnInputItem::Text { text } => {
                structured_text_parts.push(text.clone());
            }
        }
    }

    for attachment in attachments {
        user_blocks.push(ContentBlock::Attachment {
            file_name: attachment.file_name.clone(),
            file_path: attachment.file_path.clone(),
            size_bytes: attachment.size_bytes,
            mime_type: attachment.mime_type.clone(),
        });
    }

    let final_text = if structured_text_parts.is_empty() {
        message.to_string()
    } else {
        structured_text_parts.join("\n")
    };
    user_blocks.push(ContentBlock::Text {
        content: final_text,
        plan_mode: if plan_mode { Some(true) } else { None },
        is_steer: if is_steer { Some(true) } else { None },
    });

    user_blocks
}

pub(super) fn normalize_input_items(
    message: &str,
    input_items: Option<Vec<ChatInputItemPayload>>,
) -> Result<Vec<TurnInputItem>, String> {
    let mut normalized = Vec::new();

    for item in input_items.unwrap_or_default() {
        match item {
            ChatInputItemPayload::Text { text } => {
                if !text.is_empty() {
                    normalized.push(TurnInputItem::Text { text });
                }
            }
            ChatInputItemPayload::Skill { name, path } => {
                let name = name.trim();
                let path = path.trim();
                if name.is_empty() || path.is_empty() {
                    return Err("skill input items require non-empty name and path".to_string());
                }
                normalized.push(TurnInputItem::Skill {
                    name: name.to_string(),
                    path: path.to_string(),
                });
            }
            ChatInputItemPayload::Mention { name, path } => {
                let name = name.trim();
                let path = path.trim();
                if name.is_empty() || path.is_empty() {
                    return Err("mention input items require non-empty name and path".to_string());
                }
                normalized.push(TurnInputItem::Mention {
                    name: name.to_string(),
                    path: path.to_string(),
                });
            }
        }
    }

    if normalized.is_empty() {
        normalized.push(TurnInputItem::Text {
            text: message.to_string(),
        });
        return Ok(normalized);
    }

    let has_text_item = normalized
        .iter()
        .any(|item| matches!(item, TurnInputItem::Text { text } if !text.is_empty()));
    if !has_text_item && !message.trim().is_empty() {
        return Err(
            "input items must include at least one text segment when message text is provided"
                .to_string(),
        );
    }

    let mut merged = Vec::with_capacity(normalized.len());
    for item in normalized {
        match item {
            TurnInputItem::Text { text } => {
                if let Some(TurnInputItem::Text { text: current }) = merged.last_mut() {
                    current.push_str(&text);
                } else {
                    merged.push(TurnInputItem::Text { text });
                }
            }
            other => merged.push(other),
        }
    }

    Ok(merged)
}

pub(super) fn normalize_attachments(
    attachments: Option<Vec<ChatAttachmentPayload>>,
) -> Result<Vec<TurnAttachment>, String> {
    let attachments = attachments.unwrap_or_default();
    if attachments.len() > MAX_ATTACHMENTS_PER_TURN {
        return Err(format!(
            "You can attach at most {MAX_ATTACHMENTS_PER_TURN} files per turn."
        ));
    }

    let mut normalized = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        let file_path = attachment.file_path.trim().to_string();
        if file_path.is_empty() {
            return Err("Attachment path cannot be empty.".to_string());
        }

        let file_name = if attachment.file_name.trim().is_empty() {
            Path::new(&file_path)
                .file_name()
                .and_then(|value| value.to_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| file_path.clone())
        } else {
            attachment.file_name.trim().to_string()
        };

        normalized.push(TurnAttachment {
            file_name,
            file_path,
            size_bytes: attachment.size_bytes,
            mime_type: attachment.mime_type,
        });
    }

    Ok(normalized)
}

pub(super) fn validate_attachments_for_engine_model(
    attachments: &[TurnAttachment],
    engine_id: &str,
    model_id: &str,
    catalog: Option<&[EngineInfoDto]>,
) -> Result<(), String> {
    if attachments.is_empty() {
        return Ok(());
    }

    let Some(model) = catalog
        .and_then(|engines| engines.iter().find(|engine| engine.id == engine_id))
        .and_then(|engine| engine.models.iter().find(|model| model.id == model_id))
    else {
        return Ok(());
    };

    let allowed_modalities = if model.attachment_modalities.is_empty() {
        HashSet::new()
    } else {
        model
            .attachment_modalities
            .iter()
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>()
    };

    if allowed_modalities.is_empty() {
        return Err(format!(
            "{} does not support file attachments.",
            model.display_name
        ));
    }

    for attachment in attachments {
        let Some(modality) = attachment_modality(attachment) else {
            return Err(format!(
                "{} is not a supported attachment type for {}.",
                attachment.file_name, model.display_name
            ));
        };
        if !allowed_modalities.contains(modality) {
            return Err(format!(
                "{} attachments are not supported by {}.",
                attachment_modality_label(modality),
                model.display_name
            ));
        }
    }

    Ok(())
}

pub(super) fn attachment_modality(attachment: &TurnAttachment) -> Option<&'static str> {
    let extension = Path::new(&attachment.file_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_lowercase);
    let mime_type = attachment
        .mime_type
        .as_deref()
        .map(str::trim)
        .map(str::to_lowercase);

    if extension.as_deref() == Some("pdf") || mime_type.as_deref() == Some("application/pdf") {
        return Some("pdf");
    }
    if extension
        .as_deref()
        .map(|value| IMAGE_ATTACHMENT_EXTENSIONS.contains(&value))
        .unwrap_or(false)
        || mime_type
            .as_deref()
            .map(|value| value.starts_with("image/"))
            .unwrap_or(false)
    {
        return Some("image");
    }
    if extension
        .as_deref()
        .map(|value| TEXT_ATTACHMENT_EXTENSIONS.contains(&value))
        .unwrap_or(false)
        || mime_type
            .as_deref()
            .map(is_text_attachment_mime_type)
            .unwrap_or(false)
    {
        return Some("text");
    }

    None
}

pub(super) fn is_text_attachment_mime_type(value: &str) -> bool {
    value.starts_with("text/")
        || matches!(
            value,
            "application/json"
                | "application/javascript"
                | "application/typescript"
                | "application/xml"
                | "application/x-sh"
                | "application/x-yaml"
                | "application/yaml"
                | "text/csv"
        )
}

pub(super) fn attachment_modality_label(modality: &str) -> &'static str {
    match modality {
        "image" => "Image",
        "pdf" => "PDF",
        _ => "Text file",
    }
}
