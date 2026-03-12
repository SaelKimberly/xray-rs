use std::collections::HashMap;

use base64::{
    Engine,
    prelude::{BASE64_STANDARD_NO_PAD, BASE64_URL_SAFE_NO_PAD},
};
use bstr::ByteSlice;
use bytes::Bytes;
use reqwest::Client;
use sha2::Digest;
use url::Url;

/// Download data from url
/// return (SHA256 hash, data (optionally base64 decoded, if possible))
pub async fn download_data(client: Client, url: Url, b64: bool) -> anyhow::Result<(String, Bytes)> {
    let rb = client.get(url.as_str());

    let mut with_auth = false;
    let request = match url.host_str() {
        Some("raw.githubusercontent.com" | "github.com") => {
            if let Ok(token) = std::env::var("GITHUB_TOKEN") {
                with_auth = true;
                rb.bearer_auth(token)
            } else {
                rb
            }
        }
        _ => rb,
    }
    .build()?;

    tracing::info!("Executing {} | {}", request.method(), url.as_str());

    let response = client.execute(request).await?;

    match response.status() {
        status if status.is_success() => {
            tracing::info!(
                "{} [{}] {}",
                status,
                if with_auth { "AUTH" } else { "ANON" },
                response.url()
            );
        }
        _ => {
            tracing::warn!(
                "{} [{}] {}",
                response.status(),
                response.status().canonical_reason().unwrap_or("Unknown"),
                response.url()
            );
            response.error_for_status_ref()?;
        }
    }

    let data = response.bytes().await?;

    let data = if b64 {
        let to_decode = data
            .as_ref()
            .trim_end_with(|b| matches!(b, '=' | ' ' | '\n' | '\r' | '\t'));

        BASE64_STANDARD_NO_PAD
            .decode(to_decode)
            .inspect(|_| tracing::info!("-> standard base64"))
            .or_else(|_| {
                BASE64_URL_SAFE_NO_PAD
                    .decode(to_decode)
                    .inspect(|_| tracing::info!("-> urlsafe base64"))
            })
            .map(Bytes::from)
            .unwrap_or_else(|_| {
                tracing::info!("-> no base64");
                data
            })
    } else {
        tracing::info!("-> no base64");
        data
    };

    let mut hash = sha2::Sha256::default();
    hash.update(data.as_ref());
    let hash_sum = hash.finalize();
    let hash_sum = format!("{:02x}", Bytes::copy_from_slice(&hash_sum.as_slice()[..8]));

    Ok((hash_sum, data))
}

static LIMITER: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(8);

pub async fn download_subs(client: Client, urls: Vec<Url>, b64: bool) -> HashMap<String, Bytes> {
    let mut tasks = tokio::task::JoinSet::new();

    for url in urls {
        let client = client.clone();
        let url = url.clone();
        tasks.spawn(async move {
            let _permit = LIMITER.acquire().await.expect("Can't acquire permit");
            download_data(client, url, b64).await
        });
    }

    let mut subs = HashMap::new();
    while let Some(res) = tasks.join_next().await {
        match res {
            Ok(Ok((hash, data))) => {
                tracing::debug!("Downloaded: [{}] ({})", hash, data.len());
                let data = String::from_utf8_lossy(data.as_ref());
                let data = crate::norm::normalize_subscription_content(data.as_ref());
                subs.entry(hash).or_insert(data.as_bytes().to_vec().into());
            }
            Ok(Err(e)) => tracing::error!("{}", e),
            Err(e) => tracing::error!("{}", e),
        }
    }

    subs
}

#[cfg(test)]
mod tests {
    use std::{io::Write, time::Duration};

    use super::*;
    use url::Url;

    #[tokio::test]
    async fn test_download_data() -> std::io::Result<()> {
        tracing_subscriber::fmt().pretty().init();

        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(10))
            .user_agent("Fetcher")
            .gzip(true)
            .https_only(true)
            .build()
            .expect("Can't build client");

