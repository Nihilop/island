//! Client GitHub minimal : *device flow* (CLI, app), échange d'un code OAuth web (panneau
//! d'administration), identité de l'utilisateur, recherche d'un compte par son nom. Le
//! jeton GitHub ne sert qu'à lire l'identité, puis il est jeté : jamais stocké.
//!
//! Les URL de base sont configurables pour les tests (faux GitHub local).

use serde::Deserialize;

#[derive(Clone)]
pub struct GitHub {
    http: reqwest::Client,
    pub client_id: String,
    /// Requis seulement pour l'OAuth web (le device flow s'en passe).
    pub client_secret: Option<String>,
    /// `https://github.com`
    pub web: String,
    /// `https://api.github.com`
    pub api: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Identité GitHub (ce qui nous intéresse).
#[derive(Deserialize, Debug, Clone)]
pub struct Identity {
    pub id: i64,
    pub login: String,
    pub avatar_url: Option<String>,
}

/// Réponse du jeton : prêt, ou encore en attente de l'utilisateur.
pub enum Poll {
    Token(String),
    Pending,
    /// Trop rapide : le client doit espacer ses appels.
    SlowDown,
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: Option<String>,
    error: Option<String>,
}

impl GitHub {
    pub fn new(client_id: String, client_secret: Option<String>, web: String, api: String) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("island-store")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("client HTTP");
        Self { http, client_id, client_secret, web: web.trim_end_matches('/').into(), api: api.trim_end_matches('/').into() }
    }

    pub fn from_env(client_id: String, client_secret: Option<String>) -> Self {
        Self::new(client_id, client_secret, "https://github.com".into(), "https://api.github.com".into())
    }

    /// Début du device flow : code à afficher à l'utilisateur.
    pub async fn device_code(&self) -> Result<DeviceCode, String> {
        self.http
            .post(format!("{}/login/device/code", self.web))
            .header("Accept", "application/json")
            .form(&[("client_id", self.client_id.as_str()), ("scope", "read:user")])
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())
    }

    async fn token(&self, form: &[(&str, &str)]) -> Result<TokenReply, String> {
        self.http
            .post(format!("{}/login/oauth/access_token", self.web))
            .header("Accept", "application/json")
            .form(form)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())
    }

    /// Device flow : l'utilisateur a-t-il validé le code ? Erreurs : `expired`, `denied`.
    pub async fn poll(&self, device_code: &str) -> Result<Poll, String> {
        let r = self
            .token(&[("client_id", &self.client_id), ("device_code", device_code), ("grant_type", "urn:ietf:params:oauth:grant-type:device_code")])
            .await?;
        match (r.access_token, r.error.as_deref()) {
            (Some(t), _) => Ok(Poll::Token(t)),
            (None, Some("authorization_pending")) => Ok(Poll::Pending),
            (None, Some("slow_down")) => Ok(Poll::SlowDown),
            (None, Some("expired_token")) => Err("expired".into()),
            (None, Some("access_denied")) => Err("denied".into()),
            (None, e) => Err(format!("github: {}", e.unwrap_or("?"))),
        }
    }

    /// OAuth web : échange le code du retour contre un jeton.
    pub async fn exchange(&self, code: &str, redirect_uri: &str) -> Result<String, String> {
        let secret = self.client_secret.as_deref().ok_or("GITHUB_CLIENT_SECRET manquant")?;
        let r = self.token(&[("client_id", &self.client_id), ("client_secret", secret), ("code", code), ("redirect_uri", redirect_uri)]).await?;
        r.access_token.ok_or_else(|| format!("github: {}", r.error.unwrap_or_default()))
    }

    /// URL de connexion OAuth web.
    pub fn authorize_url(&self, redirect_uri: &str, state: &str) -> String {
        let enc = |s: &str| s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
        format!("{}/login/oauth/authorize?client_id={}&redirect_uri={}&scope=read:user&state={}", self.web, enc(&self.client_id), enc(redirect_uri), enc(state))
    }

    /// Identité du compte GitHub derrière un jeton.
    pub async fn identity(&self, token: &str) -> Result<Identity, String> {
        self.http
            .get(format!("{}/user", self.api))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())
    }

    /// Compte GitHub par son nom (ajout d'un modérateur) ; None s'il n'existe pas.
    pub async fn lookup(&self, login: &str) -> Result<Option<Identity>, String> {
        let r = self
            .http
            .get(format!("{}/users/{login}", self.api))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if r.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        r.error_for_status().map_err(|e| e.to_string())?.json().await.map(Some).map_err(|e| e.to_string())
    }
}
