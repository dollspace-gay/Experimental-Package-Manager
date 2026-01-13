//! Arch Linux to Rookpkg converter
//!
//! Fetches PKGBUILDs from Arch Linux GitLab and converts them to .rook format.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};

use super::pkgbuild::Pkgbuild;

/// Arch Linux package name to Rookery package name mapping
/// Some packages have different names between Arch and Rookery
fn package_name_map() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();

    // Kernel packages (excluded from conversion)
    map.insert("linux", "_SKIP_");
    map.insert("linux-headers", "_SKIP_");
    map.insert("linux-lts", "_SKIP_");
    map.insert("linux-lts-headers", "_SKIP_");
    map.insert("linux-zen", "_SKIP_");
    map.insert("linux-hardened", "_SKIP_");

    // GCC (excluded - using our own)
    map.insert("gcc", "_SKIP_");
    map.insert("gcc-libs", "_SKIP_");
    map.insert("gcc-fortran", "_SKIP_");
    map.insert("gcc-ada", "_SKIP_");
    map.insert("lib32-gcc-libs", "_SKIP_");

    // Glibc (excluded - using our own)
    map.insert("glibc", "_SKIP_");
    map.insert("lib32-glibc", "_SKIP_");

    // Common name differences
    map.insert("python", "python3");
    map.insert("python2", "_SKIP_"); // Python 2 is EOL
    map.insert("jdk-openjdk", "openjdk");
    map.insert("jre-openjdk", "openjdk-jre");
    map.insert("jdk11-openjdk", "openjdk11");
    map.insert("jre11-openjdk", "openjdk11-jre");
    map.insert("jdk17-openjdk", "openjdk17");
    map.insert("jre17-openjdk", "openjdk17-jre");
    map.insert("jdk21-openjdk", "openjdk21");
    map.insert("jre21-openjdk", "openjdk21-jre");

    // Lib32 packages (skip - Rookery is 64-bit only for now)
    // We'll filter these dynamically

    // Qt naming
    map.insert("qt5-base", "qt5");
    map.insert("qt6-base", "qt6");

    // KDE/Plasma naming (KF5 -> kf5, KF6 -> kf6)
    // These are handled dynamically

    // Multimedia
    map.insert("ffmpeg", "ffmpeg");
    map.insert("gst-plugins-base", "gstreamer-plugins-base");
    map.insert("gst-plugins-good", "gstreamer-plugins-good");
    map.insert("gst-plugins-bad", "gstreamer-plugins-bad");
    map.insert("gst-plugins-ugly", "gstreamer-plugins-ugly");

    map
}

/// Packages to always skip during conversion
fn skip_packages() -> Vec<&'static str> {
    vec![
        // Kernels
        "linux",
        "linux-headers",
        "linux-lts",
        "linux-lts-headers",
        "linux-zen",
        "linux-zen-headers",
        "linux-hardened",
        "linux-hardened-headers",
        // Toolchain
        "gcc",
        "gcc-libs",
        "glibc",
        // Lib32 (64-bit only)
        // Arch-specific
        "archlinux-keyring",
        "archlinux-mirrorlist",
        "archinstall",
        "pacman",
        "pacman-mirrorlist",
        "mkinitcpio",
        "mkinitcpio-busybox",
        // We use dracut instead
        "dracut", // We have our own config
    ]
}

/// Arch Linux package converter
pub struct ArchConverter {
    /// HTTP client for fetching PKGBUILDs
    client: reqwest::blocking::Client,
    /// Package name mapping
    name_map: HashMap<&'static str, &'static str>,
    /// Packages to skip
    skip_list: Vec<&'static str>,
}

