use crate::error::{AppError, AppResult};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthSearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub categories: Vec<String>,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub author: String,
    pub project_type: String,
    pub latest_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledContentItem {
    pub filename: String,
    pub kind: String, // "mod" | "plugin"
    pub size_bytes: u64,
    pub sha512: String,
    pub blake3_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrpackIndex {
    #[serde(rename = "formatVersion")]
    pub format_version: u32,
    pub game: String,
    #[serde(rename = "versionId")]
    pub version_id: String,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub files: Vec<MrpackFileEntry>,
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrpackFileEntry {
    pub path: String,
    pub hashes: HashMap<String, String>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    pub downloads: Vec<String>,
    #[serde(rename = "fileSize", default)]
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackProgressEvent {
    pub pack_name: String,
    pub completed_files: usize,
    pub total_files: usize,
    pub current_file: String,
}

/// Search Modrinth REST API (`https://api.modrinth.com/v2/search`) for mods, plugins, or modpacks.
pub async fn search_modrinth(
    query: &str,
    project_type: &str, // "mod" | "plugin" | "modpack"
    mc_version: Option<&str>,
    loader: Option<&str>,
) -> AppResult<Vec<ModrinthSearchHit>> {
    let client = reqwest::Client::builder()
        .user_agent("AetherMC/0.1.0 (https://github.com/aethermc)")
        .build()?;

    let mut facets: Vec<Vec<String>> = Vec::new();
    match project_type {
        "plugin" => {
            facets.push(vec!["project_type:plugin".to_string(), "project_type:mod".to_string()]);
            facets.push(vec![
                "categories:paper".to_string(),
                "categories:spigot".to_string(),
                "categories:bukkit".to_string(),
                "categories:purpur".to_string(),
            ]);
        }
        "modpack" => {
            facets.push(vec!["project_type:modpack".to_string()]);
        }
        _ => {
            facets.push(vec!["project_type:mod".to_string()]);
            if let Some(ldr) = loader {
                if !ldr.is_empty() && ldr != "paper" {
                    facets.push(vec![format!("categories:{}", ldr.to_lowercase())]);
                }
            }
        }
    }

    if let Some(ver) = mc_version {
        if !ver.is_empty() {
            facets.push(vec![format!("versions:{}", ver)]);
        }
    }

    let facets_json = serde_json::to_string(&facets)?;
    let url = reqwest::Url::parse_with_params(
        "https://api.modrinth.com/v2/search",
        &[
            ("query", query),
            ("limit", "24"),
            ("facets", &facets_json),
        ],
    )
    .map_err(|e| AppError::Content(format!("Invalid Modrinth search URL: {}", e)))?;

    let resp: serde_json::Value = client.get(url).send().await?.json().await?;
    let hits_arr = resp["hits"]
        .as_array()
        .ok_or_else(|| AppError::Content("Unexpected Modrinth search response".to_string()))?;

    let mut results = Vec::with_capacity(hits_arr.len());
    for hit in hits_arr {
        results.push(ModrinthSearchHit {
            project_id: hit["project_id"].as_str().unwrap_or_default().to_string(),
            slug: hit["slug"].as_str().unwrap_or_default().to_string(),
            title: hit["title"].as_str().unwrap_or_default().to_string(),
            description: hit["description"].as_str().unwrap_or_default().to_string(),
            categories: hit["categories"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            downloads: hit["downloads"].as_u64().unwrap_or(0),
            icon_url: hit["icon_url"].as_str().map(|s| s.to_string()),
            author: hit["author"].as_str().unwrap_or("Unknown").to_string(),
            project_type: hit["project_type"]
                .as_str()
                .unwrap_or(project_type)
                .to_string(),
            latest_version: hit["latest_version"].as_str().map(|s| s.to_string()),
        });
    }

    Ok(results)
}

#[derive(Debug, Clone, Serialize)]
pub struct ModrinthGalleryImage {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub featured: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModrinthVersionInfo {
    pub name: String,
    pub version_number: String,
    /// `release` | `beta` | `alpha`
    pub version_type: String,
    pub date_published: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub downloads: u64,
}

/// Everything the details window of a mod / plugin / modpack shows.
#[derive(Debug, Clone, Serialize)]
pub struct ModrinthProjectDetails {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    /// The long description (Markdown, may contain some HTML).
    pub body: String,
    pub project_type: String,
    pub categories: Vec<String>,
    pub downloads: u64,
    pub followers: u64,
    pub icon_url: Option<String>,
    pub updated: String,
    pub license: Option<String>,
    /// `required` | `optional` | `unsupported` | `unknown`
    pub client_side: String,
    pub server_side: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub gallery: Vec<ModrinthGalleryImage>,
    pub source_url: Option<String>,
    pub issues_url: Option<String>,
    pub wiki_url: Option<String>,
    pub discord_url: Option<String>,
    /// The project's page on modrinth.com.
    pub page_url: String,
    /// Newest first, at most [`MAX_DETAIL_VERSIONS`].
    pub versions: Vec<ModrinthVersionInfo>,
}

const MAX_DETAIL_VERSIONS: usize = 12;

fn json_strings(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// Empty strings from the API count as "not set".
fn json_opt_string(v: &serde_json::Value) -> Option<String> {
    v.as_str().filter(|s| !s.trim().is_empty()).map(str::to_string)
}

/// Load a project's full page data from Modrinth: description, gallery, links, versions.
pub async fn get_modrinth_project(project_id_or_slug: &str) -> AppResult<ModrinthProjectDetails> {
    // The id goes into the request path, so only plain ids / slugs are accepted.
    if project_id_or_slug.is_empty()
        || project_id_or_slug.len() > 64
        || !project_id_or_slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::Content("That isn't a valid Modrinth project.".to_string()));
    }

    let client = reqwest::Client::builder()
        .user_agent("AetherMC/0.1.0 (https://github.com/aethermc)")
        .build()?;
    let project_url = format!("https://api.modrinth.com/v2/project/{}", project_id_or_slug);
    let versions_url = format!("{}/version", project_url);

    let (project, versions) = tokio::join!(
        async { client.get(&project_url).send().await?.error_for_status()?.json::<serde_json::Value>().await },
        async { client.get(&versions_url).send().await?.error_for_status()?.json::<serde_json::Value>().await },
    );
    let p = project?;
    // The version list is a nice-to-have; the page still opens without it.
    let versions = versions.unwrap_or(serde_json::Value::Null);

    let project_type = p["project_type"].as_str().unwrap_or("mod").to_string();
    let slug = p["slug"].as_str().unwrap_or(project_id_or_slug).to_string();

    let mut categories = json_strings(&p["categories"]);
    for extra in json_strings(&p["additional_categories"]) {
        if !categories.contains(&extra) {
            categories.push(extra);
        }
    }

    let gallery = p["gallery"]
        .as_array()
        .map(|a| {
            let mut images: Vec<(i64, ModrinthGalleryImage)> = a
                .iter()
                .filter_map(|g| {
                    Some((
                        g["ordering"].as_i64().unwrap_or(0),
                        ModrinthGalleryImage {
                            url: json_opt_string(&g["url"])?,
                            title: json_opt_string(&g["title"]),
                            description: json_opt_string(&g["description"]),
                            featured: g["featured"].as_bool().unwrap_or(false),
                        },
                    ))
                })
                .collect();
            images.sort_by_key(|(order, _)| *order);
            images.into_iter().map(|(_, img)| img).collect()
        })
        .unwrap_or_default();

    let versions = versions
        .as_array()
        .map(|a| {
            a.iter()
                .take(MAX_DETAIL_VERSIONS)
                .map(|v| ModrinthVersionInfo {
                    name: v["name"].as_str().unwrap_or_default().to_string(),
                    version_number: v["version_number"].as_str().unwrap_or_default().to_string(),
                    version_type: v["version_type"].as_str().unwrap_or("release").to_string(),
                    date_published: v["date_published"].as_str().unwrap_or_default().to_string(),
                    game_versions: json_strings(&v["game_versions"]),
                    loaders: json_strings(&v["loaders"]),
                    downloads: v["downloads"].as_u64().unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(ModrinthProjectDetails {
        project_id: p["id"].as_str().unwrap_or_default().to_string(),
        title: p["title"].as_str().unwrap_or_default().to_string(),
        description: p["description"].as_str().unwrap_or_default().to_string(),
        body: p["body"].as_str().unwrap_or_default().to_string(),
        page_url: format!("https://modrinth.com/{}/{}", project_type, slug),
        project_type,
        slug,
        categories,
        downloads: p["downloads"].as_u64().unwrap_or(0),
        followers: p["followers"].as_u64().unwrap_or(0),
        icon_url: json_opt_string(&p["icon_url"]),
        updated: p["updated"].as_str().unwrap_or_default().to_string(),
        license: json_opt_string(&p["license"]["name"]).or_else(|| json_opt_string(&p["license"]["id"])),
        client_side: p["client_side"].as_str().unwrap_or("unknown").to_string(),
        server_side: p["server_side"].as_str().unwrap_or("unknown").to_string(),
        game_versions: json_strings(&p["game_versions"]),
        loaders: json_strings(&p["loaders"]),
        gallery,
        source_url: json_opt_string(&p["source_url"]),
        issues_url: json_opt_string(&p["issues_url"]),
        wiki_url: json_opt_string(&p["wiki_url"]),
        discord_url: json_opt_string(&p["discord_url"]),
        versions,
    })
}

/// Install a Modrinth project (mod, plugin, or `.mrpack` modpack) with SHA-512 verification.
pub async fn install_modrinth_project(
    server_dir: &Path,
    project_id_or_slug: &str,
    mc_version: &str,
    loader: &str,
    project_type: &str,
) -> AppResult<String> {
    let client = reqwest::Client::builder()
        .user_agent("AetherMC/0.1.0 (https://github.com/aethermc)")
        .build()?;

    let versions_url = format!(
        "https://api.modrinth.com/v2/project/{}/version",
        project_id_or_slug
    );
    let versions: Vec<serde_json::Value> = client.get(&versions_url).send().await?.json().await?;

    if versions.is_empty() {
        return Err(AppError::Content(format!(
            "No versions found on Modrinth for {}",
            project_id_or_slug
        )));
    }

    // Filter best matching version by MC version and loader, falling back to latest release
    let selected = versions
        .iter()
        .find(|v| {
            let ver_match = v["game_versions"]
                .as_array()
                .map(|arr| arr.iter().any(|gv| gv.as_str() == Some(mc_version)))
                .unwrap_or(false);
            let loader_match = if project_type == "plugin" {
                v["loaders"]
                    .as_array()
                    .map(|arr| {
                        arr.iter().any(|l| {
                            matches!(
                                l.as_str(),
                                Some("paper") | Some("spigot") | Some("bukkit") | Some("purpur")
                            )
                        })
                    })
                    .unwrap_or(true)
            } else {
                v["loaders"]
                    .as_array()
                    .map(|arr| arr.iter().any(|l| l.as_str() == Some(loader)))
                    .unwrap_or(true)
            };
            ver_match && loader_match
        })
        .or_else(|| {
            versions.iter().find(|v| {
                v["game_versions"]
                    .as_array()
                    .map(|arr| arr.iter().any(|gv| gv.as_str() == Some(mc_version)))
                    .unwrap_or(false)
            })
        })
        .or_else(|| versions.first())
        .ok_or_else(|| AppError::Content("No compatible version found".to_string()))?;

    let files = selected["files"]
        .as_array()
        .ok_or_else(|| AppError::Content("Version contains no downloadable files".to_string()))?;
    let primary_file = files
        .iter()
        .find(|f| f["primary"].as_bool() == Some(true))
        .or_else(|| files.first())
        .ok_or_else(|| AppError::Content("Missing primary file in Modrinth version".to_string()))?;

    let filename = primary_file["filename"]
        .as_str()
        .ok_or_else(|| AppError::Content("Missing filename".to_string()))?;
    let download_url = primary_file["url"]
        .as_str()
        .ok_or_else(|| AppError::Content("Missing download URL".to_string()))?;
    let expected_sha512 = primary_file["hashes"]["sha512"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if filename.ends_with(".mrpack") || project_type == "modpack" {
        let temp_pack = server_dir.join(filename);
        download_file_sha512(&client, download_url, &temp_pack, Some(&expected_sha512)).await?;
        install_mrpack_archive(&temp_pack, server_dir, None).await?;
        let _ = fs::remove_file(&temp_pack);
        return Ok(filename.to_string());
    }

    let target_subdir = if project_type == "plugin" || loader == "paper" {
        server_dir.join("plugins")
    } else {
        server_dir.join("mods")
    };
    tokio::fs::create_dir_all(&target_subdir).await?;

    let dest_path = target_subdir.join(filename);
    download_file_sha512(&client, download_url, &dest_path, Some(&expected_sha512)).await?;

    Ok(filename.to_string())
}

/// Parse `modrinth.index.json` inside a `.mrpack` archive and download all files concurrently
/// using a bounded `tokio::sync::Semaphore` (12 parallel streams) with SHA-512 verification.
pub async fn install_mrpack_archive(
    mrpack_path: &Path,
    target_dir: &Path,
    app_handle: Option<&AppHandle>,
) -> AppResult<MrpackIndex> {
    let mrpack_buf = mrpack_path.to_path_buf();
    let target_buf = target_dir.to_path_buf();

    // 1. Extract modrinth.index.json and overrides/ inside spawn_blocking
    let index: MrpackIndex = tokio::task::spawn_blocking(move || -> AppResult<MrpackIndex> {
        let file = fs::File::open(&mrpack_buf)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let mut index_content = String::new();
        {
            let mut idx_file = archive.by_name("modrinth.index.json").map_err(|_| {
                AppError::Content("Archive is missing modrinth.index.json".to_string())
            })?;
            idx_file.read_to_string(&mut index_content)?;
        }
        let parsed_index: MrpackIndex = serde_json::from_str(&index_content)?;

        // Extract `overrides/` and `server-overrides/` into target_dir
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let Some(enclosed) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
                continue;
            };
            let rel_opt = enclosed
                .strip_prefix("server-overrides")
                .ok()
                .or_else(|| enclosed.strip_prefix("overrides").ok());

            if let Some(rel) = rel_opt {
                if rel.as_os_str().is_empty() {
                    continue;
                }
                let out_path = target_buf.join(rel);
                if entry.is_dir() {
                    fs::create_dir_all(&out_path)?;
                } else {
                    if let Some(parent) = out_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let mut out_file = fs::File::create(&out_path)?;
                    std::io::copy(&mut entry, &mut out_file)?;
                }
            }
        }

        Ok(parsed_index)
    })
    .await
    .map_err(|e| AppError::Content(format!("Join error parsing .mrpack: {}", e)))??;

    // 2. Download all entries in index.files concurrently with bounded Semaphore(12) and SHA-512 check
    let semaphore = Arc::new(Semaphore::new(12));
    let client = Arc::new(
        reqwest::Client::builder()
            .user_agent("AetherMC/0.1.0 (https://github.com/aethermc)")
            .build()?,
    );

    let total_files = index.files.len();
    let pack_name = index.name.clone();
    let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut handles = Vec::with_capacity(total_files);

    for entry in index.files.clone() {
        // Skip client-only mods when installing on server if env.server == "unsupported"
        if let Some(ref env) = entry.env {
            if env.get("server").map(|s| s.as_str()) == Some("unsupported") {
                continue;
            }
        }

        let Some(download_url) = entry.downloads.first().cloned() else {
            continue;
        };

        // Guard against path traversal in entry.path
        let rel_path = Path::new(&entry.path);
        if rel_path.is_absolute()
            || rel_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            continue;
        }

        let dest_path = target_dir.join(rel_path);
        let expected_sha512 = entry.hashes.get("sha512").cloned();
        let sem_clone = semaphore.clone();
        let client_clone = client.clone();
        let completed_clone = completed.clone();
        let pack_name_clone = pack_name.clone();
        let app_opt = app_handle.cloned();
        let display_name = entry.path.clone();

        let task = tokio::spawn(async move {
            let _permit = sem_clone
                .acquire()
                .await
                .map_err(|e| AppError::Content(e.to_string()))?;

            download_file_sha512(
                &client_clone,
                &download_url,
                &dest_path,
                expected_sha512.as_deref(),
            )
            .await?;

            let done = completed_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if let Some(app) = app_opt {
                let _ = app.emit(
                    "content:modpack-progress",
                    ModpackProgressEvent {
                        pack_name: pack_name_clone,
                        completed_files: done,
                        total_files,
                        current_file: display_name,
                    },
                );
            }
            Ok::<(), AppError>(())
        });

        handles.push(task);
    }

    for handle in handles {
        handle
            .await
            .map_err(|e| AppError::Content(format!("Download task panicked: {}", e)))??;
    }

    Ok(index)
}

/// Download a file and verify its SHA-512 hash against `expected_sha512`.
pub async fn download_file_sha512(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha512: Option<&str>,
) -> AppResult<()> {
    if let Some(expected) = expected_sha512 {
        if !expected.is_empty() && dest.exists() {
            if let Ok(existing_hash) = compute_file_sha512(dest).await {
                if existing_hash.eq_ignore_ascii_case(expected) {
                    return Ok(());
                }
            }
        }
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::Network(format!(
            "HTTP {} while downloading {}",
            resp.status(),
            url
        )));
    }

    let mut file = fs::File::create(dest)?;
    let mut hasher = Sha512::new();
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        hasher.update(&bytes);
        file.write_all(&bytes)?;
    }
    file.flush()?;

    if let Some(expected) = expected_sha512 {
        if !expected.is_empty() {
            let actual = hex::encode(hasher.finalize());
            if !actual.eq_ignore_ascii_case(expected) {
                let _ = fs::remove_file(dest);
                return Err(AppError::Content(format!(
                    "SHA-512 mismatch for {}: expected {}, got {}",
                    dest.display(),
                    expected,
                    actual
                )));
            }
        }
    }

    Ok(())
}

pub async fn compute_file_sha512(path: &Path) -> AppResult<String> {
    let path_buf = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> AppResult<String> {
        let mut file = fs::File::open(&path_buf)?;
        let mut hasher = Sha512::new();
        let mut buf = [0u8; 65536];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hex::encode(hasher.finalize()))
    })
    .await
    .map_err(|e| AppError::Io(e.to_string()))?
}

/// List all installed mods and plugins in `server/mods` and `server/plugins`.
pub async fn list_installed_content(server_dir: &Path) -> AppResult<Vec<InstalledContentItem>> {
    let dir_buf = server_dir.to_path_buf();
    tokio::task::spawn_blocking(move || -> AppResult<Vec<InstalledContentItem>> {
        let mut items = Vec::new();
        for (folder, kind) in [("mods", "mod"), ("plugins", "plugin")] {
            let dir = dir_buf.join(folder);
            if !dir.exists() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && path.extension().and_then(|e| e.to_str()) == Some("jar")
                    {
                        let meta = entry.metadata()?;
                        let bytes = fs::read(&path)?;
                        let mut sha = Sha512::new();
                        sha.update(&bytes);
                        let sha512 = hex::encode(sha.finalize());
                        let blake3_hash = blake3::hash(&bytes).to_hex().to_string();

                        items.push(InstalledContentItem {
                            filename: entry.file_name().to_string_lossy().to_string(),
                            kind: kind.to_string(),
                            size_bytes: meta.len(),
                            sha512,
                            blake3_hash,
                        });
                    }
                }
            }
        }
        Ok(items)
    })
    .await
    .map_err(|e| AppError::Io(e.to_string()))?
}

pub fn remove_installed_content(server_dir: &Path, filename: &str, kind: &str) -> AppResult<()> {
    let safe_name = Path::new(filename)
        .file_name()
        .ok_or_else(|| AppError::Content("Invalid filename".to_string()))?;
    let folder = if kind == "plugin" { "plugins" } else { "mods" };
    let path = server_dir.join(folder).join(safe_name);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}
