use repositories::remote::{RemoteError, RemoteScheme, RemoteSpec};

#[test]
fn accepted_and_rejected_synthetic_remotes() {
    let accepted = [
        "https://example.com/a",
        "https://EXAMPLE.com./a",
        "https://example.com:443/a",
        "https://example.com:1/a",
        "https://example.com:65535/a",
        "ssh://example.com/a",
        "ssh://git@example.com/a",
        "ssh://git@example.com:22/a",
        "git@example.com:a",
        "u.ser_name-1@example.com:group/repo.git",
        "https://8.8.8.8/a",
        "https://1.1.1.1/a",
        "https://9.255.255.255/a",
        "https://11.0.0.0/a",
        "https://126.255.255.255/a",
        "https://128.0.0.0/a",
        "https://172.15.255.255/a",
        "https://172.32.0.0/a",
        "https://192.167.255.255/a",
        "https://192.169.0.0/a",
        "https://169.253.255.255/a",
        "https://169.255.0.0/a",
        "https://223.255.255.255/a",
        "https://example.com/a.b_c",
        "https://example.com/a/Repo.git",
        "ssh://USER@example.com:1234/A-B",
    ];
    for input in accepted {
        assert!(RemoteSpec::parse(input).is_ok());
    }
    let rejected = [
        "",
        "-https://example.com/a",
        "https://example.com/",
        "https://example.com/a/",
        "https://example.com/a//b",
        "https://example.com/./b",
        "https://example.com/a/../b",
        "https://example.com/-a",
        "https://example.com/a/-b",
        "https://example.com/a@b",
        "https://example.com/a:b",
        "https://example.com/a%20b",
        "https://example.com/a?b",
        "https://example.com/a#b",
        "https://example.com/a\\b",
        "https://example.com/a b",
        "https://example.com/é",
        "https://user:pass@example.com/a",
        "https://user@example.com/a",
        "ssh://user:pass@example.com/a",
        "ssh://-user@example.com/a",
        "ssh://@example.com/a",
        "ssh://u@@example.com/a",
        "file://example.com/a",
        "git://example.com/a",
        "ext::abc",
        "/tmp/repo",
        "example.com:a",
        "https://[::1]/a",
        "https://*.example.com/a",
        "https://xn--example.com/a",
        "https://example..com/a",
        "https://-example.com/a",
        "https://example-.com/a",
        "https://example.com:0/a",
        "https://example.com:65536/a",
        "https://example.com:/a",
        "https://example.com:abc/a",
        "https://localhost/a",
        "https://foo.localhost/a",
        "https://0.1.2.3/a",
        "https://10.0.0.1/a",
        "https://127.255.255.255/a",
        "https://169.254.0.1/a",
        "https://172.16.0.0/a",
        "https://172.31.255.255/a",
        "https://192.168.0.1/a",
        "https://224.0.0.1/a",
        "https://255.255.255.255/a",
        "https://127.1/a",
        "https://010.0.0.1/a",
        "https://0x7f.0.0.1/a",
        "https://example.com/...",
        "https://example.com/a\n",
    ];
    for input in rejected {
        assert!(RemoteSpec::parse(input).is_err());
    }
    assert_eq!(
        RemoteSpec::parse(&format!("https://example.com/{}", "a".repeat(2049))).unwrap_err(),
        RemoteError::TooLong
    );
    assert!(RemoteSpec::parse(&format!("https://{}.com/a", "a".repeat(64))).is_err());
}

#[test]
fn private_address_boundaries_and_malformed_numeric_hosts() {
    for host in [
        "0.0.0.0",
        "0.255.255.255",
        "10.0.0.0",
        "10.255.255.255",
        "127.0.0.0",
        "127.255.255.255",
        "169.254.0.0",
        "169.254.255.255",
        "172.16.0.0",
        "172.31.255.255",
        "192.168.0.0",
        "192.168.255.255",
        "224.0.0.0",
        "255.255.255.255",
        "1.2.3",
        "1.2.3.256",
        "01.2.3.4",
        "0x7f000001",
        "2130706433",
        "1.2.3.4.5",
    ] {
        assert!(RemoteSpec::parse(&format!("https://{host}/repo")).is_err());
    }
    for host in [
        "1.0.0.0",
        "9.255.255.255",
        "11.0.0.0",
        "126.255.255.255",
        "128.0.0.0",
        "169.253.255.255",
        "169.255.0.0",
        "172.15.255.255",
        "172.32.0.0",
        "192.167.255.255",
        "192.169.0.0",
        "223.255.255.255",
    ] {
        assert!(RemoteSpec::parse(&format!("https://{host}/repo")).is_ok());
    }
}

