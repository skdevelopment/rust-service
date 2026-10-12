//! Official Windows 11 ISO links from Microsoft's consumer download service.
//!
//! This is the same public handshake the Microsoft download page and the Rufus
//! Fido script use. The product-edition ids identify the current consumer ISO
//! (Windows 11 26H2 Home/Pro/Education: 3813 for x64, 3816 for ARM64). They are
//! not a secret, and they have to be updated when Microsoft publishes a new ISO.
//! The download URL itself is short-lived and is never taken from the technician.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use super::assess::MachineKind;

const ORG_ID: &str = "y6jn8c31";
const PROFILE_ID: &str = "606624d44113";
const INSTANCE_ID: &str = "560dc9f3-1aa5-4a2f-b63c-9e18f8d0e175";
pub(super) const REFERER: &str = "https://www.microsoft.com/software-download/windows11";
pub(super) const BROWSER_USER_AGENT: &str = USER_AGENT;
const X64_EDITION_ID: u32 = 3813;
const ARM64_EDITION_ID: u32 = 3816;
const RELEASE_NAME: &str = "Windows 11 26H2";
const EDITION_NAME: &str = "Home, Pro, and Education";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// One Windows 11 language offered by Microsoft's download page.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Windows11IsoLanguage {
    /// Value passed back to the download command. This is Microsoft's language key.
    pub id: String,
    pub name: String,
}

/// Languages for the current consumer Windows 11 ISO.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Windows11IsoCatalog {
    pub release_name: String,
    pub product_name: String,
    pub edition: String,
    pub architecture: String,
    pub languages: Vec<Windows11IsoLanguage>,
    pub suggested_language: String,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedIso {
    pub url: String,
    /// Microsoft's file name, without the download token. Used for the log only.
    pub file_name: String,
}

#[derive(Debug)]
struct Offer {
    id: String,
    name: String,
    sku_id: String,
    product_name: String,
}

struct Prepared {
    session_id: String,
    offers: Vec<Offer>,
}

pub(super) async fn list_languages() -> Result<Windows11IsoCatalog, String> {
    let arch = download_arch()?;
    let prepared = prepare(arch).await?;
    let languages: Vec<Windows11IsoLanguage> = prepared
        .offers
        .iter()
        .map(|offer| Windows11IsoLanguage {
            id: offer.id.clone(),
            name: offer.name.clone(),
        })
        .collect();
    let suggested = suggest_language(&user_locale(), &languages);
    let product_name = prepared
        .offers
        .first()
        .map(|offer| offer.product_name.clone())
        .unwrap_or_else(|| RELEASE_NAME.to_string());
    Ok(Windows11IsoCatalog {
        release_name: RELEASE_NAME.to_string(),
        product_name,
        edition: EDITION_NAME.to_string(),
        architecture: arch.label().to_string(),
        languages,
        suggested_language: suggested,
    })
}

pub(super) async fn resolve_iso(language: &str) -> Result<ResolvedIso, String> {
    let arch = download_arch()?;
    let prepared = prepare(arch).await?;
    let offer = prepared
        .offers
        .iter()
        .find(|offer| offer.id.eq_ignore_ascii_case(language))
        .ok_or_else(|| {
            format!("Microsoft is not offering a {RELEASE_NAME} ISO in that language.")
        })?;
    let url = fetch_link(&prepared.session_id, &offer.sku_id, arch).await?;
    let parsed = official_iso_url(&url)?;
    let file_name = iso_file_name(&parsed);
    Ok(ResolvedIso {
        url: parsed.to_string(),
        file_name,
    })
}

fn download_arch() -> Result<MachineKind, String> {
    match super::assess::native_machine() {
        MachineKind::X86 => {
            Err("Windows 11 installation media is 64-bit only. This PC is 32-bit.".into())
        }
        other => Ok(other),
    }
}

fn edition_id(arch: MachineKind) -> u32 {
    match arch {
        MachineKind::Arm64 => ARM64_EDITION_ID,
        MachineKind::X64 | MachineKind::X86 => X64_EDITION_ID,
    }
}