impl ArchConverter {
    /// Create a new Arch converter
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("rookpkg/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            name_map: package_name_map(),
            skip_list: skip_packages(),
        })
    }

    /// Check if a package should be skipped
    pub fn should_skip(&self, pkg_name: &str) -> bool {
        // Check explicit skip list
        if self.skip_list.contains(&pkg_name) {
            return true;
        }

        // Skip lib32 packages
        if pkg_name.starts_with("lib32-") {
            return true;
        }

        // Check if mapped to _SKIP_
        if let Some(&mapped) = self.name_map.get(pkg_name) {
            if mapped == "_SKIP_" {
                return true;
            }
        }

        false
    }

    /// Map Arch package name to Rookery package name
    pub fn map_package_name(&self, arch_name: &str) -> String {
        // Check explicit mapping
        if let Some(&mapped) = self.name_map.get(arch_name) {
            if mapped != "_SKIP_" {
                return mapped.to_string();
            }
        }

        // Dynamic transformations could go here
        // For now, keep names as-is
        arch_name.to_string()
    }

    /// Map a dependency name from Arch to Rookery
    pub fn map_dependency(&self, dep: &str) -> Option<String> {
        // Parse dependency: name>=version or name=version or name<version or just name
        let (name, version_constraint) = parse_dependency(dep);

        // Check if this dependency should be skipped
        if self.should_skip(&name) {
            return None;
        }

        // Map the name
        let mapped_name = self.map_package_name(&name);

        // Reconstruct with version if present
        if let Some(constraint) = version_constraint {
            Some(format!("{} {}", mapped_name, constraint))
        } else {
            Some(mapped_name)
        }
    }

    /// Fetch a PKGBUILD from Arch Linux GitLab
    pub fn fetch_pkgbuild(&self, pkg_name: &str) -> Result<String> {
        let url = format!(
            "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/main/PKGBUILD",
            pkg_name
        );

        tracing::debug!("Fetching PKGBUILD from: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .context("Failed to fetch PKGBUILD")?;

        if !response.status().is_success() {
            bail!(
                "Failed to fetch PKGBUILD for '{}': HTTP {}",
                pkg_name,
                response.status()
            );
        }

        response.text().context("Failed to read PKGBUILD content")
    }

    /// Fetch list of all official Arch packages
    pub fn fetch_package_list(&self) -> Result<Vec<ArchPackageInfo>> {
        // Use the Arch Linux packages JSON API with pagination
        // Each page returns up to 250 packages
        let mut packages = Vec::new();

        for repo in &["Core", "Extra"] {
            let mut page = 1;

            loop {
                let url = format!(
                    "https://archlinux.org/packages/search/json/?repo={}&arch=x86_64&page={}",
                    repo, page
                );

                if page == 1 {
                    tracing::info!("Fetching package list from {} repository...", repo);
                } else {
                    tracing::debug!("Fetching {} page {}...", repo, page);
                }

                let response = self
                    .client
                    .get(&url)
                    .send()
                    .context("Failed to fetch package list")?;

                if !response.status().is_success() {
                    tracing::warn!("HTTP {} for {}", response.status(), url);
                    break;
                }

                let text = response.text().context("Failed to read response")?;
                tracing::debug!("Response length: {} bytes", text.len());

                let search_result: ArchSearchResult = match serde_json::from_str(&text) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!("JSON parse error: {}", e);
                        tracing::debug!("First 500 chars: {}", &text[..text.len().min(500)]);
                        break;
                    }
                };

                tracing::debug!("Parsed {} results from page {}", search_result.results.len(), page);

                if search_result.results.is_empty() {
                    // No more results
                    break;
                }

                let count = search_result.results.len();
                packages.extend(search_result.results);

                // If we got fewer than the limit (250), we're done with this repo
                if count < 250 {
                    break;
                }

                page += 1;

                // Small delay to be nice to the API
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            tracing::info!("Found {} packages in {} repository",
                packages.iter().filter(|p| p.repo.eq_ignore_ascii_case(repo)).count(), repo);
        }

        Ok(packages)
    }

    /// Convert an Arch PKGBUILD to .rook format
    pub fn convert(&self, pkg_name: &str) -> Result<String> {
        if self.should_skip(pkg_name) {
            bail!("Package '{}' is in the skip list", pkg_name);
        }

        let pkgbuild_content = self.fetch_pkgbuild(pkg_name)?;
        let pkgbuild = Pkgbuild::parse(&pkgbuild_content)?;

        self.pkgbuild_to_rook(&pkgbuild)
    }

    /// Convert a parsed PKGBUILD to .rook format
    pub fn pkgbuild_to_rook(&self, pkg: &Pkgbuild) -> Result<String> {
        let mut rook = String::new();

        // [package] section
        rook.push_str("[package]\n");
        rook.push_str(&format!(
            "name = \"{}\"\n",
            self.map_package_name(&pkg.pkgname)
        ));
        rook.push_str(&format!("version = \"{}\"\n", pkg.version()));
        rook.push_str(&format!("release = {}\n", pkg.release()));
        rook.push_str(&format!(
            "summary = \"{}\"\n",
            escape_toml_string(&pkg.pkgdesc)
        ));
        rook.push_str(&format!(
            "description = \"\"\"\n{}\n\"\"\"\n",
            escape_toml_string(&pkg.pkgdesc)
        ));

        if !pkg.url.is_empty() {
            rook.push_str(&format!("homepage = \"{}\"\n", pkg.url));
        }

        if !pkg.license.is_empty() {
            rook.push_str(&format!(
                "license = \"{}\"\n",
                pkg.license.join(" AND ")
            ));
        }

        rook.push_str("maintainer = \"Converted from Arch Linux <converted@rookeryos.dev>\"\n");
        rook.push_str("arch = \"x86_64\"\n");
        rook.push('\n');

        // [sources] section
        if !pkg.source.is_empty() {
            rook.push_str("[sources]\n");
            let checksums = pkg.checksums();

            for (i, source) in pkg.source.iter().enumerate() {
                let expanded_url = pkg.expand_variables(source);
                let checksum = checksums.get(i).cloned().unwrap_or_default();

                // Handle different checksum cases
                if checksum == "SKIP" || checksum.is_empty() {
                    // SKIP means upstream doesn't provide checksum, user must compute it
                    // Use rookpkg checksum --update to fill this in
                    rook.push_str(&format!(
                        "source{} = {{ url = \"{}\", sha256 = \"_NEEDS_CHECKSUM_RUN_rookpkg_checksum_update_\" }}\n",
                        i, expanded_url
                    ));
                } else {
                    rook.push_str(&format!(
                        "source{} = {{ url = \"{}\", sha256 = \"{}\" }}\n",
                        i, expanded_url, checksum
                    ));
                }
            }
            rook.push('\n');
        }

        // [patches] section (empty for now, would need to handle patch sources)
        rook.push_str("[patches]\n\n");

        // [build_depends] section
        if !pkg.makedepends.is_empty() || !pkg.checkdepends.is_empty() {
            rook.push_str("[build_depends]\n");

            for dep in &pkg.makedepends {
                if let Some(mapped) = self.map_dependency(dep) {
                    let (name, version) = parse_dependency(&mapped);
                    if let Some(ver) = version {
                        rook.push_str(&format!("{} = \"{}\"\n", name, ver));
                    } else {
                        rook.push_str(&format!("{} = \">= 0\"\n", name));
                    }
                }
            }

            for dep in &pkg.checkdepends {
                if let Some(mapped) = self.map_dependency(dep) {
                    let (name, version) = parse_dependency(&mapped);
                    if let Some(ver) = version {
                        rook.push_str(&format!("{} = \"{}\"\n", name, ver));
                    } else {
                        rook.push_str(&format!("{} = \">= 0\"\n", name));
                    }
                }
            }
            rook.push('\n');
        }

        // [depends] section
        if !pkg.depends.is_empty() {
            rook.push_str("[depends]\n");

            for dep in &pkg.depends {
                if let Some(mapped) = self.map_dependency(dep) {
                    let (name, version) = parse_dependency(&mapped);
                    if let Some(ver) = version {
                        rook.push_str(&format!("{} = \"{}\"\n", name, ver));
                    } else {
                        rook.push_str(&format!("{} = \">= 0\"\n", name));
                    }
                }
            }
            rook.push('\n');
        }

        // [optional_depends] section
        if !pkg.optdepends.is_empty() {
            rook.push_str("[optional_depends]\n");

            for dep in &pkg.optdepends {
                // optdepends format: "pkg: description"
                let parts: Vec<&str> = dep.splitn(2, ':').collect();
                let dep_name = parts[0].trim();
                let description = parts.get(1).map(|s| s.trim()).unwrap_or("");

                if let Some(mapped) = self.map_dependency(dep_name) {
                    let (name, _) = parse_dependency(&mapped);
                    rook.push_str(&format!(
                        "{} = [\"{}\"]\n",
                        name,
                        escape_toml_string(description)
                    ));
                }
            }
            rook.push('\n');
        }

        // [environment] section
        rook.push_str("[environment]\n\n");

        // [build] section
        rook.push_str("[build]\n");

        // prepare phase
        if let Some(ref prepare) = pkg.prepare_func {
            let converted = pkg.expand_variables(prepare);
            rook.push_str(&format!("prep = \"\"\"\n{}\n\"\"\"\n\n", converted));
        } else {
            rook.push_str("prep = \"\"\"\n\"\"\"\n\n");
        }

        // configure phase (often part of build in Arch)
        rook.push_str("configure = \"\"\"\n\"\"\"\n\n");

        // build phase
        if let Some(ref build) = pkg.build_func {
            let converted = pkg.expand_variables(build);
            rook.push_str(&format!("build = \"\"\"\n{}\n\"\"\"\n\n", converted));
        } else {
            rook.push_str("build = \"\"\"\n\"\"\"\n\n");
        }

        // check phase
        if let Some(ref check) = pkg.check_func {
            let converted = pkg.expand_variables(check);
            rook.push_str(&format!("check = \"\"\"\n{}\n\"\"\"\n\n", converted));
        } else {
            rook.push_str("check = \"\"\"\n\"\"\"\n\n");
        }

        // install phase
        if let Some(ref package) = pkg.package_func {
            let converted = pkg.expand_variables(package);
            rook.push_str(&format!("install = \"\"\"\n{}\n\"\"\"\n\n", converted));
        } else {
            rook.push_str("install = \"\"\"\n\"\"\"\n\n");
        }

        // [files] section
        rook.push_str("[files]\n\n");

        // [config_files] section
        if !pkg.backup.is_empty() {
            rook.push_str("[config_files]\n");
            for file in &pkg.backup {
                rook.push_str(&format!("\"{}\" = {{}}\n", file));
            }
            rook.push('\n');
        } else {
            rook.push_str("[config_files]\n\n");
        }

        // [scripts] section
        rook.push_str("[scripts]\n\n");

        // Add review notice
        rook.push_str("# =============================================================================\n");
        rook.push_str("# CONVERTED FROM ARCH LINUX PKGBUILD - REVIEW REQUIRED\n");
        rook.push_str("# =============================================================================\n");
        rook.push_str("# This file was automatically converted and may need manual adjustments:\n");
        rook.push_str("# - Verify source URLs and checksums\n");
        rook.push_str("# - Check dependency names are correct for Rookery\n");
        rook.push_str("# - Review build instructions for Rookery-specific paths\n");
        rook.push_str("# - Add [files] entries to specify what gets packaged\n");
        rook.push_str("# =============================================================================\n");

        Ok(rook)
    }

    /// Convert all packages and save to output directory
    pub fn convert_all(&self, output_dir: &Path) -> Result<ConversionStats> {
        std::fs::create_dir_all(output_dir)
            .context("Failed to create output directory")?;

        let packages = self.fetch_package_list()?;
        let total = packages.len();

        let mut stats = ConversionStats {
            total,
            converted: 0,
            skipped: 0,
            failed: 0,
            failed_packages: Vec::new(),
        };

        tracing::info!("Converting {} packages...", total);

        for (i, pkg_info) in packages.iter().enumerate() {
            let pkg_name = &pkg_info.pkgname;

            if self.should_skip(pkg_name) {
                tracing::debug!("Skipping: {}", pkg_name);
                stats.skipped += 1;
                continue;
            }

            tracing::info!("[{}/{}] Converting: {}", i + 1, total, pkg_name);

            match self.convert(pkg_name) {
                Ok(rook_content) => {
                    let output_path = output_dir.join(format!("{}.rook", pkg_name));
                    if let Err(e) = std::fs::write(&output_path, &rook_content) {
                        tracing::error!("Failed to write {}: {}", output_path.display(), e);
                        stats.failed += 1;
                        stats.failed_packages.push(pkg_name.clone());
                    } else {
                        stats.converted += 1;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to convert {}: {}", pkg_name, e);
                    stats.failed += 1;
                    stats.failed_packages.push(pkg_name.clone());
                }
            }

            // Rate limiting to avoid 429 errors from GitLab (allows ~60 req/min)
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }

        Ok(stats)
    }
}

