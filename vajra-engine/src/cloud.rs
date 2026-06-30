//! Consumer Cloud Link Translation (Google Drive, Dropbox, OneDrive, MEGA).
//! Automatically translates public share links into direct, high-speed download endpoints.

use url::Url;

/// Checks if the URL belongs to a supported consumer cloud provider.
pub fn is_cloud_link(url_str: &str) -> bool {
    let host = match Url::parse(url_str) {
        Ok(u) => match u.host_str() {
            Some(h) => h.to_lowercase(),
            None => return false,
        },
        Err(_) => return false,
    };

    host.contains("drive.google.com")
        || host.contains("dropbox.com")
        || host.contains("onedrive.live.com")
        || host.contains("mega.nz")
}

/// Translates a consumer cloud share link into a direct download URL.
pub async fn translate_cloud_link(url_str: &str) -> anyhow::Result<String> {
    let url = Url::parse(url_str)?;
    let host = url.host_str().ok_or_else(|| anyhow::anyhow!("No host in URL"))?.to_lowercase();

    // 1. Google Drive
    if host.contains("drive.google.com") {
        // Handle format: https://drive.google.com/file/d/{FILE_ID}/view
        // Translate to: https://drive.google.com/uc?export=download&id={FILE_ID}
        if let Some(segments) = url.path_segments() {
            let seg_vec: Vec<&str> = segments.collect();
            if let Some(idx) = seg_vec.iter().position(|&s| s == "d") {
                if idx + 1 < seg_vec.len() {
                    let file_id = seg_vec[idx + 1];
                    return Ok(format!("https://drive.google.com/uc?export=download&id={}", file_id));
                }
            }
        }
        // Handle query parameter id: https://drive.google.com/open?id={FILE_ID}
        for (key, val) in url.query_pairs() {
            if key == "id" {
                return Ok(format!("https://drive.google.com/uc?export=download&id={}", val));
            }
        }
    }

    // 2. Dropbox
    if host.contains("dropbox.com") {
        // Change dl=0 to dl=1, or replace host with dl.dropboxusercontent.com
        let mut new_url = url.clone();
        let mut query_pairs: Vec<(String, String)> = url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
        if let Some(pos) = query_pairs.iter().position(|(k, _)| k == "dl") {
            query_pairs[pos].1 = "1".to_string();
        } else {
            query_pairs.push(("dl".to_string(), "1".to_string()));
        }
        
        let query_str = query_pairs.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<String>>()
            .join("&");
        new_url.set_query(Some(&query_str));
        return Ok(new_url.to_string());
    }

    // 3. OneDrive
    if host.contains("onedrive.live.com") {
        // Replace "redir" with "download" in URL query or path
        let query_pairs: Vec<(String, String)> = url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
        if let Some(pos) = query_pairs.iter().position(|(k, _)| k == "resid") {
            let resid = query_pairs[pos].1.clone();
            let authkey = query_pairs.iter().find(|(k, _)| k == "authkey").map(|(_, v)| v.clone());
            let mut download_url = format!("https://onedrive.live.com/download?resid={}", resid);
            if let Some(key) = authkey {
                download_url.push_str(&format!("&authkey={}", key));
            }
            return Ok(download_url);
        }
    }

    // 4. MEGA.nz (returns the original URL since full encryption requires specialized client headers, but marks it for processing)
    if host.contains("mega.nz") {
        return Ok(url_str.to_string());
    }

    Ok(url_str.to_string())
}
