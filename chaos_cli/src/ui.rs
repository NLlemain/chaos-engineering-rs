// UI utility functions for terminal output
use colored::Colorize;

#[allow(dead_code)]
pub fn print_header(text: &str) {
    println!("\n{}", text.bold().cyan());
    println!("{}", "=".repeat(text.len()).cyan());
}

#[allow(dead_code)]
pub fn print_success(text: &str) {
    println!("{} {}", "✓".green().bold(), text.green());
}

#[allow(dead_code)]
pub fn print_error(text: &str) {
    println!("{} {}", "✗".red().bold(), text.red());
}

#[allow(dead_code)]
pub fn print_warning(text: &str) {
    println!("{} {}", "⚠".yellow().bold(), text.yellow());
}

#[allow(dead_code)]
pub fn print_info(text: &str) {
    println!("{} {}", "ℹ".blue().bold(), text);
}
