// SPDX-License-Identifier: MIT
//! Independently implemented device-code/Xbox/Minecraft protocol. Requests are
//! restricted to fixed HTTPS endpoints; secrets and response bodies are never errors.
use crate::{MICROSOFT_CLIENT_ID, OfflineProfile};
use curl::easy::{Easy, List, NetRc};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;
use std::{fmt, time::Duration};

pub(super) const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    Network,
    InvalidResponse,
    Cancelled,
    Expired,
    Declined,
    ReauthenticationRequired,
    ApplicationConfiguration,
    ApplicationBlocked,
    NoEntitlement,
    ProfileRequired,
    XboxAccountRequired,
    XboxFamilyRestricted,
    XboxRegionRestricted,
    VaultUnavailable,
    Service { name: &'static str, status: u32 },
    Storage,
}
impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network => f.write_str("登录连接失败或超时，请重试"),
            Self::InvalidResponse => f.write_str("登录服务返回异常，请重新登录"),
            Self::Cancelled => f.write_str("已取消 Microsoft 登录"),
            Self::Expired => f.write_str("登录代码已过期，请重新获取"),
            Self::Declined => f.write_str("登录授权已拒绝，请重新登录"),
            Self::ReauthenticationRequired => f.write_str("登录已失效，请重新登录 Microsoft 账号"),
            Self::ApplicationConfiguration => {
                f.write_str("启动器的 Microsoft 登录暂不可用，请联系维护者")
            }
            Self::ApplicationBlocked => {
                f.write_str("启动器的 Minecraft 正版登录暂不可用，请联系维护者")
            }
            Self::NoEntitlement => f.write_str("这个账号没有 Minecraft Java 版游戏资格"),
            Self::ProfileRequired => {
                f.write_str("请先在 minecraft.net 创建 Java 版玩家档案，再重新登录")
            }
            Self::XboxAccountRequired => {
                f.write_str("请先为这个 Microsoft 账号创建 Xbox 档案，再重新登录")
            }
            Self::XboxFamilyRestricted => {
                f.write_str("请先在 Microsoft 账号中处理 Xbox 年龄或家庭设置")
            }
            Self::XboxRegionRestricted => f.write_str("这个账号所在地区暂不支持 Xbox Live"),
            Self::VaultUnavailable => f.write_str("系统凭证库不可用或已锁定，请解锁后重新登录"),
            Self::Service { name, status } => {
                write!(f, "{name} 登录服务暂不可用（HTTP {status}），请重试")
            }
            Self::Storage => f.write_str("未能保存玩家档案，请重新登录"),
        }
    }
}
impl std::error::Error for AuthError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Endpoint {
    Device,
    MicrosoftToken,
    Xbox,
    Xsts,
    MinecraftToken,
    Entitlements,
    Profile,
}
impl Endpoint {
    fn url(self) -> &'static str {
        match self {
            Self::Device => "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode",
            Self::MicrosoftToken => "https://login.microsoftonline.com/consumers/oauth2/v2.0/token",
            Self::Xbox => "https://user.auth.xboxlive.com/user/authenticate",
            Self::Xsts => "https://xsts.auth.xboxlive.com/xsts/authorize",
            Self::MinecraftToken => {
                "https://api.minecraftservices.com/authentication/login_with_xbox"
            }
            Self::Entitlements => "https://api.minecraftservices.com/entitlements/mcstore",
            Self::Profile => "https://api.minecraftservices.com/minecraft/profile",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Device | Self::MicrosoftToken => "Microsoft",
            Self::Xbox => "Xbox Live",
            Self::Xsts => "XSTS",
            _ => "Minecraft Services",
        }
    }
}

