//! PKGBUILD parser for Arch Linux package specifications
//!
//! Parses Arch Linux PKGBUILD files into a structured representation
//! that can be converted to .rook format.

use std::collections::HashMap;

use anyhow::Result;
use regex::Regex;

/// Parsed PKGBUILD structure
#[derive(Debug, Clone, Default)]
pub struct Pkgbuild {
    /// Package name (pkgname)
    pub pkgname: String,
    /// Package version (pkgver)
    pub pkgver: String,
    /// Package release number (pkgrel)
    pub pkgrel: String,
    /// Epoch (optional, prepended to version)
    pub epoch: Option<String>,
    /// Package description (pkgdesc)
    pub pkgdesc: String,
    /// Upstream URL
    pub url: String,
    /// Architectures (arch)
    pub arch: Vec<String>,
    /// Licenses
    pub license: Vec<String>,
    /// Runtime dependencies (depends)
    pub depends: Vec<String>,
    /// Build dependencies (makedepends)
    pub makedepends: Vec<String>,
    /// Check dependencies (checkdepends)
    pub checkdepends: Vec<String>,
    /// Optional dependencies (optdepends)
    pub optdepends: Vec<String>,
    /// Packages this provides (provides)
    pub provides: Vec<String>,
    /// Packages this conflicts with (conflicts)
    pub conflicts: Vec<String>,
    /// Packages this replaces (replaces)
    pub replaces: Vec<String>,
    /// Source URLs
    pub source: Vec<String>,
    /// SHA256 checksums
    pub sha256sums: Vec<String>,
    /// SHA512 checksums (alternative)
    pub sha512sums: Vec<String>,
    /// MD5 checksums (legacy)
    pub md5sums: Vec<String>,
    /// B2 checksums (alternative)
    pub b2sums: Vec<String>,
    /// Package groups
    pub groups: Vec<String>,
    /// Backup files (config files)
    pub backup: Vec<String>,
    /// Installation options
    pub options: Vec<String>,
    /// Install scriptlet
    pub install: Option<String>,
    /// Changelog file
    pub changelog: Option<String>,

    // Functions (shell script bodies)
    /// prepare() function body
    pub prepare_func: Option<String>,
    /// build() function body
    pub build_func: Option<String>,
    /// check() function body
    pub check_func: Option<String>,
    /// package() function body
    pub package_func: Option<String>,
    /// Split package functions (package_pkgname())
    pub package_funcs: HashMap<String, String>,

    /// All raw variables for reference
    pub raw_variables: HashMap<String, String>,
}

impl Pkgbuild {
    /// Parse a PKGBUILD from its content
    pub fn parse(content: &str) -> Result<Self> {
        let mut pkg = Pkgbuild::default();

        // First pass: extract all variable assignments
        pkg.extract_variables(content)?;

        // Second pass: extract function bodies
        pkg.extract_functions(content)?;

        // Populate struct fields from variables
        pkg.populate_fields()?;

        Ok(pkg)
    }