#[test]
fn length_and_normalization_boundaries() {
    let prefix = "https://example.com/";
    assert!(RemoteSpec::parse(&format!("{prefix}{}", "a".repeat(2048 - prefix.len()))).is_ok());
    assert_eq!(
        RemoteSpec::parse(&format!("{prefix}{}", "a".repeat(2049 - prefix.len()))).unwrap_err(),
        RemoteError::TooLong
    );
    assert!(RemoteSpec::parse(&format!("ssh://{}@example.com/a", "u".repeat(32))).is_ok());
    assert!(RemoteSpec::parse(&format!("ssh://{}@example.com/a", "u".repeat(33))).is_err());
    assert!(RemoteSpec::parse(&format!("https://{}.com/a", "a".repeat(63))).is_ok());
    assert!(RemoteSpec::parse(&format!("https://{}.com/a", "a".repeat(64))).is_err());

    let spec = RemoteSpec::parse("https://EXAMPLE.com.:00443/Long__Name---Here.git").unwrap();
    assert_eq!(
        spec.remote_url().as_str(),
        "https://example.com/Long__Name---Here.git"
    );
    assert_eq!(spec.slug().as_str(), "long-name-here-git");
    assert_eq!(spec.destination_parts().port, 443);
    let long_slug = RemoteSpec::parse(&format!("https://example.com/{}", "A".repeat(60))).unwrap();
    assert_eq!(long_slug.slug().as_str(), "a".repeat(48));
}

#[test]
fn lexical_checks_do_not_claim_dns_or_path_secret_detection() {
    // The host is only syntactically checked; DNS resolution is outside this parser.
    let spec = RemoteSpec::parse("https://resolves-to-private.example/token-like-value").unwrap();
    assert_eq!(spec.destination_parts().host, "resolves-to-private.example");
    assert_eq!(spec.destination_parts().path, "token-like-value");
    assert!(!format!("{spec:?}").contains("token-like-value"));

    for input in [
        "https://example.com/canary%20value",
        "https://canary:password@example.com/a",
        "ssh://canary:password@example.com/a",
        "https://canary.example/a?secret",
    ] {
        let error = RemoteSpec::parse(input).unwrap_err();
        assert!(!format!("{error:?}").contains("canary"));
        assert!(!format!("{error:?}").contains("password"));
        assert!(!format!("{error:?}").contains("secret"));
    }
}

#[test]
fn canonical_identity_normalization_and_masking() {
    let variants = [
        "https://EXAMPLE.com./Group/Repo.git",
        "ssh://git@example.com:123/Group/Repo.git",
        "git@example.com:Group/Repo.git",
    ];
    for input in variants {
        let spec = RemoteSpec::parse(input).unwrap();
        assert_eq!(
            spec.canonical_remote().as_str(),
            "example.com/Group/Repo.git"
        );
        assert_eq!(spec.slug().as_str(), "repo-git");
        assert!(!format!("{spec:?}").contains("Group"));
        assert!(!format!("{:?}", spec.destination_parts()).contains("example.com"));
    }
    let spec = RemoteSpec::parse("ssh://git@EXAMPLE.com.:0022/Group/Repo.git").unwrap();
    assert_eq!(
        spec.remote_url().as_str(),
        "ssh://git@example.com/Group/Repo.git"
    );
    let parts = spec.destination_parts();
    assert_eq!(
        (parts.scheme, parts.host, parts.port, parts.path),
        (RemoteScheme::Ssh, "example.com", 22, "Group/Repo.git")
    );
    assert_ne!(
        RemoteSpec::parse("https://example.com/a")
            .unwrap()
            .canonical_remote()
            .as_str(),
        RemoteSpec::parse("https://example.com/A")
            .unwrap()
            .canonical_remote()
            .as_str()
    );
    let secret = "secret-canary";
    let error =
        RemoteSpec::parse(&format!("https://{secret}:password@example.com/repo")).unwrap_err();
    assert!(!format!("{error:?}").contains(secret));
}