pub(super) struct Response {
    pub status: u32,
    pub body: Vec<u8>,
}
pub(super) trait Transport {
    fn send(
        &self,
        endpoint: Endpoint,
        body: Option<(&str, bool)>,
        bearer: Option<&str>,
    ) -> Result<Response, AuthError>;
}
pub(super) struct Http;
impl Transport for Http {
    fn send(
        &self,
        endpoint: Endpoint,
        body: Option<(&str, bool)>,
        bearer: Option<&str>,
    ) -> Result<Response, AuthError> {
        let mut easy = Easy::new();
        let network = |_: curl::Error| AuthError::Network;
        easy.url(endpoint.url()).map_err(network)?;
        easy.follow_location(false).map_err(network)?;
        easy.netrc(NetRc::Ignored).map_err(network)?;
        easy.ssl_verify_peer(true).map_err(network)?;
        easy.ssl_verify_host(true).map_err(network)?;
        easy.connect_timeout(Duration::from_secs(10))
            .map_err(network)?;
        easy.timeout(Duration::from_secs(30)).map_err(network)?;
        easy.useragent("CobbleLauncher/0.1").map_err(network)?;
        let mut headers = List::new();
        headers
            .append("Accept: application/json")
            .map_err(network)?;
        if let Some((body, form)) = body {
            easy.post(true).map_err(network)?;
            easy.post_fields_copy(body.as_bytes()).map_err(network)?;
            headers
                .append(if form {
                    "Content-Type: application/x-www-form-urlencoded"
                } else {
                    "Content-Type: application/json"
                })
                .map_err(network)?;
        }
        if let Some(token) = bearer {
            headers
                .append(&format!("Authorization: Bearer {token}"))
                .map_err(network)?;
        }
        easy.http_headers(headers).map_err(network)?;
        let mut bytes = Vec::new();
        let mut exceeded = false;
        {
            let mut transfer = easy.transfer();
            transfer
                .write_function(|chunk| {
                    if bytes.len().saturating_add(chunk.len()) > 512 * 1024 {
                        exceeded = true;
                        return Ok(0);
                    }
                    bytes.extend_from_slice(chunk);
                    Ok(chunk.len())
                })
                .map_err(network)?;
            let result = transfer.perform();
            if result.is_err() {
                return Err(AuthError::Network);
            }
        }
        if exceeded {
            return Err(AuthError::InvalidResponse);
        }
        Ok(Response {
            status: easy.response_code().map_err(network)?,
            body: bytes,
        })
    }
}

pub(super) fn form(fields: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields.iter().copied())
        .finish()
}
fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, AuthError> {
    serde_json::from_slice(bytes).map_err(|_| AuthError::InvalidResponse)
}
fn successful<T: DeserializeOwned>(response: Response, endpoint: Endpoint) -> Result<T, AuthError> {
    if response.status != 200 {
        return Err(match (endpoint, response.status) {
            (Endpoint::MinecraftToken | Endpoint::Entitlements | Endpoint::Profile, 403) => {
                AuthError::ApplicationBlocked
            }
            (Endpoint::Profile, 404) => AuthError::ProfileRequired,
            (_, 401) => AuthError::ReauthenticationRequired,
            _ => AuthError::Service {
                name: endpoint.name(),
                status: response.status,
            },
        });
    }
    decode(&response.body)
}

#[derive(Deserialize)]
pub(super) struct DeviceResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}
fn default_interval() -> u64 {
    5
}
pub(super) fn device(http: &impl Transport) -> Result<DeviceResponse, AuthError> {
    let response = http.send(
        Endpoint::Device,
        Some((
            &form(&[("client_id", MICROSOFT_CLIENT_ID), ("scope", SCOPE)]),
            true,
        )),
        None,
    )?;
    if response.status != 200 {
        return Err(oauth_response_error(&response));
    }
    let device: DeviceResponse = decode(&response.body)?;
    if device.device_code.is_empty()
        || device.device_code.len() > 8192
        || device.user_code.is_empty()
        || device.user_code.len() > 32
        || !device
            .user_code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || device.expires_in == 0
        || device.expires_in > 3600
        || device.interval == 0
        || device.interval > 60
    {
        return Err(AuthError::InvalidResponse);
    }
    verification_uri(&device.verification_uri)?;
    Ok(device)
}
pub(super) fn verification_uri(uri: &str) -> Result<(), AuthError> {
    match uri {
        "https://microsoft.com/devicelogin"
        | "https://www.microsoft.com/devicelogin"
        | "https://microsoft.com/link"
        | "https://www.microsoft.com/link" => Ok(()),
        _ => Err(AuthError::InvalidResponse),
    }
}

