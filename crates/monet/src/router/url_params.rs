use std::sync::Arc;

use http::Extensions;
use matchit::Params;

#[derive(Clone)]
pub(crate) enum UrlParams {
    Params(Vec<(Arc<str>, Arc<str>)>),
    InvalidUtf8InPathParam { key: Arc<str> },
}

pub(super) fn attach_url_params(extensions: &mut Extensions, params: &Params<'_, '_>) {
    let current_params = extensions.get_mut();

    if let Some(UrlParams::InvalidUtf8InPathParam { .. }) = current_params {
        // nothing to do here since an error was stored earlier
        return;
    }

    let params = params
        .iter()
        .filter(|(key, _)| !key.starts_with(super::NEST_TAIL_PARAM))
        // .filter(|(key, _)| !key.starts_with(super::FALLBACK_PARAM))
        .map(|(k, v)| {
            if let Some(decoded) = pct_decode(v) {
                Ok((Arc::from(k), decoded))
            } else {
                Err(Arc::from(k))
            }
        })
        .collect::<Result<Vec<_>, _>>();

    match (current_params, params) {
        (Some(UrlParams::InvalidUtf8InPathParam { .. }), _) => {
            unreachable!("we check for this state earlier in this method")
        }
        (_, Err(invalid_key)) => {
            extensions.insert(UrlParams::InvalidUtf8InPathParam { key: invalid_key });
        }
        (Some(UrlParams::Params(current)), Ok(params)) => {
            current.extend(params);
        }
        (None, Ok(params)) => {
            extensions.insert(UrlParams::Params(params));
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
