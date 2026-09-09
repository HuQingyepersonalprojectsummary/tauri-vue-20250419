
#[derive(Default)]
struct MockState {
    scenario: String,
    commands: Vec<Vec<String>>,
    reads: usize,
}
thread_local! { static MOCK: std::cell::RefCell<MockState> = std::cell::RefCell::new(MockState::default()); }

fn fixture_snapshot(after: bool) -> AdapterSnapshot {
    let ip = if after { "192.0.2.20" } else { "192.0.2.10" };
    AdapterSnapshot {
        adapter_name: "fixture".into(), interface_index: 7, interface_guid: "fixture-guid".into(),
        status: "Up".into(), dhcp_enabled: false, dns_dhcp_enabled: false,
        addresses: vec![Ipv4AddressConfig { ip_address: ip.into(), prefix_length: 24, mask: "255.255.255.0".into() }],
        gateways: vec!["192.0.2.1".into()], dns_servers: vec!["192.0.2.53".into()],
        ip: ip.into(), mask: "255.255.255.0".into(), gateway: "192.0.2.1".into(),
        dns1: "192.0.2.53".into(), dns2: String::new(),
    }
}
fn reset_mock(scenario: &str) {
    MOCK.with(|m| *m.borrow_mut() = MockState { scenario: scenario.into(), ..Default::default() });
}
fn capture(id: &str, scenario: &str, result: serde_json::Value) -> serde_json::Value {
    MOCK.with(|m| {
        let state = m.borrow();
        serde_json::json!({"id":id,"case":scenario,"result":result,"commands":state.commands,"snapshotReads":state.reads})
    })
}
pub fn audit_main() {
    let mut results = Vec::new();
    for (id, scenario) in [("R-01", "dns_timeout"), ("R-02", "readback_failure"),
        ("R-02", "wrong_dns_and_gateway"), ("R-03", "rollback_dns_failure"), ("R-02", "dns2_without_dns1")] {
        reset_mock(scenario);
        let cfg = Ipv4Config { adapter:"fixture".into(),ip:"192.0.2.20".into(),mask:"255.255.255.0".into(),
            gateway:"192.0.2.254".into(), dns1:if scenario == "dns2_without_dns1" { String::new() } else { "198.51.100.53".into() },
            dns2:if scenario == "dns2_without_dns1" { "203.0.113.53".into() } else { String::new() } };
        let result = serde_json::to_value(apply_adapter_ipv4_config_transactional(&cfg)).unwrap();
        results.push(capture(id, scenario, result));
    }
    reset_mock("multi_address_restore");
    let mut snapshot = fixture_snapshot(false);
    snapshot.addresses.push(Ipv4AddressConfig { ip_address:"198.51.100.10".into(),prefix_length:24,mask:"255.255.255.0".into() });
    snapshot.gateways.push("198.51.100.1".into());
    snapshot.dns_servers.extend(["198.51.100.53".into(),"203.0.113.53".into()]);
    let result = serde_json::to_value(rollback_snapshot("fixture", &snapshot)).unwrap();
    results.push(capture("R-03", "multi_address_restore_only_uses_first_address_gateway_and_two_dns", result));

    reset_mock("direct_rollback_dns_failure");
    let result = serde_json::to_value(rollback_snapshot("fixture", &snapshot)).unwrap();
    results.push(capture("R-03", "rollback_returns_Ok_even_when_both_DNS_restore_calls_error", result));

    reset_mock("dhcp_with_custom_dns");
    snapshot.dhcp_enabled = true;
    let result = serde_json::to_value(rollback_snapshot("fixture", &snapshot)).unwrap();
    results.push(capture("R-03", "ip_dhcp_forces_dns_dhcp_regardless_of_original_dns_mode", result));

    // Only this isolated process's environment is changed, then immediately restored.
    let saved = std::env::var_os("SystemRoot");
    std::env::set_var("SystemRoot", "Z:\\__nonexistent_audit_fixture__");
    let fallback = get_system_binary("netsh.exe");
    if let Some(value) = saved { std::env::set_var("SystemRoot", value); } else { std::env::remove_var("SystemRoot"); }
    results.push(serde_json::json!({"id":"R-06","case":"missing_trusted_path_falls_back_to_relative_name","path":fallback,"absolute":fallback.is_absolute(),"executed":false}));
    println!("{}", serde_json::to_string(&results).unwrap());
}