impl Default for ArchConverter {
    fn default() -> Self {
        Self::new().expect("Failed to create ArchConverter")
    }
}

/// Statistics from batch conversion
#[derive(Debug)]
pub struct ConversionStats {
    pub total: usize,
    pub converted: usize,
    pub skipped: usize,
    pub failed: usize,
    pub failed_packages: Vec<String>,
}

/// Package info from Arch search API
#[derive(Debug, serde::Deserialize)]
pub struct ArchPackageInfo {
    pub pkgname: String,
    #[allow(dead_code)]
    pub pkgver: String,
    #[allow(dead_code)]
    pub pkgrel: String,
    #[allow(dead_code)]
    pub pkgdesc: String,
    pub repo: String,
    #[allow(dead_code)]
    pub arch: String,
}

/// Search result from Arch API
#[derive(Debug, serde::Deserialize)]
struct ArchSearchResult {
    results: Vec<ArchPackageInfo>,
}

/// Parse a dependency string like "pkg>=1.0" into (name, constraint)
fn parse_dependency(dep: &str) -> (String, Option<String>) {
    // Handle operators: >=, <=, >, <, =
    let operators = [">=", "<=", ">", "<", "="];

    for op in operators {
        if let Some(pos) = dep.find(op) {
            let name = dep[..pos].trim().to_string();
            let version = dep[pos..].trim().to_string();
            return (name, Some(version));
        }
    }

    (dep.trim().to_string(), None)
}

/// Escape a string for TOML
fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dependency() {
        assert_eq!(
            parse_dependency("glibc>=2.38"),
            ("glibc".to_string(), Some(">=2.38".to_string()))
        );
        assert_eq!(
            parse_dependency("openssl"),
            ("openssl".to_string(), None)
        );
        assert_eq!(
            parse_dependency("qt6-base=6.7.0"),
            ("qt6-base".to_string(), Some("=6.7.0".to_string()))
        );
    }

    #[test]
    fn test_should_skip() {
        let converter = ArchConverter::new().unwrap();
        assert!(converter.should_skip("linux"));
        assert!(converter.should_skip("gcc"));
        assert!(converter.should_skip("lib32-glibc"));
        assert!(!converter.should_skip("firefox"));
    }

    #[test]
    fn test_map_package_name() {
        let converter = ArchConverter::new().unwrap();
        assert_eq!(converter.map_package_name("python"), "python3");
        assert_eq!(converter.map_package_name("firefox"), "firefox");
    }
}
