use std::sync::Arc;

use http::Extensions;
use matchit::Params;

pub(crate) const NEST_TAIL_PARAM: &str = "__private__monet_nest_tail_param";

pub(crate) const NEST_TAIL_PARAM_WILDCARD: &str = "/{*__private__axum_nest_tail_param}";

pub(crate) const FALLBACK_PARAM: &str = "__private__axum_fallback";

pub(crate) const FALLBACK_PARAM_WILDCARD: &str = "/{*__private__axum_fallback}";

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

fn pct_decode<S>(s: S) -> Option<Arc<str>>
where
    S: AsRef<str>,
{
    percent_encoding::percent_decode(s.as_ref().as_bytes())
        .decode_utf8()
        .ok()
        .map(|decoded| decoded.as_ref().into())
}