#[derive(Deserialize)]
pub(super) struct MicrosoftToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
}
pub(super) enum TokenPoll {
    Pending,
    SlowDown,
    Complete(MicrosoftToken),
}
pub(super) fn poll(http: &impl Transport, device_code: &str) -> Result<TokenPoll, AuthError> {
    let response = http.send(
        Endpoint::MicrosoftToken,
        Some((
            &form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", MICROSOFT_CLIENT_ID),
                ("device_code", device_code),
            ]),
            true,
        )),
        None,
    )?;
    if response.status == 200 {
        return Ok(TokenPoll::Complete(decode(&response.body)?));
    }
    if response.status == 429 || response.status >= 500 {
        return Err(oauth_response_error(&response));
    }
    let code: OAuthError = decode(&response.body)?;
    match code.error.as_str() {
        "authorization_pending" => Ok(TokenPoll::Pending),
        "slow_down" => Ok(TokenPoll::SlowDown),
        _ => Err(oauth_error(&response.body)),
    }
}
#[derive(Deserialize)]
struct OAuthError {
    error: String,
}
fn oauth_response_error(response: &Response) -> AuthError {
    if response.status == 429 || response.status >= 500 {
        AuthError::Service {
            name: "Microsoft",
            status: response.status,
        }
    } else {
        oauth_error(&response.body)
    }
}
fn oauth_error(bytes: &[u8]) -> AuthError {
    let Ok(error) = decode::<OAuthError>(bytes) else {
        return AuthError::InvalidResponse;
    };
    match error.error.as_str() {
        "authorization_declined" | "access_denied" => AuthError::Declined,
        "expired_token" | "code_expired" => AuthError::Expired,
        "invalid_client" | "unauthorized_client" | "invalid_scope" => {
            AuthError::ApplicationConfiguration
        }
        "invalid_grant" | "interaction_required" | "consent_required" | "bad_verification_code" => {
            AuthError::ReauthenticationRequired
        }
        _ => AuthError::Service {
            name: "Microsoft",
            status: 400,
        },
    }
}
pub(super) fn refresh(
    http: &impl Transport,
    refresh_token: &str,
) -> Result<MicrosoftToken, AuthError> {
    let response = http.send(
        Endpoint::MicrosoftToken,
        Some((
            &form(&[
                ("grant_type", "refresh_token"),
                ("client_id", MICROSOFT_CLIENT_ID),
                ("refresh_token", refresh_token),
                ("scope", SCOPE),
            ]),
            true,
        )),
        None,
    )?;
    if response.status != 200 {
        return Err(oauth_response_error(&response));
    }
    decode(&response.body)
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxToken {
    token: String,
    display_claims: Claims,
}
#[derive(Deserialize)]
struct Claims {
    xui: Vec<XboxUser>,
}
#[derive(Deserialize)]
struct XboxUser {
    uhs: String,
    #[serde(default)]
    xid: String,
}
#[derive(Deserialize)]
struct MinecraftToken {
    access_token: String,
    expires_in: u64,
}
#[derive(Deserialize)]
struct Entitlements {
    items: Vec<Entitlement>,
}
#[derive(Deserialize)]
struct Entitlement {
    name: String,
}
#[derive(Deserialize)]
struct MinecraftProfile {
    id: String,
    name: String,
}

pub(super) struct Session {
    pub name: String,
    pub uuid: String,
    pub token: String,
    pub xuid: String,
    pub expires_in: u64,
}
pub(super) fn exchange(http: &impl Transport, access_token: &str) -> Result<Session, AuthError> {
    if access_token.is_empty() {
        return Err(AuthError::InvalidResponse);
    }
    let body=json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={access_token}")},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"}).to_string();
    let xbox: XboxToken = successful(
        http.send(Endpoint::Xbox, Some((&body, false)), None)?,
        Endpoint::Xbox,
    )?;
    let body=json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[xbox.token]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"}).to_string();
    let response = http.send(Endpoint::Xsts, Some((&body, false)), None)?;
    if response.status != 200 {
        #[derive(Deserialize)]
        struct XstsError {
            #[serde(rename = "XErr")]
            code: u64,
        }
        if let Ok(error) = decode::<XstsError>(&response.body) {
            return Err(match error.code {
                2148916233 => AuthError::XboxAccountRequired,
                2148916235 => AuthError::XboxRegionRestricted,
                2148916236..=2148916238 => AuthError::XboxFamilyRestricted,
                _ => AuthError::Service {
                    name: "XSTS",
                    status: response.status,
                },
            });
        }
    }
    let xsts: XboxToken = successful(response, Endpoint::Xsts)?;
    if xbox.display_claims.xui.len() != 1
        || xsts.display_claims.xui.len() != 1
        || xsts.token.is_empty()
    {
        return Err(AuthError::InvalidResponse);
    }
    let user = &xsts.display_claims.xui[0];
    if user.uhs.is_empty() || user.uhs != xbox.display_claims.xui[0].uhs {
        return Err(AuthError::InvalidResponse);
    }
    let body = json!({"identityToken":format!("XBL3.0 x={};{}",user.uhs,xsts.token)}).to_string();
    let minecraft: MinecraftToken = successful(
        http.send(Endpoint::MinecraftToken, Some((&body, false)), None)?,
        Endpoint::MinecraftToken,
    )?;
    if minecraft.access_token.is_empty()
        || minecraft.expires_in <= 60
        || minecraft.expires_in > 7 * 86400
    {
        return Err(AuthError::InvalidResponse);
    }
    let entitlements: Entitlements = successful(
        http.send(Endpoint::Entitlements, None, Some(&minecraft.access_token))?,
        Endpoint::Entitlements,
    )?;
    if !entitlements
        .items
        .iter()
        .any(|item| matches!(item.name.as_str(), "game_minecraft" | "product_minecraft"))
    {
        return Err(AuthError::NoEntitlement);
    }
    let profile: MinecraftProfile = successful(
        http.send(Endpoint::Profile, None, Some(&minecraft.access_token))?,
        Endpoint::Profile,
    )?;
    let uuid = uuid::Uuid::parse_str(&profile.id)
        .map_err(|_| AuthError::InvalidResponse)?
        .simple()
        .to_string();
    if profile.name.is_empty()
        || profile.name.len() > 16
        || !profile
            .name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(AuthError::InvalidResponse);
    }
    Ok(Session {
        name: profile.name,
        uuid,
        token: minecraft.access_token,
        xuid: user.xid.clone(),
        expires_in: minecraft.expires_in,
    })
}

