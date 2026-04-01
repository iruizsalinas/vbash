#[cfg(feature = "network")]
use crate::NetworkPolicy;

/// Parse a single IPv4 address component, supporting decimal, octal (0-prefix),
/// and hexadecimal (0x-prefix) notations.
fn parse_ipv4_component(part: &str) -> Option<u64> {
    if part.is_empty() {
        return None;
    }
    if part.starts_with("0x") || part.starts_with("0X") {
        let hex = &part[2..];
        if hex.is_empty() {
            return None;
        }
        u64::from_str_radix(hex, 16).ok()
    } else if part.len() > 1 && part.starts_with('0') {
        // Octal
        u64::from_str_radix(&part[1..], 8).ok()
    } else {
        part.parse::<u64>().ok()
    }
}

/// Parse an IPv4 address supporting 1-4 part notations:
/// - 1 part:  32-bit integer
/// - 2 parts: a.B (B is 24-bit)
/// - 3 parts: a.b.C (C is 16-bit)
/// - 4 parts: a.b.c.d (standard dotted-quad)
///
/// Each part can be decimal, octal (0-prefix), or hex (0x-prefix).
fn parse_ipv4(host: &str) -> Option<[u8; 4]> {
    let parts: Vec<&str> = host.split('.').collect();
    match parts.len() {
        1 => {
            let n = parse_ipv4_component(parts[0])?;
            if n > 0xFFFF_FFFF {
                return None;
            }
            Some([
                (n >> 24) as u8,
                (n >> 16) as u8,
                (n >> 8) as u8,
                n as u8,
            ])
        }
        2 => {
            let a = parse_ipv4_component(parts[0])?;
            if a > 255 {
                return None;
            }
            let b = parse_ipv4_component(parts[1])?;
            if b > 0xFF_FFFF {
                return None;
            }
            Some([a as u8, (b >> 16) as u8, (b >> 8) as u8, b as u8])
        }
        3 => {
            let a = parse_ipv4_component(parts[0])?;
            if a > 255 {
                return None;
            }
            let b = parse_ipv4_component(parts[1])?;
            if b > 255 {
                return None;
            }
            let c = parse_ipv4_component(parts[2])?;
            if c > 0xFFFF {
                return None;
            }
            Some([a as u8, b as u8, (c >> 8) as u8, c as u8])
        }
        4 => {
            let a = parse_ipv4_component(parts[0])?;
            if a > 255 {
                return None;
            }
            let b = parse_ipv4_component(parts[1])?;
            if b > 255 {
                return None;
            }
            let c = parse_ipv4_component(parts[2])?;
            if c > 255 {
                return None;
            }
            let d = parse_ipv4_component(parts[3])?;
            if d > 255 {
                return None;
            }
            Some([a as u8, b as u8, c as u8, d as u8])
        }
        _ => None,
    }
}

/// Check whether an IPv4 address falls in a private/reserved range.
fn is_private_ipv4(ip: [u8; 4]) -> bool {
    let [a, b, c, _] = ip;
    a == 127                                    // 127.0.0.0/8 loopback
    || a == 10                                  // 10.0.0.0/8
    || (a == 172 && (16..=31).contains(&b))     // 172.16.0.0/12
    || (a == 192 && b == 168)                   // 192.168.0.0/16
    || (a == 169 && b == 254)                   // 169.254.0.0/16 link-local
    || a == 0                                   // 0.0.0.0/8
    || (a == 100 && (64..=127).contains(&b))    // 100.64.0.0/10 CGNAT/RFC 6598
    || (a == 198 && (b == 18 || b == 19))       // 198.18.0.0/15 benchmarking
    || (a == 192 && b == 0 && c == 0)           // 192.0.0.0/24 IETF
    || (a == 192 && b == 0 && c == 2)           // 192.0.2.0/24 TEST-NET-1
    || (a == 198 && b == 51 && c == 100)        // 198.51.100.0/24 TEST-NET-2
    || (a == 203 && b == 0 && c == 113)         // 203.0.113.0/24 TEST-NET-3
    || a >= 240                                 // 240.0.0.0/4 reserved
}