async fn prepare(arch: MachineKind) -> Result<Prepared, String> {
    let mut last = String::from("Could not reach Microsoft.");
    for attempt in 0..2 {
        if attempt == 1 {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        match prepare_once(arch).await {
            Ok(prepared) if !prepared.offers.is_empty() => return Ok(prepared),
            Ok(_) => last = "Microsoft did not return any Windows 11 languages.".into(),
            Err(error) => last = error,
        }
    }
    Err(last)
}

async fn prepare_once(arch: MachineKind) -> Result<Prepared, String> {
    let client = handshake_client()?;
    let session_id = uuid::Uuid::new_v4().to_string();
    let tags =
        format!("https://vlscppe.microsoft.com/tags?org_id={ORG_ID}&session_id={session_id}");
    get_ok(&client, &tags).await?;

    let mdt_url = format!(
        "https://ov-df.microsoft.com/mdt.js?instanceId={INSTANCE_ID}&PageId=si&session_id={session_id}"
    );
    let mdt_body = get_text(&client, &mdt_url).await?;
    let mdt = parse_mdt(&mdt_body)?;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let reply = format!(
        "https://ov-df.microsoft.com/?session_id={session_id}&CustomerId={INSTANCE_ID}&PageId=si&w={}&mdt={now_ms}&rticks={}",
        mdt.w, mdt.rticks
    );
    get_ok(&client, &reply).await?;

    let sku_url = format!(
        "https://www.microsoft.com/software-download-connector/api/getskuinformationbyproductedition?profile={PROFILE_ID}&productEditionId={}&SKU=undefined&friendlyFileName=undefined&Locale=en-US&sessionID={session_id}",
        edition_id(arch)
    );
    let body = get_text(&client, &sku_url).await?;
    let offers = parse_offers(&body)?;
    Ok(Prepared { session_id, offers })
}

async fn fetch_link(session_id: &str, sku_id: &str, arch: MachineKind) -> Result<String, String> {
    let client = handshake_client()?;
    let url = format!(
        "https://www.microsoft.com/software-download-connector/api/GetProductDownloadLinksBySku?profile={PROFILE_ID}&productEditionId=undefined&SKU={sku_id}&friendlyFileName=undefined&Locale=en-US&sessionID={session_id}"
    );
    let response = client
        .get(&url)
        .header(reqwest::header::REFERER, REFERER)
        .send()
        .await
        .map_err(net_err)?;
    let status = response.status();
    let body = response.text().await.map_err(net_err)?;
    if !status.is_success() {
        return Err(format!(
            "Microsoft returned HTTP {status} for the ISO link."
        ));
    }
    parse_download_link(&body, arch)
}

fn handshake_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| format!("Could not start the download client: {e}"))
}

async fn get_ok(client: &reqwest::Client, url: &str) -> Result<(), String> {
    let response = client.get(url).send().await.map_err(net_err)?;
    let status = response.status();
    // Finish the body so the connection is not left half-open.
    let _ = response.bytes().await;
    if status.is_success() {
        Ok(())
    } else {
        Err(format!("Microsoft returned HTTP {status}."))
    }
}

async fn get_text(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let response = client.get(url).send().await.map_err(net_err)?;
    let status = response.status();
    let body = response.text().await.map_err(net_err)?;
    if status.is_success() {
        Ok(body)
    } else {
        Err(format!("Microsoft returned HTTP {status}."))
    }
}

struct Mdt {
    w: String,
    rticks: String,
}

