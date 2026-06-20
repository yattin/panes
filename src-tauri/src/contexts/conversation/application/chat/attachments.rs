use super::*;

pub(super) async fn save_pasted_image_attachment(
    file_name: String,
    mime_type: String,
    data_base64: String,
) -> Result<ChatAttachmentPayload, String> {
    let normalized_mime = mime_type.trim().to_lowercase();
    if !normalized_mime.starts_with("image/") {
        return Err("Pasted attachment is not an image.".to_string());
    }

    let encoded = data_base64
        .split_once(',')
        .map(|(_, data)| data)
        .unwrap_or(data_base64.as_str())
        .trim();
    let bytes = BASE64
        .decode(encoded)
        .map_err(|_| "Pasted image data is not valid base64.".to_string())?;
    if bytes.is_empty() {
        return Err("Pasted image data is empty.".to_string());
    }
    if bytes.len() > MAX_PASTED_IMAGE_ATTACHMENT_BYTES {
        return Err("Pasted image exceeds the 10 MB attachment limit.".to_string());
    }

    let extension = pasted_image_extension(&file_name, &normalized_mime)
        .ok_or_else(|| "Pasted image type is not supported.".to_string())?;
    let stored_file_name = format!("pasted-image-{}.{}", Uuid::new_v4().simple(), extension);
    let attachment_dir = runtime_env::app_data_dir()
        .join("attachments")
        .join("pasted-images");
    tokio_fs::create_dir_all(&attachment_dir)
        .await
        .map_err(|error| format!("failed to create pasted image attachment directory: {error}"))?;
    let file_path = attachment_dir.join(&stored_file_name);
    tokio_fs::write(&file_path, &bytes)
        .await
        .map_err(|error| format!("failed to save pasted image attachment: {error}"))?;

    Ok(ChatAttachmentPayload {
        file_name: stored_file_name,
        file_path: file_path.display().to_string(),
        size_bytes: bytes.len() as u64,
        mime_type: Some(normalized_mime),
    })
}

pub(super) async fn read_attachment_preview(
    file_path: String,
    mime_type: Option<String>,
) -> Result<Option<AttachmentPreviewPayload>, String> {
    let file_path = file_path.trim().to_string();
    if file_path.is_empty() {
        return Ok(None);
    }

    let Some(preview_mime_type) =
        normalize_image_preview_mime_type(&file_path, mime_type.as_deref())
    else {
        return Ok(None);
    };

    let metadata = tokio_fs::metadata(&file_path)
        .await
        .map_err(|error| format!("failed to read attachment metadata: {error}"))?;
    if !metadata.is_file() {
        return Ok(None);
    }
    if metadata.len() > MAX_PASTED_IMAGE_ATTACHMENT_BYTES as u64 {
        return Ok(None);
    }

    let bytes = tokio_fs::read(&file_path)
        .await
        .map_err(|error| format!("failed to read attachment preview: {error}"))?;

    Ok(Some(AttachmentPreviewPayload {
        mime_type: preview_mime_type,
        data_base64: BASE64.encode(bytes),
    }))
}

pub(super) fn pasted_image_extension(_file_name: &str, mime_type: &str) -> Option<&'static str> {
    match mime_type {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/bmp" => Some("bmp"),
        "image/tiff" => Some("tiff"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

pub(super) fn normalize_image_preview_mime_type(
    file_path: &str,
    mime_type: Option<&str>,
) -> Option<String> {
    if let Some(mime_type) = mime_type.map(str::trim).filter(|value| !value.is_empty()) {
        let normalized = mime_type.to_lowercase();
        if normalized.starts_with("image/") {
            return Some(match normalized.as_str() {
                "image/jpg" => "image/jpeg".to_string(),
                _ => normalized,
            });
        }
    }

    let extension = Path::new(file_path)
        .extension()
        .and_then(|value| value.to_str())?
        .to_lowercase();
    image_mime_type_for_extension(&extension).map(ToOwned::to_owned)
}

pub(super) fn image_mime_type_for_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}
