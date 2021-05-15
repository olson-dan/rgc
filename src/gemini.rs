use async_native_tls::TlsConnector;
use async_std::net::TcpStream;
use async_std::prelude::*;
use url::Url;

#[derive(Debug)]
pub enum GeminiStatus<'a> {
    Input(u32, &'a str),
    Success(u32, &'a str),
    Redirect(u32, &'a str),
    TemporaryFailure(u32, &'a str),
    PermanentFailure(u32, &'a str),
    ClientCertificateRequired(u32, &'a str),
    InvalidResponse(&'a str),
}

impl<'a> GeminiStatus<'a> {
    pub fn from_response(response: &str) -> GeminiStatus<'_> {
        if response.len() < 2 {
            return GeminiStatus::InvalidResponse(response);
        }
        let (code, meta) = response.split_at(2);
        let code = if let Ok(val) = code.parse::<u32>() {
            val
        } else {
            return GeminiStatus::InvalidResponse(response);
        };
        let meta = meta.trim();
        match code {
            10..=19 => GeminiStatus::Input(code, meta),
            20..=29 => GeminiStatus::Success(code, meta),
            30..=39 => GeminiStatus::Redirect(code, meta),
            40..=49 => GeminiStatus::TemporaryFailure(code, meta),
            50..=59 => GeminiStatus::PermanentFailure(code, meta),
            60..=69 => GeminiStatus::ClientCertificateRequired(code, meta),
            _ => GeminiStatus::InvalidResponse(response),
        }
    }
}

pub async fn request(base: &str, url: &str) -> (String, String, String) {
    let plain = "text/plain".to_string();
    let mut url = url.to_string();
    let parsed_url = if base.is_empty() {
        if !url.contains("://") {
            url = format!("gemini://{}", url);
        }
        Url::parse(&url).unwrap()
    } else {
        let parsed_base = Url::parse(base).unwrap();
        parsed_base.join(&url).unwrap()
    };
    let url = parsed_url.to_string();
    let host = parsed_url.host_str().unwrap();

    let stream = match TcpStream::connect(format!("{}:1965", host)).await {
        Ok(s) => s,
        Err(e) => {
            return (
                url.clone(),
                plain,
                format!("Could connect to server: {}\n{:?}", url, e),
            )
        }
    };
    let mut stream = match TlsConnector::new()
        .use_sni(true)
        .danger_accept_invalid_certs(true)
        .connect(host, stream)
        .await
    {
        Ok(s) => s,
        Err(e) => {
            return (
                url.clone(),
                plain,
                format!("Could not get TLS certificate for URL {}\n{:?}", url, e),
            )
        }
    };

    match stream.write_all(format!("{}\r\n", url).as_bytes()).await {
        Ok(_) => {}
        Err(e) => return (url, plain, format!("Error writing to socket: {:?}", e)),
    };

    let mut res = Vec::new();
    match stream.read_to_end(&mut res).await {
        Ok(_) => {}
        Err(e) => return (url, plain, format!("Error reading from socket: {:?}", e)),
    };

    let response = String::from_utf8_lossy(&res);
    if let Some((response, contents)) = response.split_once("\r\n") {
        let status = GeminiStatus::from_response(response);
        match status {
            GeminiStatus::Success(_, meta) => {
                (url, meta.trim().to_string(), String::from(contents))
            }
            _ => (
                url.clone(),
                plain,
                format!(
                    "Unexpected response from server loading URL: {}\n{:?}",
                    url, status
                ),
            ),
        }
    } else {
        (
            url.clone(),
            plain,
            format!("Invalid response from server loading URL: {}", url),
        )
    }
}