/// Parse an IPv6 address string into 8 hextets.
/// Handles `::` zero compression, bracketed notation, and IPv4-mapped suffixes.
fn parse_ipv6(host: &str) -> Option<[u16; 8]> {
    let h = host
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(host);

    // Strip zone ID (e.g., %eth0)
    let h = h.split('%').next().unwrap_or(h);

    if h.is_empty() {
        return None;
    }

    // Split on "::" - only one allowed
    let double_colon_parts: Vec<&str> = h.splitn(3, "::").collect();
    if double_colon_parts.len() > 2 {
        // More than one "::" found
        return None;
    }

    let (left_str, right_str) = if double_colon_parts.len() == 2 {
        (double_colon_parts[0], double_colon_parts[1])
    } else {
        (h, "")
    };
    let has_double_colon = double_colon_parts.len() == 2;

    let left_parts: Vec<&str> = if left_str.is_empty() {
        Vec::new()
    } else {
        left_str.split(':').collect()
    };

    let right_parts: Vec<&str> = if right_str.is_empty() {
        Vec::new()
    } else {
        right_str.split(':').collect()
    };

    // Check if the last part is an IPv4 address (IPv4-mapped/compatible)
    let last_part = if !right_parts.is_empty() {
        right_parts.last().copied()
    } else if !left_parts.is_empty() {
        left_parts.last().copied()
    } else {
        None
    };

    let has_ipv4_suffix =
        last_part.is_some_and(|p| p.contains('.'));

    let mut hextets = [0u16; 8];

    if has_ipv4_suffix {
        // Parse trailing IPv4
        let ipv4_str = last_part.unwrap();
        let ipv4_octets: Vec<&str> = ipv4_str.split('.').collect();
        if ipv4_octets.len() != 4 {
            return None;
        }
        let mut ipv4_bytes = [0u8; 4];
        for (i, octet_str) in ipv4_octets.iter().enumerate() {
            ipv4_bytes[i] = octet_str.parse::<u8>().ok()?;
        }
        hextets[6] = (u16::from(ipv4_bytes[0]) << 8) | u16::from(ipv4_bytes[1]);
        hextets[7] = (u16::from(ipv4_bytes[2]) << 8) | u16::from(ipv4_bytes[3]);

        // Parse hex hextets (everything except the last part which is IPv4)
        let hex_left = &left_parts;
        let hex_right = if right_parts.is_empty() {
            // IPv4 was in left_parts
            let hl = &left_parts[..left_parts.len() - 1];
            // Rebuild: left hextets only
            let total_specified = hl.len();
            if !has_double_colon && total_specified != 6 {
                return None;
            }
            for (i, part) in hl.iter().enumerate() {
                hextets[i] = u16::from_str_radix(part, 16).ok()?;
            }
            return Some(hextets);
        } else {
            &right_parts[..right_parts.len() - 1]
        };

        let total_hex = hex_left.len() + hex_right.len();
        // 6 hextet positions are available (indices 0..5), last 2 are IPv4
        if total_hex > 6 {
            return None;
        }
        if !has_double_colon && total_hex != 6 {
            return None;
        }

        for (i, part) in hex_left.iter().enumerate() {
            hextets[i] = u16::from_str_radix(part, 16).ok()?;
        }
        let zero_fill = 6 - total_hex;
        let right_start = hex_left.len() + zero_fill;
        for (i, part) in hex_right.iter().enumerate() {
            hextets[right_start + i] = u16::from_str_radix(part, 16).ok()?;
        }
    } else {
        let total = left_parts.len() + right_parts.len();
        if total > 8 {
            return None;
        }
        if !has_double_colon && total != 8 {
            return None;
        }

        for (i, part) in left_parts.iter().enumerate() {
            hextets[i] = u16::from_str_radix(part, 16).ok()?;
        }
        let zero_fill = 8 - total;
        let right_start = left_parts.len() + zero_fill;
        for (i, part) in right_parts.iter().enumerate() {
            hextets[right_start + i] = u16::from_str_radix(part, 16).ok()?;
        }
    }

    Some(hextets)
}

