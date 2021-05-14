use async_native_tls::TlsConnector;
use async_std::net::TcpStream;
use async_std::prelude::*;
use url_escape::encode_fragment;
use urlparse::urlparse;

#[derive(Debug)]
pub enum GeminiStatus {
    Input(u32, String),
    Success(u32, String),
    Redirect(u32, String),
    TemporaryFailure(u32, String),
    PermanentFailure(u32, String),
    ClientCertificateRequired(u32, String),
    InvalidResponse,
}

impl GeminiStatus {
    pub fn from_response(response: &str) -> GeminiStatus {
        if response.len() < 3 {
            return GeminiStatus::InvalidResponse;
        }
        let (code, meta) = response.split_at(2);
        let code = if let Ok(val) = code.parse::<u32>() {
            val
        } else {
            return GeminiStatus::InvalidResponse;
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
            GeminiStatus::InvalidResponse
        }
    }

    pub fn success(&self) -> bool {
        matches!(self, GeminiStatus::Success(_, _))
    }
}

pub async fn request(url: &str) -> (String, String) {
    println!("{}", url);
    let mut url = encode_fragment(&url).to_string();
    if !url.contains("://") {
        url = format!("gemini://{}", url);
    }

    let parsed_url = urlparse(&url);
    let stream = match TcpStream::connect(format!("{}:1965", parsed_url.netloc)).await {
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
        .connect(&parsed_url.netloc, stream)
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
