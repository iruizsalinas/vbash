//! Run with: cargo run --example network --features network

#[cfg(feature = "network")]
fn main() {
    use vbash::{NetworkPolicy, Shell};

    let policy = NetworkPolicy {
        allowed_url_prefixes: vec![
            "https://httpbin.org/".to_string(),
        ],
        block_private_ips: true,
        allowed_methods: vec!["GET".into(), "HEAD".into()],
        max_response_size: 1024 * 1024, // 1MB
        max_redirects: 5,
    };

    let mut shell = Shell::builder()
        .network_policy(policy)
        .build();

    let r = shell.exec("curl -s https://httpbin.org/get | jq '.url'").unwrap();
    println!("response: {}", r.stdout.trim());

    // these will be rejected
    let r = shell.exec("curl http://localhost/secret 2>&1").unwrap();
    println!("blocked: {}", r.stdout.trim());

    let r = shell.exec("curl https://evil.com/ 2>&1").unwrap();
    println!("blocked: {}", r.stdout.trim());
}

#[cfg(not(feature = "network"))]
fn main() {
    eprintln!("run with: cargo run --example network --features network");
}