/// Check whether an IPv6 address falls in a private/reserved range.
fn is_private_ipv6(hextets: [u16; 8]) -> bool {
    // All zeros (::)
    if hextets.iter().all(|&h| h == 0) {
        return true;
    }
    // Loopback ::1
    if hextets[..7].iter().all(|&h| h == 0) && hextets[7] == 1 {
        return true;
    }
    // fe80::/10 link-local
    if (hextets[0] & 0xFFC0) == 0xFE80 {
        return true;
    }
    // fc00::/7 unique local
    if (hextets[0] & 0xFE00) == 0xFC00 {
        return true;
    }
    // ::ffff:x.x.x.x IPv4-mapped
    if hextets[..5].iter().all(|&h| h == 0) && hextets[5] == 0xFFFF {
        let mapped = [
            (hextets[6] >> 8) as u8,
            hextets[6] as u8,
            (hextets[7] >> 8) as u8,
            hextets[7] as u8,
        ];
        return is_private_ipv4(mapped);
    }
    // 2001:db8::/32 documentation
    if hextets[0] == 0x2001 && hextets[1] == 0x0DB8 {
        return true;
    }
    // 64:ff9b::/96 NAT64 - check embedded IPv4
    if hextets[0] == 0x0064
        && hextets[1] == 0xFF9B
        && hextets[2..6].iter().all(|&h| h == 0)
    {
        let embedded = [
            (hextets[6] >> 8) as u8,
            hextets[6] as u8,
            (hextets[7] >> 8) as u8,
            hextets[7] as u8,
        ];
        return is_private_ipv4(embedded);
    }
    // 64:ff9b:1::/48 NAT64 local-use
    if hextets[0] == 0x0064 && hextets[1] == 0xFF9B && hextets[2] == 1 {
        return true;
    }
    // 2002::/16 6to4 - check embedded IPv4
    if hextets[0] == 0x2002 {
        let embedded = [
            (hextets[1] >> 8) as u8,
            hextets[1] as u8,
            (hextets[2] >> 8) as u8,
            hextets[2] as u8,
        ];
        return is_private_ipv4(embedded);
    }
    false
}

/// Check if a hostname or IP literal is a private/loopback address.
/// Handles octal, hex, and multi-notation IPv4 as well as full IPv6 parsing.
pub(super) fn is_private_host(host: &str) -> bool {
    let h = host.trim().to_ascii_lowercase();
    let h = h
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(&h);

    if h == "localhost" || h.ends_with(".localhost") {
        return true;
    }
    if let Some(ip) = parse_ipv4(h) {
        return is_private_ipv4(ip);
    }
    if let Some(hextets) = parse_ipv6(h) {
        return is_private_ipv6(hextets);
    }
    false
}

/// Split a URL into (origin, path). The origin includes scheme + host + port.
pub(super) fn split_origin_path(url: &str) -> (&str, &str) {
    // Find scheme end
    let after_scheme = if let Some(pos) = url.find("://") {
        pos + 3
    } else {
        return (url, "");
    };
    // Find the first '/' after the authority
    if let Some(slash_pos) = url[after_scheme..].find('/') {
        let origin_end = after_scheme + slash_pos;
        (&url[..origin_end], &url[origin_end..])
    } else {
        (url, "")
    }
}

/// Segment-aware URL prefix matching. Origins must match exactly (case-insensitive
/// for the host part), and paths are matched on segment boundaries.
/// Rejects URLs containing encoded path separators to prevent traversal.
pub(super) fn matches_url_prefix(url: &str, prefix: &str) -> bool {
    let (url_origin, url_path) = split_origin_path(url);
    let (prefix_origin, prefix_path) = split_origin_path(prefix);

    // Origins must match exactly (case-insensitive for host)
    if !url_origin.eq_ignore_ascii_case(prefix_origin) {
        return false;
    }

    // Reject ambiguous path separators in URL
    let path_lower = url_path.to_ascii_lowercase();
    if url_path.contains('\\') || path_lower.contains("%2f") || path_lower.contains("%5c") {
        return false;
    }

    // Segment-aware path matching
    if prefix_path.is_empty() || prefix_path == "/" {
        return true;
    }
    if prefix_path.ends_with('/') {
        return url_path.starts_with(prefix_path);
    }
    url_path == prefix_path || url_path.starts_with(&format!("{prefix_path}/"))
}

/// Extract the host portion from a URL string (without port).
pub(super) fn extract_url_host(url: &str) -> Option<&str> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);
    // Strip userinfo (user:pass@host)
    let after_at = host_port.rsplit('@').next().unwrap_or(host_port);
    // Handle IPv6 bracket notation: [::1]:port
    if after_at.starts_with('[') {
        let end = after_at.find(']')?;
        Some(&after_at[..=end])
    } else {
        // Strip port
        Some(after_at.split(':').next().unwrap_or(after_at))
    }
}

