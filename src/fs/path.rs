//! Path normalization for virtual filesystem paths.

/// Normalize a virtual path: resolve `.` and `..`, ensure leading `/`,
/// strip trailing slashes (except for root).
pub fn normalize(path: &str) -> String {
    let path = if path.is_empty() || !path.starts_with('/') {
        // Relative paths shouldn't appear but handle gracefully
        return String::from("/");
    } else {
        path
    };

    let mut components: Vec<&str> = Vec::new();

    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            other => components.push(other),
        }
    }

    if components.is_empty() {
        return String::from("/");
    }

    let mut result = String::with_capacity(path.len());
    for component in &components {
        result.push('/');
        result.push_str(component);
    }
    result
}

/// Return the parent directory of a path, or `/` for root.
pub fn parent(path: &str) -> &str {
    if path == "/" {
        return "/";
    }
    match path.rfind('/') {
        Some(0) | None => "/",
        Some(pos) => &path[..pos],
    }
}

/// Return the final component of a path.
pub fn basename(path: &str) -> &str {
    if path == "/" {
        return "/";
    }
    let trimmed = path.strip_suffix('/').unwrap_or(path);
    match trimmed.rfind('/') {
        Some(pos) => &trimmed[pos + 1..],
        None => trimmed,
    }
}

/// Join a base path with a relative component.
pub fn join(base: &str, relative: &str) -> String {
    if relative.starts_with('/') {
        return normalize(relative);
    }
    let combined = if base == "/" {
        format!("/{relative}")
    } else {
        format!("{base}/{relative}")
    };
    normalize(&combined)
}

/// Resolve a potentially relative path against a base directory.
pub fn resolve(base: &str, path: &str) -> String {
    if path.starts_with('/') {
        normalize(path)
    } else {
        join(base, path)
    }
}

/// Check if `path` starts with null bytes (security validation).
pub fn validate(path: &str) -> bool {
    !path.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_root() {
        assert_eq!(normalize("/"), "/");
    }

    #[test]
    fn normalize_simple() {
        assert_eq!(normalize("/home/user"), "/home/user");
    }

    #[test]
    fn normalize_trailing_slash() {
        assert_eq!(normalize("/home/user/"), "/home/user");
    }

    #[test]
    fn normalize_dots() {
        assert_eq!(normalize("/home/./user/../user/./docs"), "/home/user/docs");
    }

    #[test]
    fn normalize_double_dot_at_root() {
        assert_eq!(normalize("/.."), "/");
        assert_eq!(normalize("/../.."), "/");
    }

    #[test]
    fn normalize_double_slashes() {
        assert_eq!(normalize("//home///user//"), "/home/user");
    }

    #[test]
    fn parent_paths() {
        assert_eq!(parent("/"), "/");
        assert_eq!(parent("/home"), "/");
        assert_eq!(parent("/home/user"), "/home");
        assert_eq!(parent("/home/user/docs"), "/home/user");
    }

    #[test]
    fn basename_paths() {
        assert_eq!(basename("/"), "/");
        assert_eq!(basename("/home"), "home");
        assert_eq!(basename("/home/user"), "user");
        assert_eq!(basename("/home/user/"), "user");
    }

    #[test]
    fn join_absolute() {
        assert_eq!(join("/home", "/etc/passwd"), "/etc/passwd");
    }

    #[test]
    fn join_relative() {
        assert_eq!(join("/home/user", "docs/file.txt"), "/home/user/docs/file.txt");
    }

    #[test]
    fn join_from_root() {
        assert_eq!(join("/", "etc"), "/etc");
    }

    #[test]
    fn resolve_absolute() {
        assert_eq!(resolve("/home", "/etc/passwd"), "/etc/passwd");
    }

    #[test]
    fn resolve_relative() {
        assert_eq!(resolve("/home/user", "docs"), "/home/user/docs");
    }

    #[test]
    fn validate_clean_paths() {
        assert!(validate("/home/user"));
        assert!(validate("/"));
    }

    #[test]
    fn validate_null_byte() {
        assert!(!validate("/home/\0evil"));
    }
}