#[test]
fn canonical_collisions_preserve_full_operation_identity() {
    let https = RemoteSpec::parse("https://Example.com:8443/Team/Repo.git").unwrap();
    let ssh = RemoteSpec::parse("ssh://alice@example.com:2222/Team/Repo.git").unwrap();
    let scp = RemoteSpec::parse("bob@example.com:Team/Repo.git").unwrap();
    assert_eq!(
        https.canonical_remote().as_str(),
        ssh.canonical_remote().as_str()
    );
    assert_eq!(
        ssh.canonical_remote().as_str(),
        scp.canonical_remote().as_str()
    );
    assert_ne!(https.remote_url().as_str(), ssh.remote_url().as_str());
    assert_ne!(ssh.remote_url().as_str(), scp.remote_url().as_str());
    assert_eq!(https.destination_parts().port, 8443);
    assert_eq!(ssh.destination_parts().port, 2222);
    assert_eq!(scp.destination_parts().port, 22);
    assert_ne!(
        https.canonical_remote().as_str(),
        RemoteSpec::parse("https://example.com/Team/Repo")
            .unwrap()
            .canonical_remote()
            .as_str()
    );
    assert_ne!(
        https.canonical_remote().as_str(),
        RemoteSpec::parse("https://example.com/team/Repo.git")
            .unwrap()
            .canonical_remote()
            .as_str()
    );
}

#[test]
fn canonical_identity_preserves_graphic_path_punctuation() {
    let https = RemoteSpec::parse("https://example.com/Team/Repo+name.git").unwrap();
    let ssh = RemoteSpec::parse("git@example.com:Team/Repo+name.git").unwrap();
    assert_eq!(
        https.canonical_remote().as_str(),
        "example.com/Team/Repo+name.git"
    );
    assert_eq!(
        https.canonical_remote().as_str(),
        ssh.canonical_remote().as_str()
    );
    assert_ne!(
        https.canonical_remote().as_str(),
        RemoteSpec::parse("https://example.com/Team/Reponame.git")
            .unwrap()
            .canonical_remote()
            .as_str()
    );
}

#[test]
fn slug_truncation_and_ssh_user_identity_are_stable() {
    for (name, expected) in [
        (
            format!("{}--Z", "a".repeat(46)),
            format!("{}-z", "a".repeat(46)),
        ),
        (
            format!("{}--Z", "a".repeat(47)),
            format!("{}z", "a".repeat(47)),
        ),
        (format!("{}--Z", "a".repeat(48)), "a".repeat(48)),
    ] {
        let spec = RemoteSpec::parse(&format!("ssh://Alice@EXAMPLE.com:022/{name}")).unwrap();
        assert_eq!(spec.slug().as_str(), expected);
        assert_eq!(
            spec.remote_url().as_str(),
            format!("ssh://Alice@example.com/{name}")
        );
    }
}

#[test]
fn rejects_options_and_encoded_or_ambiguous_authorities_without_echo() {
    for input in [
        "git@example.com:-upload-pack=canary",
        "git@example.com:team/-canary",
        "ssh://canary@example.com:22:23/repo",
        "https://example.com%2ecanary/repo",
        "https://example.com./repo%2fcanary",
        "https://example.com../canary",
        "https://0x7f.0.0.1/canary",
        "https://0177.0.0.1/canary",
        "https://example.com/canary\t",
        "ssh://canary:secret@example.com/repo",
    ] {
        let error = RemoteSpec::parse(input).unwrap_err();
        let rendered = format!("{error:?}");
        assert!(!rendered.contains("canary"));
        assert!(!rendered.contains("secret"));
    }
}
