pub fn root_response() -> wry::http::Response<Vec<u8>> {
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Wry Test</title>
    <script src="wry://init"></script>
</head>
<body>
</body>
</html>"#,
    );

    wry::http::Response::builder()
        .header("Content-Type", "text/html")
        .header("access-control-allow-origin", "*")
        .body(html.as_bytes().to_vec())
        .map_err(|e| e.to_string())
        .expect("Failed to build response")
}
