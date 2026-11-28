use std::path::Path;
use std::process::exit;

const GAT_WALKABLE: u8 = 0;
const GAT_BLOCKED: u8 = 1;

/// The client builds the same field in memory, so changing this rule means
/// changing both repos.
fn flat_cells(x_size: i16, y_size: i16) -> Vec<u8> {
    let (x_size, y_size) = (x_size as usize, y_size as usize);
    (0..x_size * y_size)
        .map(|i| {
            let (x, y) = (i % x_size, i / x_size);
            if x == 0 || y == 0 || x == x_size - 1 || y == y_size - 1 {
                GAT_BLOCKED
            } else {
                GAT_WALKABLE
            }
        })
        .collect()
}

fn run(args: &[String]) -> Result<(), String> {
    let [name, x_size, y_size, dir] = args else {
        return Err("usage: flat-map-tool <map_name> <x_size> <y_size> <output_dir>".to_string());
    };
    let parse = |label: &str, value: &str| {
        value
            .parse::<i16>()
            .ok()
            .filter(|size| *size >= 3)
            .ok_or_else(|| format!("{label} must be an integer between 3 and {}, got '{value}'", i16::MAX))
    };
    let (x_size, y_size) = (parse("x_size", x_size)?, parse("y_size", y_size)?);
    map_cache::write_mcache(Path::new(dir), name, x_size, y_size, &flat_cells(x_size, y_size)).map_err(|e| e.to_string())?;
    println!("wrote {dir}/{name}.mcache ({x_size}x{y_size})");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(message) = run(&args) {
        eprintln!("{message}");
        exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_is_blocked_and_interior_walkable() {
        let cells = flat_cells(64, 64);

        assert_eq!(cells.len(), 64 * 64);
        let at = |x: usize, y: usize| cells[x + y * 64];
        assert_eq!(at(32, 32), GAT_WALKABLE);
        assert_eq!((at(1, 1), at(62, 62)), (GAT_WALKABLE, GAT_WALKABLE));
        assert_eq!(
            (at(0, 0), at(63, 10), at(10, 63), at(0, 32)),
            (GAT_BLOCKED, GAT_BLOCKED, GAT_BLOCKED, GAT_BLOCKED)
        );
        assert_eq!(cells.iter().filter(|c| **c == GAT_WALKABLE).count(), 62 * 62);
    }

    #[test]
    fn rejects_bad_arguments() {
        let args = |parts: &[&str]| parts.iter().map(|p| p.to_string()).collect::<Vec<_>>();

        assert!(run(&args(&["tanthu", "64", "64"])).is_err());
        assert!(run(&args(&["tanthu", "0", "64", "."])).is_err());
        assert!(run(&args(&["tanthu", "abc", "64", "."])).is_err());
        assert!(run(&args(&["tanthu", "2", "64", "."])).is_err());
    }
}
