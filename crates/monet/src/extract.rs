use http::{HeaderMap, header};

pub mod query;

pub(super) fn has_content_type(headers: &HeaderMap, expected_content_type: &mime::Mime) -> bool {
    let Some(content_type) = headers.get(header::CONTENT_TYPE) else {
        return false;
    };

    let Ok(content_type) = content_type.to_str() else {
        return false;
    };

    content_type.starts_with(expected_content_type.as_ref())
}