        let urls = [
            "https://github.com/sakha1370/OpenRay/raw/refs/heads/main/output/all_valid_proxies.txt",
            "https://raw.githubusercontent.com/sevcator/5ubscrpt10n/main/protocols/vl.txt",
            "https://raw.githubusercontent.com/yitong2333/proxy-minging/refs/heads/main/v2ray.txt",
            "https://raw.githubusercontent.com/acymz/AutoVPN/refs/heads/main/data/V2.txt",
            "https://raw.githubusercontent.com/miladtahanian/V2RayCFGDumper/refs/heads/main/sub.txt",
            "https://raw.githubusercontent.com/roosterkid/openproxylist/main/V2RAY_RAW.txt",
            "https://github.com/Epodonios/v2ray-configs/raw/main/Splitted-By-Protocol/trojan.txt",
            "https://raw.githubusercontent.com/CidVpn/cid-vpn-config/refs/heads/main/general.txt",
            "https://raw.githubusercontent.com/mohamadfg-dev/telegram-v2ray-configs-collector/refs/heads/main/category/vless.txt",
            "https://raw.githubusercontent.com/mheidari98/.proxy/refs/heads/main/vless", 
            "https://raw.githubusercontent.com/youfoundamin/V2rayCollector/main/mixed_iran.txt", 
            "https://github.com/expressalaki/ExpressVPN/blob/main/configs3.txt", 
            "https://raw.githubusercontent.com/MahsaNetConfigTopic/config/refs/heads/main/xray_final.txt", 
            "https://github.com/LalatinaHub/Mineral/raw/refs/heads/master/result/nodes", 
            "https://github.com/miladtahanian/Config-Collector/raw/refs/heads/main/vless_iran.txt", 
            "https://raw.githubusercontent.com/Pawdroid/Free-servers/refs/heads/main/sub", 
            "https://github.com/MhdiTaheri/V2rayCollector_Py/raw/refs/heads/main/sub/Mix/mix.txt", 
            "https://raw.githubusercontent.com/free18/v2ray/refs/heads/main/v.txt", 
            "https://github.com/MhdiTaheri/V2rayCollector/raw/refs/heads/main/sub/mix", 
            "https://github.com/Argh94/Proxy-List/raw/refs/heads/main/All_Config.txt", 
            "https://raw.githubusercontent.com/shabane/kamaji/master/hub/merged.txt", 
            "https://raw.githubusercontent.com/wuqb2i4f/xray-config-toolkit/main/output/base64/mix-uri", 
            "https://raw.githubusercontent.com/zipvpn/FreeVPNNodes/refs/heads/main/free_v2ray_xray_nodes.txt", 
            "https://raw.githubusercontent.com/STR97/STRUGOV/refs/heads/main/STR.BYPASS#STR.BYPASS%F0%9F%91%BE", 
            "https://raw.githubusercontent.com/V2RayRoot/V2RayConfig/refs/heads/main/Config/vless.txt", 
            "https://raw.githubusercontent.com/igareck/vpn-configs-for-russia/refs/heads/main/WHITE-CIDR-RU-all.txt",
            "https://raw.githubusercontent.com/igareck/vpn-configs-for-russia/refs/heads/main/WHITE-SNI-RU-all.txt",
            "https://raw.githubusercontent.com/zieng2/wl/main/vless.txt",
            "https://raw.githubusercontent.com/zieng2/wl/refs/heads/main/vless_universal.txt",
            "https://raw.githubusercontent.com/zieng2/wl/main/vless_lite.txt",
            "https://raw.githubusercontent.com/EtoNeYaProject/etoneyaproject.github.io/refs/heads/main/2",
            "https://raw.githubusercontent.com/gbwltg/gbwl/refs/heads/main/m3EsPqwmlc",
            "https://bp.wl.free.nf/confs/wl.txt",
            "https://storage.yandexcloud.net/cid-vpn/whitelist.txt"
        ].iter().flat_map(
            |s| Url::parse(s).inspect_err(|e| tracing::warn!(
                "Invalid Url: {s} ({e})"
            ))
        ).collect::<Vec<_>>();

        let subs = download_subs(client, urls, true).await;

        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("full.txt")?;

        let mut file = std::io::BufWriter::new(file);
        for (_hash, data) in subs {
            file.write_all(data.as_ref())?;
        }
        Ok(())
    }
}