/// Validate a URL against the given network policy.
/// Returns `Ok(())` if the request is allowed, or `Err(message)` if blocked.
pub(super) fn validate_url(url: &str, policy: Option<&NetworkPolicy>) -> Result<(), String> {
    let Some(policy) = policy else {
        return Err(
            "curl: network requests are blocked (no network policy configured)".to_string(),
        );
    };

    // Only allow http and https schemes (defense in depth)
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!(
            "curl: unsupported protocol in URL '{url}' (only http and https are allowed)"
        ));
    }

    // Check allowed URL prefixes (if any are set)
    if !policy.allowed_url_prefixes.is_empty() {
        let allowed = policy
            .allowed_url_prefixes
            .iter()
            .any(|prefix| matches_url_prefix(url, prefix));
        if !allowed {
            return Err(format!(
                "curl: URL '{url}' is not in the allowed URL prefixes"
            ));
        }
    }

    if policy.block_private_ips {
        if let Some(host) = extract_url_host(url) {
            if is_private_host(host) {
                return Err(format!(
                    "curl: requests to private/loopback address '{host}' are blocked"
                ));
            }
            resolve_and_check_dns(host)?;
        }
    }

    Ok(())
}

/// Resolve a hostname via DNS and check all returned IPs against private ranges.
/// Skips IP literals (already handled by `is_private_host`).
/// Fails closed on unexpected DNS errors.
fn resolve_and_check_dns(host: &str) -> Result<(), String> {
    use std::net::ToSocketAddrs;

    if !host.chars().any(|c| c.is_ascii_alphabetic()) {
        return Ok(());
    }

    match (host, 0u16).to_socket_addrs() {
        Ok(addrs) => {
            for addr in addrs {
                let ip_str = addr.ip().to_string();
                if is_private_host(&ip_str) {
                    return Err(format!(
                        "curl: hostname '{host}' resolves to private IP {ip_str}"
                    ));
                }
            }
            Ok(())
        }
        Err(e) => {
            let msg = e.to_string().to_ascii_lowercase();
            if msg.contains("no such host")
                || msg.contains("name or service not known")
                || msg.contains("nodename nor servname")
                || msg.contains("no address")
                || msg.contains("getaddrinfo")
            {
                Ok(())
            } else {
                Err(format!("curl: DNS lookup failed for '{host}': {e}"))
            }
        }
    }
}

/// Validate that the HTTP method is in the policy's allowlist.
pub(super) fn validate_method(method: &str, policy: Option<&NetworkPolicy>) -> Result<(), String> {
    let Some(policy) = policy else {
        return Ok(());
    };
    if policy.allowed_methods.is_empty() {
        return Ok(());
    }
    let method_upper = method.to_ascii_uppercase();
    if policy
        .allowed_methods
        .iter()
        .any(|m| m.to_ascii_uppercase() == method_upper)
    {
        Ok(())
    } else {
        Err(format!(
            "curl: HTTP method '{method}' is not allowed (allowed: {})",
            policy.allowed_methods.join(", ")
        ))
    }
}

