use crate::Body;
use crate::BoxError;
use crate::HttpResponse;
use std::borrow::Cow;
use std::convert::Infallible;

pub trait IntoResponse {
    /// Create a response.
    fn into_response(self) -> HttpResponse;
}

impl<B> IntoResponse for HttpResponse<B>
where
    B: http_body::Body<Data = bytes::Bytes> + 'static,
    B::Error: Into<BoxError>,
{
    fn into_response(self) -> HttpResponse {
        self.map(Body::new)
    }
}

impl IntoResponse for Body {
    fn into_response(self) -> HttpResponse {
        HttpResponse::new(self)
    }
}

impl IntoResponse for () {
    fn into_response(self) -> HttpResponse {
        Body::empty().into_response()
    }
}

impl IntoResponse for http::StatusCode {
    fn into_response(self) -> HttpResponse {
        let mut res = ().into_response();
        *res.status_mut() = self;
        res
    }
}

impl IntoResponse for Cow<'static, str> {
    fn into_response(self) -> HttpResponse {
        let res = Body::from(self).into_response();
        // res.headers_mut().insert(
        //     http::header::CONTENT_TYPE,
        //     http::HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
        // );
        res
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> HttpResponse {
        Cow::Borrowed(self).into_response()
    }
}

impl IntoResponse for String {
    fn into_response(self) -> HttpResponse {
        Cow::<'static, str>::Owned(self).into_response()
    }
}

impl IntoResponse for Box<str> {
    fn into_response(self) -> HttpResponse {
        String::from(self).into_response()
    }
}

impl IntoResponse for Infallible {
    fn into_response(self) -> HttpResponse {
        match self {}
    }
}
