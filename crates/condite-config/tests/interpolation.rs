use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    secret: String,
}

#[test]
fn substitutions_are_values_and_never_toml_structure() {
    let config: Config = condite_config::parse_with("secret = '${SECRET}'", |key| {
        assert_eq!(key, "SECRET");
        Some("quote'\n東京\n[attacker]\nsecret='${OTHER}'".into())
    })
    .unwrap();
    assert_eq!(config.secret, "quote'\n東京\n[attacker]\nsecret='${OTHER}'");
}

#[test]
fn missing_variables_and_unknown_fields_are_secret_safe() {
    let missing = condite_config::parse_with::<Config>("secret = '${MISSING}'", |_| None);
    assert!(missing.err().unwrap().to_string().contains("MISSING"));
    let invalid =
        condite_config::parse_with::<Config>("secret = 'TOP-SECRET'\nother = 'SECRET-2'", |_| None);
    let error = invalid.err().unwrap().to_string();
    assert!(!error.contains("TOP-SECRET"));
    assert!(!error.contains("SECRET-2"));
}

#[test]
fn escaping_and_nonrecursive_expansion() {
    let config: Config = condite_config::parse_with("secret = '$${LITERAL} ${VALUE}'", |_| {
        Some("${NO_RECURSION}".into())
    })
    .unwrap();
    assert_eq!(config.secret, "${LITERAL} ${NO_RECURSION}");
}

#[test]
fn core_schema_rejects_duplicate_roots_invalid_urls_and_secrets_in_errors() {
    let source = "sql_conn_uri = 'turso::memory:'\ns3_endpoint = 'https://s3.example'\ns3_region = 'test'\ns3_bucket = 'test'\ns3_access_key = 'secret-access'\ns3_secret_key = 'secret-storage'\npublic_api_url = 'https://files.example'\n[[root_keys]]\nname = 'admin'\nkey = 'secret-root-01234567890123456789012345'\n[[root_keys]]\nname = 'admin'\nkey = 'secret-root-01234567890123456789012346'";
    let error = condite_config::CoreConfig::parse(source)
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("duplicate"));
    assert!(!error.contains("secret-"));
    assert!(
        condite_config::CoreConfig::parse(&source.replace("https://s3.example", "bad://url"))
            .is_err()
    );
}
