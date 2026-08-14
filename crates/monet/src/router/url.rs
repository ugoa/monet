use std::rc::Rc;

use matchit::Params;

use crate::request::Extensions;

pub(crate) const NEST_TAIL_PARAM: &str = "__private__monet_nest_tail_param";

pub(crate) const FALLBACK_PARAM: &str = "__private__monet_fallback";

pub(crate) const NEST_TAIL_PARAM_WILDCARD: &str = "/{*__private__monet_nest_tail_param}";

#[derive(Clone, Debug)]
pub(crate) enum UrlParams {
    PairParams(Vec<(Rc<str>, Rc<str>)>),
    InvalidUtf8Param { key: Rc<str> },
}

pub(super) fn insert_matched_params(ext: &mut Extensions, params: &Params<'_, '_>) {
    let current_params: Option<&mut UrlParams> = ext.get_mut();

    if let Some(UrlParams::InvalidUtf8Param { .. }) = current_params {
        // nothing to do here since an error was stored earlier
        return;
    }

    let pair_params: Result<Vec<(Rc<str>, Rc<str>)>, Rc<str>> = params
        .iter()
        .filter(|(key, _)| !key.starts_with(NEST_TAIL_PARAM))
        .filter(|(key, _)| !key.starts_with(FALLBACK_PARAM))
        .map(|(k, v)| {
            percent_decode(v)
                .map(|decoded| (Rc::from(k), decoded))
                .ok_or(Rc::from(k))
        })
        .collect();

    match (current_params, pair_params) {
        // Brand new pair of key/value, set it
        (None, Ok(params)) => {
            ext.insert(UrlParams::PairParams(params));
        }
        // both params exist and valid, extend it
        (Some(UrlParams::PairParams(current)), Ok(params)) => {
            current.extend(params);
        }
        // If new params is invalid, set it as invalid
        (_, Err(invalid_key)) => {
            ext.insert(UrlParams::InvalidUtf8Param { key: invalid_key });
        }
        (Some(UrlParams::InvalidUtf8Param { .. }), _) => {
            unreachable!("we check for this state earlier in this method")
        }
    }
}

/*
 * assert_eq!(percent_decode(b"foo%20bar%3f").decode_utf8().unwrap(), "foo bar?");
 */
fn percent_decode<S>(s: S) -> Option<Rc<str>>
where
    S: AsRef<str>,
{
    percent_encoding::percent_decode(s.as_ref().as_bytes())
        .decode_utf8()
        .ok()
        .map(|decoded| decoded.as_ref().into())
}

#[derive(Clone, Debug)]
pub struct MatchedNestedPath(pub Rc<str>);

#[derive(Clone, Debug)]
pub struct MatchedPath(pub Rc<str>);

// Todo: Add testing
pub(crate) fn insert_matched_path(state: &mut Extensions, path: &Rc<str>) {
    let matched_path = {
        if let Some(previous) = state
            .get::<MatchedPath>()
            .map(|matched_path| &matched_path.0)
            .or_else(|| Some(&state.get::<MatchedNestedPath>()?.0))
        {
            let previous = previous
                .strip_suffix(NEST_TAIL_PARAM_WILDCARD)
                .unwrap_or(previous);

            let matched_path = format!("{previous}{path}");
            matched_path.into()
        } else {
            Rc::clone(path)
        }
    };

    if matched_path.ends_with(NEST_TAIL_PARAM_WILDCARD) {
        state.insert(MatchedNestedPath(matched_path));
        debug_assert!(state.remove::<MatchedPath>().is_none());
    } else {
        state.insert(MatchedPath(matched_path));
        state.remove::<MatchedNestedPath>();
    }
}
