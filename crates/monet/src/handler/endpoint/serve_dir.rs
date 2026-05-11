mod headers;

use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use bytes::Bytes;
use compio::{fs::File, io::AsyncReadAtExt};
use http::{HeaderValue, Method, StatusCode, Uri, header};
use percent_encoding::percent_decode;

use crate::{
    Endpoint, IntoResponse, Request, Response,
    handler::endpoint::serve_dir::headers::{
        IfModifiedSince, LastModified, check_modified_headers,
    },
};

// default capacity 64KiB
const DEFAULT_CAPACITY: usize = 65536;

#[derive(Clone, Debug)]
pub struct ServeDir {
    base: PathBuf,
    buf_chunk_size: usize,
    append_index_html_on_dir: bool,
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
            append_index_html_on_dir: true,
        }
    }
}

#[async_trait(?Send)]
impl Endpoint for ServeDir {
    async fn call(&self, req: Request) -> Response {
        if req.method() != Method::GET && req.method() != Method::HEAD {
            return StatusCode::METHOD_NOT_ALLOWED.into_response();
        }

        let Some(path) = build_and_validate_path(&self.base, req.uri().path()) else {
            return StatusCode::NOT_FOUND.into_response();
        };

        let buf_size = self.buf_chunk_size;
        let append = self.append_index_html_on_dir;

        match open_file(req, path, buf_size, append).await {
            Ok(OpenFileOutput::FileOpened(file_output)) => build_response(*file_output).await,
            Err(_) => panic!("normal error"),
            _ => panic!("fetal error"),
        }
    }
}

async fn build_response(output: FileOpened) -> Response {
    let (maybe_file, size) = match output.extent {
        FileRequestExtent::Full(file, size) => (Some(file), size),
        FileRequestExtent::Head(size) => (None, size),
    };
    // Load file all at once into buffer, not good for big files. Room for improve to use stream
    let mut resp = if let Some(file) = maybe_file {
        let (_, buffer) = file
            .read_to_end_at(Vec::with_capacity(65536), 0)
            .await
            .unwrap();

        let bytes: Bytes = buffer.into();
        bytes.into_response()
    } else {
        ().into_response()
    };

    let headers = resp.headers_mut();
    headers.insert(header::CONTENT_TYPE, output.mime);
    headers.insert(header::CONTENT_LENGTH, size.into());

    // TODO support partial request with ranges
    // headers.insert(header::ACCEPT_RANGES, "bytes");

    if let Some(last_modified) = output.last_modified {
        headers.insert(
            header::LAST_MODIFIED,
            HeaderValue::from_str(&last_modified.0.to_string()).unwrap(),
        );
    }
    resp
}

pub(super) async fn open_file(
    req: Request,
    mut file_path: PathBuf,
    buf_size: usize,
    append: bool,
) -> std::io::Result<OpenFileOutput> {
    if let Some(output) = maybe_redirect_or_append_index(&mut file_path, req.uri(), append).await {
        return Ok(output);
    }

    let mime = mime_guess::from_path(&file_path)
        .first_raw()
        .map(HeaderValue::from_static)
        .unwrap_or_else(|| HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()));

    let if_unmodified_since = req
        .headers()
        .get(header::IF_UNMODIFIED_SINCE)
        .and_then(headers::IfUnmodifiedSince::from_header_value);

    let if_modified_since = req
        .headers()
        .get(header::IF_MODIFIED_SINCE)
        .and_then(IfModifiedSince::from_header_value);

    if req.method() == Method::HEAD {
        let meta = compio::fs::metadata(&file_path).await?;
        let last_modified = meta.modified().ok().map(LastModified::from);

        if let Some(output) = check_modified_headers(
            last_modified.as_ref(),
            if_unmodified_since,
            if_modified_since,
        ) {
            return Ok(output);
        }

        let file_opened = FileOpened {
            extent: FileRequestExtent::Head(meta.len()),
            chunk_size: buf_size,
            mime,
            last_modified,
        };
        Ok(OpenFileOutput::FileOpened(Box::new(file_opened)))
    } else {
        dbg!(&file_path);
        let file = match File::open(&file_path).await {
            Ok(file) => file,
            // Only applies to NULL bytes
            Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => {
                return Ok(OpenFileOutput::InvalidFilename);
            }
            Err(err) => {
                dbg!(&err);
                return Err(err);
            }
        };

        let meta = file.metadata().await?;
        let last_modified = meta.modified().ok().map(LastModified::from);
        if let Some(output) = check_modified_headers(
            last_modified.as_ref(),
            if_unmodified_since,
            if_modified_since,
        ) {
            return Ok(output);
        }

        let file_opened = FileOpened {
            extent: FileRequestExtent::Full(file, meta.len()),
            chunk_size: buf_size,
            mime,
            last_modified,
        };
        Ok(OpenFileOutput::FileOpened(Box::new(file_opened)))
    }
}

async fn maybe_redirect_or_append_index(
    path_to_file: &mut PathBuf,
    uri: &Uri,
    append_index_html_on_dir: bool,
) -> Option<OpenFileOutput> {
    // Check if the path exists and is a Dir, return if false
    if !compio::fs::metadata(&path_to_file)
        .await
        .is_ok_and(|m| m.is_dir())
    {
        return None;
    }

    // Found dir, but we are not allowed to give the ./index.html, so return file not found
    if !append_index_html_on_dir {
        return Some(OpenFileOutput::FileNotFound);
    }

    if uri.path().ends_with('/') {
        path_to_file.push("index.html");
        return None;
    }

    match append_slash_on_path(uri.clone()) {
        Ok(uri) => {
            let location = HeaderValue::from_str(&uri.to_string()).unwrap();
            Some(OpenFileOutput::Redirect { location })
        }
        Err(err) => Some(err),
    }
}

fn append_slash_on_path(uri: Uri) -> Result<Uri, OpenFileOutput> {
    let http::uri::Parts {
        scheme,
        authority,
        path_and_query,
        ..
    } = uri.into_parts();

    let mut uri_builder = Uri::builder();

    if let Some(scheme) = scheme {
        uri_builder = uri_builder.scheme(scheme);
    }

    if let Some(authority) = authority {
        uri_builder = uri_builder.authority(authority);
    }

    let uri_builder = if let Some(path_and_query) = path_and_query {
        if let Some(query) = path_and_query.query() {
            uri_builder.path_and_query(format!("{}/?{}", path_and_query.path(), query))
        } else {
            uri_builder.path_and_query(format!("{}/", path_and_query.path()))
        }
    } else {
        uri_builder.path_and_query("/")
    };

    uri_builder.build().map_err(|_err| {
        #[cfg(not(feature = "no-tracing"))]
        tracing::error!(err = ?_err, "redirect uri failed to build");

        OpenFileOutput::InvalidRedirectUri
    })
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

pub(crate) enum OpenFileOutput {
    FileOpened(Box<FileOpened>),
    Redirect { location: HeaderValue },
    FileNotFound,
    PreconditionFailed,
    NotModified,
    InvalidRedirectUri,
    InvalidFilename,
}

pub(crate) struct FileOpened {
    pub(super) extent: FileRequestExtent,
    pub(super) chunk_size: usize,
    pub(super) mime: HeaderValue,
    pub(super) last_modified: Option<headers::LastModified>,
}

pub(crate) enum FileRequestExtent {
    Full(File, u64),
    Head(u64),
}