fn parse_mdt(body: &str) -> Result<Mdt, String> {
    let w = capture(body, r"[?&]w=([A-Fa-f0-9]+)")
        .ok_or_else(|| "Microsoft did not return a download session.".to_string())?;
    let rticks = capture(body, r#"rticks\s*=\s*"?\+?(\d+)"#)
        .ok_or_else(|| "Microsoft did not return a download session.".to_string())?;
    Ok(Mdt { w, rticks })
}

fn capture(text: &str, pattern: &str) -> Option<String> {
    let regex = regex::Regex::new(pattern).ok()?;
    regex
        .captures(text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

fn parse_offers(body: &str) -> Result<Vec<Offer>, String> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| "Microsoft returned an unexpected language list.".to_string())?;
    if let Some(message) = explain_errors(&value) {
        return Err(message);
    }
    let skus = value
        .get("Skus")
        .and_then(Value::as_array)
        .ok_or_else(|| "Microsoft returned an unexpected language list.".to_string())?;
    let mut offers = Vec::new();
    for sku in skus {
        let Some(id) = sku.get("Language").and_then(Value::as_str) else {
            continue;
        };
        let Some(sku_id) = json_id(sku.get("Id")) else {
            continue;
        };
        let name = sku
            .get("LocalizedLanguage")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or(id);
        let product_name = sku
            .get("ProductDisplayName")
            .and_then(Value::as_str)
            .unwrap_or(RELEASE_NAME);
        if offers.iter().any(|offer: &Offer| offer.id == id) {
            continue;
        }
        offers.push(Offer {
            id: id.to_string(),
            name: name.to_string(),
            sku_id,
            product_name: product_name.to_string(),
        });
    }
    offers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    if offers.is_empty() {
        Err("Microsoft did not return any Windows 11 languages.".into())
    } else {
        Ok(offers)
    }
}

pub(super) fn parse_download_link(body: &str, arch: MachineKind) -> Result<String, String> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| "Microsoft returned an unexpected download link.".to_string())?;
    if let Some(message) = explain_errors(&value) {
        return Err(message);
    }
    let options = value
        .get("ProductDownloadOptions")
        .and_then(Value::as_array)
        .ok_or_else(|| "Microsoft returned an unexpected download link.".to_string())?;
    let want = match arch {
        MachineKind::X64 => 1,
        MachineKind::Arm64 => 2,
        MachineKind::X86 => {
            return Err("Windows 11 installation media is 64-bit only. This PC is 32-bit.".into());
        }
    };
    let link = options.iter().find_map(|option| {
        let kind = option.get("DownloadType").and_then(Value::as_i64)?;
        if kind != want {
            return None;
        }
        option
            .get("Uri")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    link.ok_or_else(|| format!("Microsoft did not offer a {} Windows 11 ISO.", arch.label()))
}

fn explain_errors(value: &Value) -> Option<String> {
    let mut errors = Vec::new();
    push_errors(value.get("Errors"), &mut errors);
    push_errors(value.pointer("/ValidationContainer/Errors"), &mut errors);
    if errors.is_empty() {
        return None;
    }
    if errors.iter().any(|error| error.kind == Some(9)) {
        return Some(
            "Microsoft refused the ISO request (code 715-123130). This network has asked for too many downloads. Wait and try again.".into(),
        );
    }
    let text = errors
        .iter()
        .filter_map(|error| error.message.clone())
        .collect::<Vec<_>>()
        .join(" ");
    if text.trim().is_empty() {
        Some("Microsoft refused the ISO request.".into())
    } else {
        Some(text)
    }
}

struct ApiProblem {
    kind: Option<i64>,
    message: Option<String>,
}

fn push_errors(value: Option<&Value>, out: &mut Vec<ApiProblem>) {
    let Some(items) = value.and_then(Value::as_array) else {
        return;
    };
    for item in items {
        if let Some(problem) = api_problem(item) {
            out.push(problem);
        }
    }
}

fn api_problem(value: &Value) -> Option<ApiProblem> {
    match value {
        Value::String(text) if !text.is_empty() => Some(ApiProblem {
            kind: None,
            message: Some(text.clone()),
        }),
        Value::Object(_) => Some(ApiProblem {
            kind: value.get("Type").and_then(Value::as_i64),
            message: value
                .get("Value")
                .and_then(Value::as_str)
                .map(str::to_string),
        }),
        _ => None,
    }
}

fn json_id(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

pub(super) const ISO_REDIRECT_LIMIT: usize = 10;

/// A hop after the official ISO link. The final host may be a Microsoft CDN.
/// The scheme stays HTTPS. `previous_hops` includes the original URL.
pub(super) fn iso_redirect_target(url: &str, previous_hops: usize) -> Result<(), String> {
    if previous_hops > ISO_REDIRECT_LIMIT {
        return Err("The ISO download was redirected too many times.".into());
    }
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| "The ISO download was redirected to a bad address.".to_string())?;
    if parsed.scheme() != "https" {
        return Err("The ISO download was redirected off HTTPS.".into());
    }
    Ok(())
}

pub(super) fn official_iso_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| "Microsoft returned a download link that is not a URL.".to_string())?;
    if parsed.scheme() != "https" {
        return Err("Microsoft returned a download link that is not HTTPS.".into());
    }
    let host = parsed.host_str().unwrap_or("");
    let allowed = host == "software.download.prss.microsoft.com"
        || host.ends_with(".download.prss.microsoft.com");
    if !allowed {
        return Err(format!(
            "Microsoft returned an unexpected download server ({host})."
        ));
    }
    if !parsed.path().to_ascii_lowercase().ends_with(".iso") {
        return Err("Microsoft did not return an ISO file.".into());
    }
    Ok(parsed)
}