    /// Extract all variable assignments from PKGBUILD
    fn extract_variables(&mut self, content: &str) -> Result<()> {
        // Simple variable: varname=value or varname="value" or varname='value'
        let simple_var_re = Regex::new(r#"^([a-zA-Z_][a-zA-Z0-9_]*)=([^(].*?)$"#)?;

        // Array start: varname=(
        let array_start_re = Regex::new(r#"^([a-zA-Z_][a-zA-Z0-9_]*)=\((.*)$"#)?;

        let mut lines = content.lines().peekable();
        let mut current_var: Option<String> = None;
        let mut current_array: Vec<String> = Vec::new();
        let mut in_array = false;

        while let Some(line) = lines.next() {
            let trimmed = line.trim();

            // Skip comments and empty lines (unless in array)
            if !in_array && (trimmed.is_empty() || trimmed.starts_with('#')) {
                continue;
            }

            // Skip function definitions (handled separately)
            if trimmed.contains("()") && trimmed.contains('{') {
                continue;
            }
            if trimmed.ends_with("() {") || trimmed.ends_with("(){") {
                continue;
            }

            if in_array {
                // Continue collecting array elements
                let elements = self.parse_array_elements(trimmed);
                current_array.extend(elements);

                // Check if array ends
                if trimmed.contains(')') && !trimmed.contains("$(") {
                    // Array complete
                    if let Some(ref var_name) = current_var {
                        self.raw_variables
                            .insert(var_name.clone(), current_array.join("\n"));
                    }
                    current_var = None;
                    current_array.clear();
                    in_array = false;
                }
            } else if let Some(caps) = array_start_re.captures(trimmed) {
                // Array variable
                let var_name = caps.get(1).unwrap().as_str().to_string();
                let rest = caps.get(2).unwrap().as_str();

                current_var = Some(var_name.clone());
                in_array = true;
                current_array.clear();

                // Parse elements on the same line
                let elements = self.parse_array_elements(rest);
                current_array.extend(elements);

                // Check if array ends on same line
                if rest.contains(')') && !rest.contains("$(") {
                    self.raw_variables
                        .insert(var_name, current_array.join("\n"));
                    current_var = None;
                    current_array.clear();
                    in_array = false;
                }
            } else if let Some(caps) = simple_var_re.captures(trimmed) {
                // Simple variable
                let var_name = caps.get(1).unwrap().as_str().to_string();
                let value = caps.get(2).unwrap().as_str();

                // Strip quotes
                let clean_value = self.strip_quotes(value);
                self.raw_variables.insert(var_name, clean_value);
            }
        }

        Ok(())
    }

    /// Parse array elements from a line
    fn parse_array_elements(&self, line: &str) -> Vec<String> {
        let mut elements = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut quote_char = ' ';
        let mut chars = line.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '"' | '\'' if !in_quotes => {
                    in_quotes = true;
                    quote_char = c;
                }
                c if in_quotes && c == quote_char => {
                    in_quotes = false;
                    if !current.is_empty() {
                        elements.push(current.clone());
                        current.clear();
                    }
                }
                ')' if !in_quotes => {
                    if !current.is_empty() {
                        elements.push(current.clone());
                        current.clear();
                    }
                    break;
                }
                ' ' | '\t' if !in_quotes => {
                    if !current.is_empty() {
                        elements.push(current.clone());
                        current.clear();
                    }
                }
                '#' if !in_quotes => {
                    // Comment - stop parsing
                    break;
                }
                '(' if !in_quotes => {
                    // Skip opening paren
                }
                _ => {
                    current.push(c);
                }
            }
        }

        if !current.is_empty() {
            elements.push(current);
        }

        elements
    }

