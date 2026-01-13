//! Convert command - converts package specs from other distributions

use std::path::Path;

use anyhow::{bail, Result};
use colored::Colorize;

use crate::config::Config;
use crate::convert::ArchConverter;

/// Run Arch Linux conversion for a single package
pub fn run_arch_single(pkg_name: &str, output: Option<&Path>, _config: &Config) -> Result<()> {
    println!(
        "{} Converting Arch package: {}",
        "→".cyan(),
        pkg_name.bold()
    );

    let converter = ArchConverter::new()?;

    if converter.should_skip(pkg_name) {
        bail!(
            "Package '{}' is in the skip list (kernel, gcc, glibc, lib32, or arch-specific)",
            pkg_name
        );
    }

    let rook_content = converter.convert(pkg_name)?;

    // Determine output path
    let output_path = if let Some(dir) = output {
        std::fs::create_dir_all(dir)?;
        dir.join(format!("{}.rook", pkg_name))
    } else {
        std::path::PathBuf::from(format!("{}.rook", pkg_name))
    };

    std::fs::write(&output_path, &rook_content)?;

    println!(
        "{} Converted to: {}",
        "✓".green(),
        output_path.display()
    );
    println!();
    println!(
        "{}",
        "Review the generated file and run:".yellow()
    );
    println!(
        "  {} {} --update",
        "rookpkg checksum".cyan(),
        output_path.display()
    );

    Ok(())
}

/// Run Arch Linux conversion for all packages
pub fn run_arch_all(output_dir: &Path, _config: &Config) -> Result<()> {
    println!(
        "{} Converting all Arch Linux packages to: {}",
        "→".cyan(),
        output_dir.display()
    );
    println!();
    println!(
        "{}",
        "This will fetch and convert thousands of packages. This may take a while...".yellow()
    );
    println!();

    let converter = ArchConverter::new()?;
    let stats = converter.convert_all(output_dir)?;

    println!();
    println!("{}", "═".repeat(60).cyan());
    println!("{}", "Conversion Complete".bold());
    println!("{}", "═".repeat(60).cyan());
    println!();
    println!("  Total packages:    {}", stats.total);
    println!(
        "  {} Converted:        {}",
        "✓".green(),
        stats.converted
    );
    println!(
        "  {} Skipped:          {}",
        "○".yellow(),
        stats.skipped
    );
    println!(
        "  {} Failed:           {}",
        "✗".red(),
        stats.failed
    );

    if !stats.failed_packages.is_empty() && stats.failed_packages.len() <= 20 {
        println!();
        println!("{}", "Failed packages:".red());
        for pkg in &stats.failed_packages {
            println!("  - {}", pkg);
        }
    } else if stats.failed_packages.len() > 20 {
        println!();
        println!(
            "{} {} packages failed (too many to list)",
            "!".red(),
            stats.failed_packages.len()
        );
    }

    println!();
    println!("{}", "Next steps:".yellow());
    println!("  1. Review generated .rook files for correctness");
    println!("  2. Update checksums: rookpkg checksum {} --all --update", output_dir.display());
    println!("  3. Fix any dependency name mismatches");
    println!("  4. Build packages: rookpkg buildall {}", output_dir.display());

    Ok(())
}