pub(super) fn iso_file_name(url: &reqwest::Url) -> String {
    let raw = url.path().rsplit('/').next().unwrap_or("Windows11.iso");
    let safe: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.len() > 4 && safe.to_ascii_lowercase().ends_with(".iso") {
        safe
    } else {
        "Windows11.iso".to_string()
    }
}

struct LocaleNames {
    prefix: &'static str,
    names: &'static [&'static str],
}

/// Longest matching prefix wins, so `en-us` beats `en` and `es-es` beats `es`.
const LOCALE_NAMES: &[LocaleNames] = &[
    LocaleNames {
        prefix: "en-us",
        names: &["English"],
    },
    LocaleNames {
        prefix: "en",
        names: &["English (United Kingdom)", "English"],
    },
    LocaleNames {
        prefix: "pt-br",
        names: &["Brazilian Portuguese"],
    },
    LocaleNames {
        prefix: "pt",
        names: &["Portuguese", "Brazilian Portuguese"],
    },
    LocaleNames {
        prefix: "zh-tw",
        names: &["Chinese (Traditional)"],
    },
    LocaleNames {
        prefix: "zh-hk",
        names: &["Chinese (Traditional)"],
    },
    LocaleNames {
        prefix: "zh-mo",
        names: &["Chinese (Traditional)"],
    },
    LocaleNames {
        prefix: "zh",
        names: &["Chinese (Simplified)"],
    },
    LocaleNames {
        prefix: "fr-ca",
        names: &["French Canadian", "French"],
    },
    LocaleNames {
        prefix: "fr",
        names: &["French"],
    },
    LocaleNames {
        prefix: "de",
        names: &["German"],
    },
    LocaleNames {
        prefix: "es-es",
        names: &["Spanish"],
    },
    LocaleNames {
        prefix: "es",
        names: &["Spanish (Mexico)", "Spanish"],
    },
    LocaleNames {
        prefix: "nl",
        names: &["Dutch"],
    },
    LocaleNames {
        prefix: "pl",
        names: &["Polish"],
    },
    LocaleNames {
        prefix: "it",
        names: &["Italian"],
    },
    LocaleNames {
        prefix: "ja",
        names: &["Japanese"],
    },
    LocaleNames {
        prefix: "ko",
        names: &["Korean"],
    },
    LocaleNames {
        prefix: "ru",
        names: &["Russian"],
    },
    LocaleNames {
        prefix: "sv",
        names: &["Swedish"],
    },
    LocaleNames {
        prefix: "da",
        names: &["Danish"],
    },
    LocaleNames {
        prefix: "fi",
        names: &["Finnish"],
    },
    LocaleNames {
        prefix: "nb",
        names: &["Norwegian"],
    },
    LocaleNames {
        prefix: "nn",
        names: &["Norwegian"],
    },
    LocaleNames {
        prefix: "no",
        names: &["Norwegian"],
    },
    LocaleNames {
        prefix: "cs",
        names: &["Czech"],
    },
    LocaleNames {
        prefix: "hu",
        names: &["Hungarian"],
    },
    LocaleNames {
        prefix: "tr",
        names: &["Turkish"],
    },
    LocaleNames {
        prefix: "ar",
        names: &["Arabic"],
    },
    LocaleNames {
        prefix: "he",
        names: &["Hebrew"],
    },
    LocaleNames {
        prefix: "uk",
        names: &["Ukrainian"],
    },
    LocaleNames {
        prefix: "el",
        names: &["Greek"],
    },
    LocaleNames {
        prefix: "ro",
        names: &["Romanian"],
    },
    LocaleNames {
        prefix: "sk",
        names: &["Slovak"],
    },
    LocaleNames {
        prefix: "sl",
        names: &["Slovenian", "Slovene"],
    },
    LocaleNames {
        prefix: "hr",
        names: &["Croatian"],
    },
    LocaleNames {
        prefix: "bg",
        names: &["Bulgarian"],
    },
    LocaleNames {
        prefix: "et",
        names: &["Estonian"],
    },
    LocaleNames {
        prefix: "lv",
        names: &["Latvian"],
    },
    LocaleNames {
        prefix: "lt",
        names: &["Lithuanian"],
    },
    LocaleNames {
        prefix: "th",
        names: &["Thai"],
    },
    LocaleNames {
        prefix: "vi",
        names: &["Vietnamese"],
    },
    LocaleNames {
        prefix: "id",
        names: &["Indonesian"],
    },
    LocaleNames {
        prefix: "sr",
        names: &["Serbian Latin", "Serbian"],
    },
];

