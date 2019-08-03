struct NewMessage {
    username: String,
    message: String,
}

fn parse_form(from_chunk: Chunk) -> FutureResult<NewMessage, hyper::Error> {
    futures::future::ok(NewMessage {
        username: String::new(),
        message: String::new(),
    })
}

fn write_to_db(entry: NewMessage) -> FutureResult<i64, hyper::Error> {
    futures::future::ok(0)
}

fn make_post_response(
    result: Result<i64, hyper::Error>,
) -> FutureResult<hyper::Response, hyper::Error> {
    futures::future::ok(Response::new().with_status(StatusCode::NotFound))
}