    /// Strip surrounding quotes from a value
    fn strip_quotes(&self, value: &str) -> String {
        let trimmed = value.trim();
        if (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        {
            trimmed[1..trimmed.len() - 1].to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Extract function bodies from PKGBUILD
    fn extract_functions(&mut self, content: &str) -> Result<()> {
        // Match function definitions like: funcname() { or funcname () {
        let func_re = Regex::new(r"^([a-zA-Z_][a-zA-Z0-9_]*)\s*\(\s*\)\s*\{")?;

        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            if let Some(caps) = func_re.captures(line) {
                let func_name = caps.get(1).unwrap().as_str().to_string();
                let (body, end_idx) = self.extract_function_body(&lines, i)?;

                // Store function body
                match func_name.as_str() {
                    "prepare" => self.prepare_func = Some(body),
                    "build" => self.build_func = Some(body),
                    "check" => self.check_func = Some(body),
                    "package" => self.package_func = Some(body),
                    name if name.starts_with("package_") => {
                        let pkg_name = name.strip_prefix("package_").unwrap().to_string();
                        self.package_funcs.insert(pkg_name, body);
                    }
                    _ => {
                        // Other functions - store in raw variables
                        self.raw_variables.insert(format!("func_{}", func_name), body);
                    }
                }

                i = end_idx + 1;
            } else {
                i += 1;
            }
        }

        Ok(())
    }

    /// Extract function body (handles nested braces)
    fn extract_function_body(&self, lines: &[&str], start: usize) -> Result<(String, usize)> {
        let mut body = Vec::new();
        let mut brace_count = 0;
        let mut started = false;

        for (i, line) in lines.iter().enumerate().skip(start) {
            for c in line.chars() {
                if c == '{' {
                    brace_count += 1;
                    started = true;
                } else if c == '}' {
                    brace_count -= 1;
                }
            }

            if started {
                // Skip the first line (function declaration)
                if i > start {
                    body.push(*line);
                }

                if brace_count == 0 {
                    // Remove the closing brace line if it only contains }
                    if let Some(last) = body.last() {
                        if last.trim() == "}" {
                            body.pop();
                        }
                    }
                    return Ok((body.join("\n"), i));
                }
            }
        }

        // If we get here, braces weren't balanced
        Ok((body.join("\n"), lines.len() - 1))
    }

    /// Populate struct fields from extracted variables
    fn populate_fields(&mut self) -> Result<()> {
        // Helper to get string value
        let get_str = |vars: &HashMap<String, String>, key: &str| -> String {
            vars.get(key).cloned().unwrap_or_default()
        };

        // Helper to get array values
        let get_array = |vars: &HashMap<String, String>, key: &str| -> Vec<String> {
            vars.get(key)
                .map(|v| v.lines().map(|s| s.to_string()).collect())
                .unwrap_or_default()
        };

        // Handle split packages: pkgname can be an array, use pkgbase or first name
        let raw_pkgname = get_str(&self.raw_variables, "pkgname");
        if raw_pkgname.contains('\n') {
            // Split package - use pkgbase if available, otherwise first pkgname
            self.pkgname = self.raw_variables
                .get("pkgbase")
                .cloned()
                .unwrap_or_else(|| raw_pkgname.lines().next().unwrap_or_default().to_string());
        } else {
            self.pkgname = raw_pkgname;
        }

        self.pkgver = get_str(&self.raw_variables, "pkgver");
        self.pkgrel = get_str(&self.raw_variables, "pkgrel");
        self.epoch = self.raw_variables.get("epoch").cloned();
        self.pkgdesc = get_str(&self.raw_variables, "pkgdesc");
        self.url = get_str(&self.raw_variables, "url");

        self.arch = get_array(&self.raw_variables, "arch");
        self.license = get_array(&self.raw_variables, "license");
        self.depends = get_array(&self.raw_variables, "depends");
        self.makedepends = get_array(&self.raw_variables, "makedepends");
        self.checkdepends = get_array(&self.raw_variables, "checkdepends");
        self.optdepends = get_array(&self.raw_variables, "optdepends");
        self.provides = get_array(&self.raw_variables, "provides");
        self.conflicts = get_array(&self.raw_variables, "conflicts");
        self.replaces = get_array(&self.raw_variables, "replaces");
        self.source = get_array(&self.raw_variables, "source");
        self.sha256sums = get_array(&self.raw_variables, "sha256sums");
        self.sha512sums = get_array(&self.raw_variables, "sha512sums");
        self.md5sums = get_array(&self.raw_variables, "md5sums");
        self.b2sums = get_array(&self.raw_variables, "b2sums");
        self.groups = get_array(&self.raw_variables, "groups");
        self.backup = get_array(&self.raw_variables, "backup");
        self.options = get_array(&self.raw_variables, "options");

        self.install = self.raw_variables.get("install").cloned();
        self.changelog = self.raw_variables.get("changelog").cloned();

        Ok(())
    }

    /// Expand Arch-specific variables in a string
    pub fn expand_variables(&self, input: &str) -> String {
        let mut result = input.to_string();

        // Standard variable expansions
        let expansions = [
            ("${pkgname}", &self.pkgname),
            ("$pkgname", &self.pkgname),
            ("${pkgbase}", &self.pkgname),  // pkgbase usually equals pkgname
            ("$pkgbase", &self.pkgname),
            ("${pkgver}", &self.pkgver),
            ("$pkgver", &self.pkgver),
            ("${pkgrel}", &self.pkgrel),
            ("$pkgrel", &self.pkgrel),
        ];

        for (pattern, value) in expansions {
            result = result.replace(pattern, value);
        }

        // Replace srcdir and pkgdir with rookpkg equivalents
        result = result.replace("$srcdir", "$ROOKPKG_BUILD");
        result = result.replace("${srcdir}", "$ROOKPKG_BUILD");
        result = result.replace("$pkgdir", "$ROOKPKG_DESTDIR");
        result = result.replace("${pkgdir}", "$ROOKPKG_DESTDIR");

        result
    }

    /// Get the full version string (epoch:pkgver-pkgrel)
    pub fn full_version(&self) -> String {
        if let Some(ref epoch) = self.epoch {
            format!("{}:{}-{}", epoch, self.pkgver, self.pkgrel)
        } else {
            format!("{}-{}", self.pkgver, self.pkgrel)
        }
    }

    /// Get version without release (for .rook)
    pub fn version(&self) -> String {
        if let Some(ref epoch) = self.epoch {
            format!("{}:{}", epoch, self.pkgver)
        } else {
            self.pkgver.clone()
        }
    }

    /// Get release number
    pub fn release(&self) -> u32 {
        self.pkgrel.parse().unwrap_or(1)
    }

    /// Get checksums (prefer sha256, fallback to others)
    pub fn checksums(&self) -> Vec<String> {
        if !self.sha256sums.is_empty() {
            self.sha256sums.clone()
        } else if !self.sha512sums.is_empty() {
            self.sha512sums.clone()
        } else if !self.b2sums.is_empty() {
            self.b2sums.clone()
        } else {
            self.md5sums.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pkgbuild() {
        let content = r#"
pkgname=example
pkgver=1.0.0
pkgrel=1
pkgdesc="An example package"
arch=('x86_64')
url="https://example.com"
license=('MIT')
depends=('glibc' 'openssl')
makedepends=('cmake' 'ninja')
source=("https://example.com/${pkgname}-${pkgver}.tar.gz")
sha256sums=('abc123def456')

build() {
    cd "$srcdir/$pkgname-$pkgver"
    cmake -B build
    cmake --build build
}

package() {
    cd "$srcdir/$pkgname-$pkgver"
    DESTDIR="$pkgdir" cmake --install build
}
"#;

        let pkg = Pkgbuild::parse(content).unwrap();

        assert_eq!(pkg.pkgname, "example");
        assert_eq!(pkg.pkgver, "1.0.0");
        assert_eq!(pkg.pkgrel, "1");
        assert_eq!(pkg.pkgdesc, "An example package");
        assert_eq!(pkg.url, "https://example.com");
        assert_eq!(pkg.arch, vec!["x86_64"]);
        assert_eq!(pkg.license, vec!["MIT"]);
        assert_eq!(pkg.depends, vec!["glibc", "openssl"]);
        assert_eq!(pkg.makedepends, vec!["cmake", "ninja"]);
        assert!(pkg.build_func.is_some());
        assert!(pkg.package_func.is_some());
    }

    #[test]
    fn test_expand_variables() {
        let mut pkg = Pkgbuild::default();
        pkg.pkgname = "mypackage".to_string();
        pkg.pkgver = "2.0.0".to_string();
        pkg.pkgrel = "1".to_string();

        let input = "cd $srcdir/${pkgname}-${pkgver}";
        let expanded = pkg.expand_variables(input);
        assert_eq!(expanded, "cd $ROOKPKG_BUILD/mypackage-2.0.0");

        let input2 = "DESTDIR=\"$pkgdir\" make install";
        let expanded2 = pkg.expand_variables(input2);
        assert_eq!(expanded2, "DESTDIR=\"$ROOKPKG_DESTDIR\" make install");
    }

    #[test]
    fn test_multiline_array() {
        let content = r#"
pkgname=test
pkgver=1.0
depends=(
    'dep1'
    'dep2'
    'dep3'
)
"#;

        let pkg = Pkgbuild::parse(content).unwrap();
        assert_eq!(pkg.depends.len(), 3);
        assert_eq!(pkg.depends[0], "dep1");
        assert_eq!(pkg.depends[2], "dep3");
    }
}
