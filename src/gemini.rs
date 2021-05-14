use async_native_tls::TlsConnector;
use async_std::net::TcpStream;
use async_std::prelude::*;
use url::Url;

#[derive(Debug)]
pub enum GeminiStatus {
    Input(u32, String),
    Success(u32, String),
    Redirect(u32, String),
    TemporaryFailure(u32, String),
    PermanentFailure(u32, String),
    ClientCertificateRequired(u32, String),
    InvalidResponse(String),
}

impl GeminiStatus {
    pub fn from_response(response: &str) -> GeminiStatus {
        if response.len() < 2 {
            return GeminiStatus::InvalidResponse(response.to_string());
        }
        let (code, meta) = response.split_at(2);
        let code = if let Ok(val) = code.parse::<u32>() {
            val
        } else {
            return GeminiStatus::InvalidResponse(response.to_string());
        };
        let meta = String::from(meta.trim());
        if code >= 10 && code < 20 {
            GeminiStatus::Input(code, meta)
        } else if code >= 20 && code < 30 {
            GeminiStatus::Success(code, meta)
        } else if code >= 30 && code < 40 {
            GeminiStatus::Redirect(code, meta)
        } else if code >= 40 && code < 50 {
            GeminiStatus::TemporaryFailure(code, meta)
        } else if code >= 50 && code < 60 {
            GeminiStatus::PermanentFailure(code, meta)
        } else if code >= 60 && code < 70 {
            GeminiStatus::ClientCertificateRequired(code, meta)
        } else {
            GeminiStatus::InvalidResponse(response.to_string())
        }
    }

    pub fn success(&self) -> bool {
        matches!(self, GeminiStatus::Success(_, _))
    }
}

pub async fn request(base: &str, url: &str) -> (String, String) {
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
                format!("Could not get TLS certificate for URL {}\n{:?}", url, e),
            )
        }
    };

    match stream.write_all(format!("{}\r\n", url).as_bytes()).await {
        Ok(_) => {}
        Err(e) => return (url, format!("Error writing to socket: {:?}", e)),
    };

    let mut res = Vec::new();
    match stream.read_to_end(&mut res).await {
        Ok(_) => {}
        Err(e) => return (url, format!("Error reading from socket: {:?}", e)),
    };

    let response = String::from_utf8_lossy(&res);
    if let Some((response, contents)) = response.split_once("\r\n") {
        let status = GeminiStatus::from_response(response);
        if status.success() {
            (url, String::from(contents))
        } else {
            return (
                url.clone(),
                format!(
                    "Unexpected response from server loading URL: {}\n{:?}",
                    url, status
                ),
            );
        }
    } else {
        (
            url.clone(),
            format!("Invalid response from server loading URL: {}", url),
        )
    }
}
