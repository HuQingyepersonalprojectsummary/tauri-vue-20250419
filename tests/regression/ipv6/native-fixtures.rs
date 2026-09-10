#[derive(Default)]
struct MockState { scenario: String, calls: Vec<serde_json::Value>, reads: usize }
thread_local! { static MOCK: std::cell::RefCell<MockState> = std::cell::RefCell::new(MockState::default()); }
fn snapshot(after: bool) -> AdapterSnapshot {
 let ip = if after {"192.0.2.20"} else {"192.0.2.10"};
 serde_json::from_value(serde_json::json!({
  "adapterName":"fixture","interfaceIndex":7,"interfaceGuid":"fixture-guid","status":"Up",
  "dhcpEnabled":false,"dnsDhcpEnabled":false,"ipv6Enabled":true,
  "addresses":[{"ipAddress":ip,"prefixLength":24,"mask":"255.255.255.0"}],
  "gateways":["192.0.2.1"],"dnsServers":[if after {"198.51.100.53"} else {"192.0.2.53"}],
  "ip":ip,"mask":"255.255.255.0","gateway":"192.0.2.1",
  "dns1":if after {"198.51.100.53"} else {"192.0.2.53"},"dns2":""
 })).unwrap()
}
pub fn audit_main() {
 let mut rows = Vec::new();
 for scenario in ["wrong_doh_template_and_fallback", "wrong_fallback_only", "off_still_on", "new_doh_rollback", "residual_global_doh", "dns_timeout", "dhcp_ipv6_only", "empty_static_preflight", "unsupported_preflight"] {
  MOCK.with(|m| *m.borrow_mut() = MockState {scenario:scenario.into(),..Default::default()});
  let mut cfg: Ipv4Config = serde_json::from_value(serde_json::json!({
    "adapter":"fixture","ip":"192.0.2.20","mask":"255.255.255.0","gateway":"192.0.2.1",
    "dns1":"198.51.100.53","dns2":"","ipv6Enabled":true,
    "doh1":{"mode":if scenario=="off_still_on" {"off"} else {"manual"},"template":"https://expected.example/dns-query","allowFallback":false}
  })).unwrap();
  if scenario == "dhcp_ipv6_only" { cfg.ip_mode=Some("dhcp".into()); cfg.dns_mode=Some("dhcp".into()); }
  let result = apply_adapter_ipv4_config_transactional(&cfg);
  MOCK.with(|m| rows.push(serde_json::json!({"case":scenario,"result":result,"calls":m.borrow().calls,"reads":m.borrow().reads})));
 }
 let mut before = snapshot(false);
 before.addresses.push(Ipv4AddressConfig{ip_address:"192.0.2.11".into(),prefix_length:24,mask:"255.255.255.0".into()});
 before.gateways.push("192.0.2.254".into());
 let current = snapshot(false);
 rows.push(serde_json::json!({"case":"missing_secondary_address_and_gateway","result":verify_snapshot_restored(&before,&current)}));
 let before = snapshot(false);
 let mut current = before.clone(); current.dns_servers.push("203.0.113.53".into());
 rows.push(serde_json::json!({"case":"unexpected_extra_dns","result":verify_snapshot_restored(&before,&current)}));
 let mut current=before.clone(); current.doh_settings.insert("198.51.100.53".into(),DohServerSetting{template:"https://residual.example/dns-query".into(),allow_fallback:true,auto_upgrade:true});
 rows.push(serde_json::json!({"case":"global_doh_table_ignored_by_restore_verification","result":verify_snapshot_restored(&before,&current)}));
 MOCK.with(|m| *m.borrow_mut()=MockState::default());
 let mut before=snapshot(false);before.addresses.clear(); before.dns_servers.clear();
 let result=rollback_snapshot("fixture",&before);
 MOCK.with(|m| rows.push(serde_json::json!({"case":"empty_static_restore_commands","result":result,"calls":m.borrow().calls})));
 ipv6_cases(&mut rows);
 println!("{}",serde_json::to_string_pretty(&rows).unwrap());
}