fn preferred_names(locale: &str) -> &'static [&'static str] {
    if locale.is_empty() {
        return &["English"];
    }
    let mut best: Option<&LocaleNames> = None;
    for choice in LOCALE_NAMES {
        if locale.starts_with(choice.prefix)
            && best.is_none_or(|current| choice.prefix.len() > current.prefix.len())
        {
            best = Some(choice);
        }
    }
    best.map(|choice| choice.names).unwrap_or(&["English"])
}

pub(super) fn suggest_language(locale: &str, languages: &[Windows11IsoLanguage]) -> String {
    let locale = locale.trim().to_ascii_lowercase();
    let preferred = preferred_names(&locale);
    for name in preferred {
        if let Some(language) = languages
            .iter()
            .find(|language| language.id.eq_ignore_ascii_case(name))
        {
            return language.id.clone();
        }
    }
    languages
        .first()
        .map(|language| language.id.clone())
        .unwrap_or_default()
}

fn user_locale() -> String {
    #[cfg(windows)]
    {
        let mut buf = [0u16; 85];
        let len = unsafe {
            winapi::um::winnls::GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32)
        };
        if len > 1 {
            return String::from_utf16_lossy(&buf[..(len as usize - 1)]);
        }
    }
    "en-US".to_string()
}

fn net_err(error: reqwest::Error) -> String {
    let raw = redact_urls(&error.to_string());
    if error.is_timeout() {
        "The request to Microsoft timed out.".into()
    } else if error.is_connect() {
        "Could not connect to Microsoft. Check the network and try again.".into()
    } else {
        format!("Could not reach Microsoft: {raw}")
    }
}