pub(super) fn offline_valid(profile: &OfflineProfile) -> Result<(), AuthError> {
    if OfflineProfile::new(&profile.name).map_err(|_| AuthError::InvalidResponse)? != *profile {
        return Err(AuthError::InvalidResponse);
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod fixtures {
    use super::*;
    use std::{collections::VecDeque, sync::Mutex};
    pub struct Fixture {
        replies: Mutex<VecDeque<(Endpoint, u32, serde_json::Value)>>,
        pub calls: Mutex<Vec<(Endpoint, String, Option<String>)>>,
    }
    impl Fixture {
        pub fn new(replies: Vec<(Endpoint, u32, serde_json::Value)>) -> Self {
            Self {
                replies: Mutex::new(replies.into()),
                calls: Mutex::new(Vec::new()),
            }
        }
        pub fn chain() -> Vec<(Endpoint, u32, serde_json::Value)> {
            vec![
                (
                    Endpoint::Xbox,
                    200,
                    json!({"Token":"synthetic-xbox","DisplayClaims":{"xui":[{"uhs":"synthetic-user"}]}}),
                ),
                (
                    Endpoint::Xsts,
                    200,
                    json!({"Token":"synthetic-xsts","DisplayClaims":{"xui":[{"uhs":"synthetic-user","xid":"12345"}]}}),
                ),
                (
                    Endpoint::MinecraftToken,
                    200,
                    json!({"access_token":"synthetic-minecraft-secret","expires_in":86400}),
                ),
                (
                    Endpoint::Entitlements,
                    200,
                    json!({"items":[{"name":"game_minecraft"}]}),
                ),
                (
                    Endpoint::Profile,
                    200,
                    json!({"id":"0123456789abcdef0123456789abcdef","name":"OnlineName"}),
                ),
            ]
        }
    }
    impl Transport for Fixture {
        fn send(
            &self,
            endpoint: Endpoint,
            body: Option<(&str, bool)>,
            bearer: Option<&str>,
        ) -> Result<Response, AuthError> {
            self.calls.lock().unwrap().push((
                endpoint,
                body.map(|(body, _)| body.to_owned()).unwrap_or_default(),
                bearer.map(str::to_owned),
            ));
            let (expected, status, body) = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(AuthError::InvalidResponse)?;
            assert_eq!(expected, endpoint);
            Ok(Response {
                status,
                body: serde_json::to_vec(&body).unwrap(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::Fixture;
    #[test]
    fn complete_chain_binds_the_xbox_user_and_only_sends_minecraft_bearer_to_minecraft() {
        let http = Fixture::new(Fixture::chain());
        let session = exchange(&http, "synthetic-ms-secret").unwrap();
        assert_eq!(session.name, "OnlineName");
        assert_eq!(session.uuid, "0123456789abcdef0123456789abcdef");
        assert_eq!(session.xuid, "12345");
        let calls = http.calls.lock().unwrap();
        let xbox: serde_json::Value = serde_json::from_str(&calls[0].1).unwrap();
        assert_eq!(xbox["Properties"]["RpsTicket"], "d=synthetic-ms-secret");
        let xsts: serde_json::Value = serde_json::from_str(&calls[1].1).unwrap();
        assert_eq!(xsts["RelyingParty"], "rp://api.minecraftservices.com/");
        let minecraft: serde_json::Value = serde_json::from_str(&calls[2].1).unwrap();
        assert_eq!(
            minecraft["identityToken"],
            "XBL3.0 x=synthetic-user;synthetic-xsts"
        );
        assert!(calls[..3].iter().all(|(_, _, bearer)| bearer.is_none()));
        assert!(
            calls[3..]
                .iter()
                .all(|(_, _, bearer)| bearer.as_deref() == Some("synthetic-minecraft-secret"))
        );
    }
    #[test]
    fn pending_slowdown_expiration_and_refresh_errors_are_sanitized() {
        let http = Fixture::new(vec![
            (
                Endpoint::MicrosoftToken,
                400,
                json!({"error":"authorization_pending"}),
            ),
            (Endpoint::MicrosoftToken, 400, json!({"error":"slow_down"})),
            (
                Endpoint::MicrosoftToken,
                400,
                json!({"error":"expired_token","error_description":"synthetic-secret-not-for-errors"}),
            ),
        ]);
        assert!(matches!(
            poll(&http, "synthetic-device"),
            Ok(TokenPoll::Pending)
        ));
        assert!(matches!(
            poll(&http, "synthetic-device"),
            Ok(TokenPoll::SlowDown)
        ));
        let error = match poll(&http, "synthetic-device") {
            Err(error) => error,
            _ => panic!("expected expiration"),
        };
        assert_eq!(error, AuthError::Expired);
        assert!(!error.to_string().contains("synthetic-secret"));
        let http = Fixture::new(vec![(
            Endpoint::MicrosoftToken,
            400,
            json!({"error":"invalid_grant","refresh_token":"synthetic-secret"}),
        )]);
        assert!(matches!(
            refresh(&http, "synthetic-refresh"),
            Err(AuthError::ReauthenticationRequired)
        ));
        let http = Fixture::new(vec![(Endpoint::MicrosoftToken, 503, json!("unavailable"))]);
        assert!(matches!(
            poll(&http, "synthetic-device"),
            Err(AuthError::Service { status: 503, .. })
        ));
    }
    #[test]
    fn denied_entitlement_family_restrictions_and_application_blocks_are_distinct() {
        let mut replies = Fixture::chain();
        replies[3].2 = json!({"items":[]});
        let http = Fixture::new(replies);
        assert!(matches!(
            exchange(&http, "synthetic-ms"),
            Err(AuthError::NoEntitlement)
        ));
        assert_eq!(http.calls.lock().unwrap().len(), 4);
        let mut replies = Fixture::chain();
        replies[1].1 = 401;
        replies[1].2 = json!({"XErr":2148916238u64,"Message":"synthetic-secret"});
        assert!(matches!(
            exchange(&Fixture::new(replies), "synthetic-ms"),
            Err(AuthError::XboxFamilyRestricted)
        ));
        let mut replies = Fixture::chain();
        replies[2].1 = 403;
        assert!(matches!(
            exchange(&Fixture::new(replies), "synthetic-ms"),
            Err(AuthError::ApplicationBlocked)
        ));
    }
    #[test]
    fn device_login_url_scope_and_malformed_responses_do_not_escape_the_fixed_contract() {
        let response = json!({"device_code":"synthetic-device-secret","user_code":"ABCD-EFGH","verification_uri":"https://microsoft.com/devicelogin","expires_in":900,"interval":5});
        let http = Fixture::new(vec![(Endpoint::Device, 200, response.clone())]);
        let code = device(&http).unwrap();
        assert_eq!(code.user_code, "ABCD-EFGH");
        let calls = http.calls.lock().unwrap();
        let fields: std::collections::HashMap<_, _> =
            url::form_urlencoded::parse(calls[0].1.as_bytes())
                .into_owned()
                .collect();
        assert_eq!(fields["client_id"], MICROSOFT_CLIENT_ID);
        assert_eq!(fields["scope"], SCOPE);
        drop(calls);
        let mut foreign = response;
        foreign["verification_uri"] = "https://example.invalid/collect".into();
        assert!(matches!(
            device(&Fixture::new(vec![(Endpoint::Device, 200, foreign)])),
            Err(AuthError::InvalidResponse)
        ));
        let error = successful::<serde_json::Value>(
            Response {
                status: 302,
                body: b"synthetic-token".to_vec(),
            },
            Endpoint::MinecraftToken,
        )
        .unwrap_err();
        assert!(!error.to_string().contains("synthetic-token"));
        assert!(decode::<MinecraftToken>(b"{invalid synthetic-token}").is_err());
    }
}