/// Resolve a potentially relative redirect URL against the original request URL.
pub(super) fn resolve_redirect_url(base: &str, location: &str) -> String {
    // If the Location is already absolute, use it directly
    if location.starts_with("http://") || location.starts_with("https://") {
        return location.to_string();
    }
    // Extract the origin from the base URL
    let (origin, _) = split_origin_path(base);
    if location.starts_with('/') {
        // Absolute path - combine with origin
        format!("{origin}{location}")
    } else {
        // Relative path - combine with base path directory
        let (_, base_path) = split_origin_path(base);
        let dir = if let Some(last_slash) = base_path.rfind('/') {
            &base_path[..=last_slash]
        } else {
            "/"
        };
        format!("{origin}{dir}{location}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

#[test]
fn parse_ipv4_standard_dotted_quad() {
    assert_eq!(parse_ipv4("192.168.1.1"), Some([192, 168, 1, 1]));
    assert_eq!(parse_ipv4("10.0.0.1"), Some([10, 0, 0, 1]));
    assert_eq!(parse_ipv4("0.0.0.0"), Some([0, 0, 0, 0]));
    assert_eq!(parse_ipv4("255.255.255.255"), Some([255, 255, 255, 255]));
}

#[test]
fn parse_ipv4_octal_notation() {
    // 0177 = 127 in octal
    assert_eq!(parse_ipv4("0177.0.0.1"), Some([127, 0, 0, 1]));
    // 0300 = 192, 0250 = 168
    assert_eq!(parse_ipv4("0300.0250.1.1"), Some([192, 168, 1, 1]));
}

#[test]
fn parse_ipv4_hex_notation() {
    // 0x7f = 127
    assert_eq!(parse_ipv4("0x7f.0.0.1"), Some([127, 0, 0, 1]));
}

#[test]
fn parse_ipv4_single_integer() {
    // 0x7f000001 = 2130706433 = 127.0.0.1
    assert_eq!(parse_ipv4("0x7f000001"), Some([127, 0, 0, 1]));
    // Decimal single integer
    assert_eq!(parse_ipv4("2130706433"), Some([127, 0, 0, 1]));
}

#[test]
fn parse_ipv4_two_parts() {
    // 127.1 = 127.0.0.1
    assert_eq!(parse_ipv4("127.1"), Some([127, 0, 0, 1]));
}

#[test]
fn parse_ipv4_three_parts() {
    // 127.0.1 = 127.0.0.1
    assert_eq!(parse_ipv4("127.0.1"), Some([127, 0, 0, 1]));
}

#[test]
fn is_private_ipv4_loopback() {
    assert!(is_private_ipv4([127, 0, 0, 1]));
    assert!(is_private_ipv4([127, 255, 255, 255]));
}

#[test]
fn is_private_ipv4_rfc1918() {
    assert!(is_private_ipv4([10, 0, 0, 1]));
    assert!(is_private_ipv4([172, 16, 0, 1]));
    assert!(is_private_ipv4([172, 31, 255, 255]));
    assert!(is_private_ipv4([192, 168, 0, 1]));
}

#[test]
fn is_private_ipv4_link_local() {
    assert!(is_private_ipv4([169, 254, 169, 254]));
}

#[test]
fn is_private_ipv4_cgnat() {
    assert!(is_private_ipv4([100, 64, 0, 1]));
    assert!(is_private_ipv4([100, 127, 255, 255]));
    assert!(!is_private_ipv4([100, 63, 255, 255]));
    assert!(!is_private_ipv4([100, 128, 0, 0]));
}

#[test]
fn is_private_ipv4_test_nets() {
    assert!(is_private_ipv4([192, 0, 2, 1]));   // TEST-NET-1
    assert!(is_private_ipv4([198, 51, 100, 1])); // TEST-NET-2
    assert!(is_private_ipv4([203, 0, 113, 1]));  // TEST-NET-3
}

#[test]
fn is_private_ipv4_benchmarking() {
    assert!(is_private_ipv4([198, 18, 0, 1]));
    assert!(is_private_ipv4([198, 19, 255, 255]));
}

#[test]
fn is_private_ipv4_ietf() {
    assert!(is_private_ipv4([192, 0, 0, 1]));
}

#[test]
fn is_private_ipv4_reserved() {
    assert!(is_private_ipv4([240, 0, 0, 1]));
    assert!(is_private_ipv4([255, 255, 255, 255]));
}

#[test]
fn is_private_ipv4_zero() {
    assert!(is_private_ipv4([0, 0, 0, 0]));
}

#[test]
fn is_private_ipv4_public() {
    assert!(!is_private_ipv4([8, 8, 8, 8]));
    assert!(!is_private_ipv4([1, 1, 1, 1]));
    assert!(!is_private_ipv4([142, 250, 80, 14])); // google
}

#[test]
fn parse_ipv6_loopback() {
    assert_eq!(parse_ipv6("::1"), Some([0, 0, 0, 0, 0, 0, 0, 1]));
}

#[test]
fn parse_ipv6_all_zeros() {
    assert_eq!(parse_ipv6("::"), Some([0, 0, 0, 0, 0, 0, 0, 0]));
}

#[test]
fn parse_ipv6_full() {
    assert_eq!(
        parse_ipv6("2001:0db8:0000:0000:0000:0000:0000:0001"),
        Some([0x2001, 0x0db8, 0, 0, 0, 0, 0, 1])
    );
}

#[test]
fn parse_ipv6_compressed() {
    assert_eq!(
        parse_ipv6("2001:db8::1"),
        Some([0x2001, 0x0db8, 0, 0, 0, 0, 0, 1])
    );
}

#[test]
fn parse_ipv6_bracketed() {
    assert_eq!(parse_ipv6("[::1]"), Some([0, 0, 0, 0, 0, 0, 0, 1]));
}

#[test]
fn parse_ipv6_ipv4_mapped() {
    // ::ffff:127.0.0.1
    assert_eq!(
        parse_ipv6("::ffff:127.0.0.1"),
        Some([0, 0, 0, 0, 0, 0xFFFF, 0x7F00, 0x0001])
    );
}

#[test]
fn parse_ipv6_link_local() {
    assert_eq!(
        parse_ipv6("fe80::1"),
        Some([0xFE80, 0, 0, 0, 0, 0, 0, 1])
    );
}

#[test]
fn is_private_ipv6_loopback() {
    assert!(is_private_ipv6([0, 0, 0, 0, 0, 0, 0, 1]));
}

#[test]
fn is_private_ipv6_all_zeros() {
    assert!(is_private_ipv6([0, 0, 0, 0, 0, 0, 0, 0]));
}

#[test]
fn is_private_ipv6_link_local() {
    assert!(is_private_ipv6([0xFE80, 0, 0, 0, 0, 0, 0, 1]));
}

#[test]
fn is_private_ipv6_unique_local() {
    assert!(is_private_ipv6([0xFC00, 0, 0, 0, 0, 0, 0, 1]));
    assert!(is_private_ipv6([0xFD00, 0, 0, 0, 0, 0, 0, 1]));
}

#[test]
fn is_private_ipv6_ipv4_mapped() {
    // ::ffff:127.0.0.1 = [0,0,0,0,0,0xFFFF,0x7F00,0x0001]
    assert!(is_private_ipv6([0, 0, 0, 0, 0, 0xFFFF, 0x7F00, 0x0001]));
    // ::ffff:8.8.8.8 = [0,0,0,0,0,0xFFFF,0x0808,0x0808]
    assert!(!is_private_ipv6([0, 0, 0, 0, 0, 0xFFFF, 0x0808, 0x0808]));
}

#[test]
fn is_private_ipv6_documentation() {
    assert!(is_private_ipv6([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 1]));
}

#[test]
fn is_private_ipv6_nat64() {
    // 64:ff9b:: with embedded private IPv4
    assert!(is_private_ipv6([0x0064, 0xFF9B, 0, 0, 0, 0, 0x7F00, 0x0001]));
    // 64:ff9b:: with embedded public IPv4
    assert!(!is_private_ipv6([0x0064, 0xFF9B, 0, 0, 0, 0, 0x0808, 0x0808]));
}

#[test]
fn is_private_ipv6_nat64_local_use() {
    assert!(is_private_ipv6([0x0064, 0xFF9B, 1, 0, 0, 0, 0, 0]));
}

#[test]
fn is_private_ipv6_6to4() {
    // 2002:7f00:0001:: embeds 127.0.0.1
    assert!(is_private_ipv6([0x2002, 0x7F00, 0x0001, 0, 0, 0, 0, 0]));
    // 2002:0808:0808:: embeds 8.8.8.8
    assert!(!is_private_ipv6([0x2002, 0x0808, 0x0808, 0, 0, 0, 0, 0]));
}

#[test]
fn is_private_ipv6_public() {
    assert!(!is_private_ipv6([0x2607, 0xF8B0, 0x4004, 0x0800, 0, 0, 0, 0x200E]));
}

#[test]
fn private_host_localhost() {
    assert!(is_private_host("localhost"));
    assert!(is_private_host("foo.localhost"));
    assert!(is_private_host("LOCALHOST"));
}

#[test]
fn private_host_octal_bypass() {
    // 0177.0.0.1 = 127.0.0.1
    assert!(is_private_host("0177.0.0.1"));
}

#[test]
fn private_host_hex_bypass() {
    // 0x7f000001 = 127.0.0.1
    assert!(is_private_host("0x7f000001"));
}

#[test]
fn private_host_ipv4_mapped_ipv6() {
    assert!(is_private_host("::ffff:127.0.0.1"));
}

#[test]
fn private_host_ipv6_bracketed() {
    assert!(is_private_host("[::1]"));
}

#[test]
fn private_host_cgnat() {
    assert!(is_private_host("100.64.0.1"));
}

#[test]
fn private_host_test_nets() {
    assert!(is_private_host("192.0.2.1"));
    assert!(is_private_host("198.51.100.1"));
    assert!(is_private_host("203.0.113.1"));
}

#[test]
fn private_host_6to4_embedded() {
    assert!(is_private_host("2002:7f00:0001::"));
}

#[test]
fn private_host_public() {
    assert!(!is_private_host("8.8.8.8"));
    assert!(!is_private_host("example.com"));
    assert!(!is_private_host("1.1.1.1"));
}

#[test]
fn prefix_exact_segment_match() {
    assert!(matches_url_prefix(
        "https://api.com/v1/users",
        "https://api.com/v1"
    ));
}

#[test]
fn prefix_rejects_partial_segment() {
    assert!(!matches_url_prefix(
        "https://api.com/v10",
        "https://api.com/v1"
    ));
}

#[test]
fn prefix_path_traversal_blocked() {
    assert!(!matches_url_prefix(
        "https://api.com/%2f..%2f/admin",
        "https://api.com/api"
    ));
}

#[test]
fn prefix_backslash_blocked() {
    assert!(!matches_url_prefix(
        "https://api.com/api\\..\\admin",
        "https://api.com/api"
    ));
}

#[test]
fn prefix_origin_only() {
    assert!(matches_url_prefix(
        "https://api.com/anything",
        "https://api.com"
    ));
}

#[test]
fn prefix_trailing_slash() {
    assert!(matches_url_prefix(
        "https://api.com/v1/users",
        "https://api.com/v1/"
    ));
    assert!(!matches_url_prefix(
        "https://api.com/v10/users",
        "https://api.com/v1/"
    ));
}

#[test]
fn prefix_different_origin() {
    assert!(!matches_url_prefix(
        "https://evil.com/v1",
        "https://api.com/v1"
    ));
}

#[test]
fn prefix_case_insensitive_origin() {
    assert!(matches_url_prefix(
        "https://API.COM/v1/users",
        "https://api.com/v1"
    ));
}

#[test]
fn prefix_encoded_separator_in_url() {
    assert!(!matches_url_prefix(
        "https://api.com/api%2Fother",
        "https://api.com/api"
    ));
    assert!(!matches_url_prefix(
        "https://api.com/api%5Cother",
        "https://api.com/api"
    ));
}

#[test]
fn no_policy_blocks_all_requests() {
    let result = validate_url("https://example.com", None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("no network policy"));
}

#[test]
fn empty_prefixes_allows_any_non_private() {
    let policy = NetworkPolicy::default();
    assert!(validate_url("https://example.com/api", Some(&policy)).is_ok());
}

#[test]
fn prefix_filter_blocks_non_matching() {
    let policy = NetworkPolicy {
        allowed_url_prefixes: vec!["https://api.example.com/".to_string()],
        ..NetworkPolicy::default()
    };
    assert!(validate_url("https://api.example.com/v1/data", Some(&policy)).is_ok());
    assert!(validate_url("https://evil.com/", Some(&policy)).is_err());
}

#[test]
fn private_ips_blocked() {
    let policy = NetworkPolicy::default();
    assert!(validate_url("http://localhost/admin", Some(&policy)).is_err());
    assert!(validate_url("http://127.0.0.1/secret", Some(&policy)).is_err());
    assert!(validate_url("http://[::1]/secret", Some(&policy)).is_err());
    assert!(validate_url("http://0.0.0.0/", Some(&policy)).is_err());
    assert!(validate_url("http://10.0.0.1/internal", Some(&policy)).is_err());
    assert!(validate_url("http://172.16.0.1/internal", Some(&policy)).is_err());
    assert!(validate_url("http://172.31.255.1/internal", Some(&policy)).is_err());
    assert!(validate_url("http://192.168.1.1/router", Some(&policy)).is_err());
    assert!(validate_url("http://169.254.169.254/metadata", Some(&policy)).is_err());
}

#[test]
fn private_ips_allowed_when_disabled() {
    let policy = NetworkPolicy {
        block_private_ips: false,
        ..NetworkPolicy::default()
    };
    assert!(validate_url("http://localhost/admin", Some(&policy)).is_ok());
    assert!(validate_url("http://127.0.0.1/secret", Some(&policy)).is_ok());
    assert!(validate_url("http://10.0.0.1/internal", Some(&policy)).is_ok());
}

#[test]
fn extract_host_basic() {
    assert_eq!(extract_url_host("https://example.com/path"), Some("example.com"));
    assert_eq!(extract_url_host("http://10.0.0.1:8080/api"), Some("10.0.0.1"));
    assert_eq!(extract_url_host("http://[::1]:8080/api"), Some("[::1]"));
    assert_eq!(extract_url_host("http://user:pass@host.com/path"), Some("host.com"));
}

#[test]
fn non_private_172_allowed() {
    let policy = NetworkPolicy::default();
    // 172.15.x.x and 172.32.x.x are NOT private
    assert!(validate_url("http://172.15.0.1/ok", Some(&policy)).is_ok());
    assert!(validate_url("http://172.32.0.1/ok", Some(&policy)).is_ok());
}

#[test]
fn octal_ip_bypass_blocked() {
    let policy = NetworkPolicy::default();
    // 0177.0.0.1 = 127.0.0.1 in octal
    assert!(validate_url("http://0177.0.0.1/secret", Some(&policy)).is_err());
}

#[test]
fn hex_ip_bypass_blocked() {
    let policy = NetworkPolicy::default();
    // 0x7f000001 = 127.0.0.1
    assert!(validate_url("http://0x7f000001/secret", Some(&policy)).is_err());
}

#[test]
fn ipv4_mapped_ipv6_blocked() {
    let policy = NetworkPolicy::default();
    assert!(validate_url("http://[::ffff:127.0.0.1]/secret", Some(&policy)).is_err());
}

#[test]
fn segment_aware_prefix_matching_in_validation() {
    let policy = NetworkPolicy {
        allowed_url_prefixes: vec!["https://api.com/v1".to_string()],
        ..NetworkPolicy::default()
    };
    assert!(validate_url("https://api.com/v1/users", Some(&policy)).is_ok());
    assert!(validate_url("https://api.com/v10", Some(&policy)).is_err());
}

#[test]
fn path_traversal_in_validation() {
    let policy = NetworkPolicy {
        allowed_url_prefixes: vec!["https://api.com/api".to_string()],
        ..NetworkPolicy::default()
    };
    assert!(validate_url("https://api.com/%2f..%2f/admin", Some(&policy)).is_err());
}

#[test]
fn cgnat_blocked_in_validation() {
    let policy = NetworkPolicy::default();
    assert!(validate_url("http://100.64.0.1/internal", Some(&policy)).is_err());
}

#[test]
fn test_nets_blocked_in_validation() {
    let policy = NetworkPolicy::default();
    assert!(validate_url("http://192.0.2.1/test", Some(&policy)).is_err());
    assert!(validate_url("http://198.51.100.1/test", Some(&policy)).is_err());
    assert!(validate_url("http://203.0.113.1/test", Some(&policy)).is_err());
}

#[test]
fn validate_method_allowed() {
    let policy = NetworkPolicy::default();
    assert!(validate_method("GET", Some(&policy)).is_ok());
    assert!(validate_method("POST", Some(&policy)).is_ok());
    assert!(validate_method("get", Some(&policy)).is_ok()); // case-insensitive
}

#[test]
fn validate_method_blocked() {
    let policy = NetworkPolicy::default();
    assert!(validate_method("CONNECT", Some(&policy)).is_err());
    assert!(validate_method("TRACE", Some(&policy)).is_err());
}

#[test]
fn validate_method_no_policy() {
    assert!(validate_method("ANYTHING", None).is_ok());
}

#[test]
fn resolve_absolute_redirect() {
    assert_eq!(
        resolve_redirect_url("https://a.com/path", "https://b.com/other"),
        "https://b.com/other"
    );
}

#[test]
fn resolve_absolute_path_redirect() {
    assert_eq!(
        resolve_redirect_url("https://a.com/old/path", "/new/path"),
        "https://a.com/new/path"
    );
}

#[test]
fn resolve_relative_path_redirect() {
    assert_eq!(
        resolve_redirect_url("https://a.com/dir/page", "other"),
        "https://a.com/dir/other"
    );
}

#[test]
fn dns_skip_for_ip_literal() {
    assert!(resolve_and_check_dns("127.0.0.1").is_ok());
}

#[test]
fn dns_skip_for_ipv6_literal() {
    assert!(resolve_and_check_dns("::1").is_ok());
}

#[test]
fn dns_nonexistent_domain_allowed() {
    assert!(resolve_and_check_dns("this-domain-does-not-exist-xyz123.invalid").is_ok());
}

#[test]
fn dns_empty_host_skipped() {
    assert!(resolve_and_check_dns("").is_ok());
}
}