pub(super) fn redact_urls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find("http://").or_else(|| rest.find("https://")) {
        out.push_str(&rest[..index]);
        let url = &rest[index..];
        let end = url.find(char::is_whitespace).unwrap_or(url.len());
        let base = url[..end].split('?').next().unwrap_or(&url[..end]);
        out.push_str(base);
        rest = &url[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(id: &str) -> Windows11IsoLanguage {
        Windows11IsoLanguage {
            id: id.to_string(),
            name: id.to_string(),
        }
    }

    #[test]
    fn mdt_script_yields_the_session_fields() {
        let body = r#"src="https://ov-df.microsoft.com/?session_id=abc&w=8DF278C2D7641F9";src+="&rticks="+1791718712152;"#;
        let mdt = parse_mdt(body).unwrap();
        assert_eq!(mdt.w, "8DF278C2D7641F9");
        assert_eq!(mdt.rticks, "1791718712152");
    }

    #[test]
    fn sku_list_keeps_language_names_and_numeric_ids() {
        let body = r#"{
            "ValidationContainer": { "Errors": [] },
            "Skus": [
                {"Id":"27160","Language":"English","LocalizedLanguage":"English (United States)","ProductDisplayName":"Windows 11 Client - Build 26300.9457"},
                {"Id":27134,"Language":"French","LocalizedLanguage":"French","ProductDisplayName":"Windows 11 Client - Build 26300.9457"}
            ]
        }"#;
        let offers = parse_offers(body).unwrap();
        assert_eq!(offers.len(), 2);
        assert_eq!(offers[0].id, "English");
        assert_eq!(offers[1].sku_id, "27134");
    }

    #[test]
    fn banned_session_is_explained_without_the_raw_payload() {
        let body = r#"{"Errors":[{"Type":9,"Value":"too many"}]}"#;
        let error = parse_offers(body).unwrap_err();
        assert!(error.contains("715-123130"));
    }

    #[test]
    fn download_link_picks_the_x64_iso() {
        let body = r#"{"ProductDownloadOptions":[
            {"DownloadType":2,"Uri":"https://software.download.prss.microsoft.com/arm.iso?t=1"},
            {"DownloadType":1,"Uri":"https://software.download.prss.microsoft.com/dbazure/Win.iso?t=secret"}
        ]}"#;
        let url = parse_download_link(body, MachineKind::X64).unwrap();
        assert!(url.contains("Win.iso"));
        assert!(url.contains("secret"));
    }

    #[test]
    fn official_host_is_required() {
        assert!(official_iso_url(
            "https://software.download.prss.microsoft.com/dbazure/Win.iso?t=1"
        )
        .is_ok());
        assert!(official_iso_url("https://evil.example/Win.iso").is_err());
        assert!(official_iso_url("http://software.download.prss.microsoft.com/Win.iso").is_err());
        assert!(official_iso_url("https://software.download.prss.microsoft.com/Win.exe").is_err());
    }

    #[test]
    fn file_name_comes_from_the_iso_path() {
        let url = official_iso_url(
            "https://software.download.prss.microsoft.com/dbazure/Windows11_Client_x64_en-us.iso?t=secret",
        )
        .unwrap();
        assert_eq!(iso_file_name(&url), "Windows11_Client_x64_en-us.iso");
    }

    #[test]
    fn locale_picks_a_matching_language() {
        let languages = vec![
            lang("English"),
            lang("English (United Kingdom)"),
            lang("German"),
            lang("French"),
            lang("Spanish"),
            lang("Spanish (Mexico)"),
            lang("Chinese (Traditional)"),
            lang("Chinese (Simplified)"),
        ];
        assert_eq!(suggest_language("de-DE", &languages), "German");
        assert_eq!(suggest_language("en-US", &languages), "English");
        assert_eq!(
            suggest_language("en-GB", &languages),
            "English (United Kingdom)"
        );
        assert_eq!(suggest_language("es-ES", &languages), "Spanish");
        assert_eq!(suggest_language("es-MX", &languages), "Spanish (Mexico)");
        assert_eq!(
            suggest_language("zh-TW", &languages),
            "Chinese (Traditional)"
        );
        assert_eq!(
            suggest_language("zh-CN", &languages),
            "Chinese (Simplified)"
        );
        assert_eq!(suggest_language("ja-JP", &languages), "English");
        assert_eq!(suggest_language("", &languages), "English");
    }

    #[test]
    fn iso_redirects_stay_on_https_and_may_leave_the_first_host() {
        assert!(iso_redirect_target("https://cdn.example/windows.iso", 1).is_ok());
        assert!(iso_redirect_target("http://cdn.example/windows.iso", 1).is_err());
        assert!(iso_redirect_target("https://cdn.example/windows.iso", ISO_REDIRECT_LIMIT).is_ok());
        assert!(
            iso_redirect_target("https://cdn.example/windows.iso", ISO_REDIRECT_LIMIT + 1).is_err()
        );
    }

    #[test]
    fn redact_urls_drops_the_download_token() {
        let text =
            redact_urls("failed https://software.download.prss.microsoft.com/a.iso?t=secret more");
        assert!(!text.contains("secret"));
        assert!(text.contains("https://software.download.prss.microsoft.com/a.iso"));
    }
}
