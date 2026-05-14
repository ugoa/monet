use std::sync::Arc;

use http::Extensions;
use matchit::Params;

pub(crate) const NEST_TAIL_PARAM: &str = "__private__monet_nest_tail_param";

pub(crate) const NEST_TAIL_PARAM_WILDCARD: &str = "/{*__private__monet_nest_tail_param}";

pub(crate) const FALLBACK_PARAM: &str = "__private__monet_fallback";

#[derive(Clone)]
pub(crate) enum UrlParams {
    Params(Vec<(Arc<str>, Arc<str>)>),
    InvalidUtf8InPathParam { key: Arc<str> },
}

pub(super) fn insert_matched_params(extensions: &mut Extensions, params: &Params<'_, '_>) {
    let current_params = extensions.get_mut();

    if let Some(UrlParams::InvalidUtf8InPathParam { .. }) = current_params {
        // nothing to do here since an error was stored earlier
        return;
    }

    let params = params
        .iter()
        .filter(|(key, _)| !key.starts_with(NEST_TAIL_PARAM))
        .filter(|(key, _)| !key.starts_with(FALLBACK_PARAM))
        .map(|(k, v)| {
            if let Some(decoded) = pct_decode(v) {
                Ok((Arc::from(k), decoded))
            } else {
                Err(Arc::from(k))
            }
        })
        .collect::<Result<Vec<_>, _>>();

    match (current_params, params) {
        (None, Ok(params)) => {
            extensions.insert(UrlParams::Params(params));
        }
        (Some(UrlParams::Params(current)), Ok(params)) => {
            current.extend(params);
        }
        (_, Err(invalid_key)) => {
            extensions.insert(UrlParams::InvalidUtf8InPathParam { key: invalid_key });
        }
        (Some(UrlParams::InvalidUtf8InPathParam { .. }), _) => {
            unreachable!("we check for this state earlier in this method")
        }
    }
}

pub(crate) fn pct_decode<S>(s: S) -> Option<Arc<str>>
where
    S: AsRef<str>,
{
    percent_encoding::percent_decode(s.as_ref().as_bytes())
        .decode_utf8()
        .ok()
        .map(|decoded| decoded.as_ref().into())
}

#[derive(Clone, Debug)]
struct MatchedNestedPath(Arc<str>);

#[derive(Clone, Debug)]
pub struct MatchedPath(pub(crate) Arc<str>);

pub(crate) fn insert_matched_path(ext: &mut Extensions, path: &Arc<str>) {
    let matched_path = append_nested_matched_path(&Arc::new(path), ext);

    if matched_path.ends_with(NEST_TAIL_PARAM_WILDCARD) {
        ext.insert(MatchedNestedPath(matched_path));
        debug_assert!(ext.remove::<MatchedPath>().is_none());
    } else {
        ext.insert(MatchedPath(matched_path));
        ext.remove::<MatchedNestedPath>();
    }
}

pub(crate) fn append_nested_matched_path(
    matched_path: &Arc<str>,
    extensions: &http::Extensions,
) -> Arc<str> {
    if let Some(previous) = extensions
        .get::<MatchedPath>()
        .map(|matched_path| &matched_path.0)
        .or_else(|| Some(&extensions.get::<MatchedNestedPath>()?.0))
    {
        let previous = previous
            .strip_suffix(NEST_TAIL_PARAM_WILDCARD)
            .unwrap_or(previous);

        let matched_path = format!("{previous}{matched_path}");
        matched_path.into()
    } else {
        Arc::clone(matched_path)
    }
}

pub(crate) fn concat_path(prefix: &str, path: &str) -> String {
    debug_assert!(prefix.starts_with('/'));
    debug_assert!(path.starts_with('/'));

    if prefix.ends_with('/') {
        format!("{prefix}{}", path.trim_start_matches('/'))
    } else if path == "/" {
        prefix.into()
    } else {
        format!("{prefix}{path}")
    }
}
