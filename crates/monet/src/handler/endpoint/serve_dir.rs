use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use http::{HeaderValue, Method, StatusCode, header};
use percent_encoding::percent_decode;

use crate::{Endpoint, IntoResponse, Request, Response};

// default capacity 64KiB
const DEFAULT_CAPACITY: usize = 65536;

#[derive(Clone, Debug)]
pub struct ServeDir {
    base: PathBuf,
    buf_chunk_size: usize,
    append_index_html_on_directories: bool,
}
// Todo: Support precompressed_variants
// precompressed_variants: Option<PrecompressedVariants>,

impl ServeDir {
    pub fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let mut base = PathBuf::from(".");
        base.push(path.as_ref());

        Self {
            base,
            buf_chunk_size: DEFAULT_CAPACITY,
            append_index_html_on_directories: true,
        }
    }
}

#[async_trait(?Send)]
impl Endpoint for ServeDir {
    async fn call(&self, req: Request) -> Response {
        if req.method() != Method::GET && req.method() != Method::HEAD {
            return StatusCode::METHOD_NOT_ALLOWED.into_response();
        }

        let Some(path_to_file) = build_and_validate_path(&self.base, req.uri().path()) else {
            return StatusCode::NOT_FOUND.into_response();
        };

        let buf_chunk_size = self.buf_chunk_size;
        let range_header = req
            .headers()
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_owned());

        todo!()
    }
}

fn build_and_validate_path(base_path: &Path, requested_path: &str) -> Option<PathBuf> {
    let path = requested_path.trim_start_matches('/');

    let path_decoded = percent_decode(path.as_ref()).decode_utf8().ok()?;
    let path_decoded = Path::new(&*path_decoded);

    let mut path_to_file = base_path.to_path_buf();
    for component in path_decoded.components() {
        match component {
            Component::Normal(comp) => {
                // protect against paths like `/foo/c:/bar/baz` (#204)
                if Path::new(&comp)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                {
                    #[cfg(windows)]
                    {
                        use std::os::windows::ffi::OsStrExt;
                        if is_reserved_dos_name(|| comp.encode_wide()) {
                            return None;
                        }
                    }

                    path_to_file.push(comp)
                } else {
                    return None;
                }
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return None;
            }
        }
    }
    Some(path_to_file)
}

enum OpenFileOutput {
    FileOpened(Box<FileOpened>),
    Redirect { location: HeaderValue },
    FileNotFound,
    PreconditionFailed,
    NotModified,
    InvalidRedirectUri,
    InvalidFilename,
}

struct FileOpened {
    pub(super) extent: FileRequestExtent,
    pub(super) chunk_size: usize,
    pub(super) mime_header_value: HeaderValue,
    pub(super) maybe_encoding: Option<Encoding>,
    pub(super) maybe_range: Option<Result<Vec<RangeInclusive<u64>>, RangeUnsatisfiableError>>,
    pub(super) last_modified: Option<LastModified>,
}

pub(super) enum FileRequestExtent {
    Full(File, Metadata),
    Head(Metadata),
}
