fn ps_snapshot_json() -> String {
 serde_json::json!({
  "adapterName":"fixture","interfaceIndex":7,"interfaceGuid":"fixture-guid","status":"Up",
  "dhcpEnabled":false,"dnsDhcpEnabled":false,"ipv6Enabled":true,"dohSupported":true,"dohSettings":{},
  "addresses":[{"IPAddress":"192.0.2.10","PrefixLength":24}],"gateways":[],"dnsServers":[],
  "ipv6Addresses":[{"IPAddress":"2001:db8::10","PrefixLength":64,"PrefixOrigin":"Manual","SuffixOrigin":"Manual"}],
  "ipv6Gateways":["fe80::1"],"ipv6DnsServers":["2001:db8::53"],
  "ipv6DhcpEnabled":false,"ipv6DnsDhcpEnabled":false
 }).to_string()
}
fn v6_snapshot(scenario: &str, reads: usize) -> AdapterSnapshot {
 let mut s=snapshot(false);
 s.ipv6_dhcp_enabled=false;s.ipv6_dns_dhcp_enabled=false;
 s.ipv6_addresses=vec![crate::domain::Ipv6AddressConfig{ip_address:"2001:db8::10".into(),prefix_length:64,prefix_origin:"Manual".into(),suffix_origin:"Manual".into()}];
 s.ipv6_gateways=vec!["fe80::1".into()];s.ipv6_dns_servers=vec!["2001:db8::53".into()];
 if scenario=="v6_dhcp_rollback_residual" {s.ipv6_dhcp_enabled=true;}
 if reads>1 {
  if scenario=="v6_extra_dns" {s.ipv6_dns_servers=vec!["2001:db8::54".into(),"2001:db8::55".into(),"2001:db8::56".into()];}
  if ["v6_delete_timeout","v6_dhcp_rollback_residual","v6_static_rollback_residual"].contains(&scenario) {s.ipv6_addresses.push(crate::domain::Ipv6AddressConfig{ip_address:"2001:db8::20".into(),prefix_length:64,prefix_origin:"Manual".into(),suffix_origin:"Manual".into()});}
  if scenario=="v6_missing_secondary_dns" {s.ipv6_dns_servers=vec!["2001:db8::54".into()];s.ipv6_dns_dhcp_enabled=true;}
  if scenario=="v6_wrong_gateway_and_mode" {s.ipv6_addresses[0].ip_address="2001:db8::20".into();s.ipv6_dhcp_enabled=true;}
  if scenario=="v6_equivalent_address" {s.ipv6_addresses[0].ip_address="2001:db8::20".into();}
 }
 s
}
fn ipv6_cases(rows: &mut Vec<serde_json::Value>) {
 MOCK.with(|m| *m.borrow_mut()=MockState{scenario:"parser".into(),..Default::default()});
 let parsed=parse_snapshot_fixture("fixture");
 rows.push(serde_json::json!({"case":"actual_snapshot_parser_casing","input":serde_json::from_str::<serde_json::Value>(&ps_snapshot_json()).unwrap(),"result":parsed}));
 for scenario in ["v6_missing_secondary_dns","v6_wrong_gateway_and_mode","v6_equivalent_address","v6_extra_dns","v6_delete_timeout","v6_dhcp_rollback_residual","v6_static_rollback_residual"] {
  MOCK.with(|m| *m.borrow_mut()=MockState{scenario:scenario.into(),..Default::default()});
  let mut cfg:Ipv4Config=serde_json::from_value(serde_json::json!({"adapter":"fixture","ipMode":"keep","dnsMode":"keep","ipv6Mode":"keep","ipv6DnsMode":"keep"})).unwrap();
  if scenario=="v6_missing_secondary_dns" || scenario=="v6_extra_dns" {cfg.ipv6_dns_mode=Some("static".into());cfg.ipv6_dns1="2001:db8::54".into();cfg.ipv6_dns2="2001:db8::55".into();}
  else {cfg.ipv6_mode=Some("static".into());cfg.ipv6_ip=if scenario=="v6_equivalent_address" {"2001:0db8:0:0:0:0:0:20"} else {"2001:db8::20"}.into();cfg.ipv6_prefix=Some(64);if scenario=="v6_wrong_gateway_and_mode" {cfg.ipv6_gateway="fe80::2".into();}}
  if scenario=="v6_dhcp_rollback_residual" || scenario=="v6_static_rollback_residual" {cfg.ipv6_dns_mode=Some("static".into());cfg.ipv6_dns1="2001:db8::54".into();}
  let result=apply_adapter_ipv4_config_transactional(&cfg);
  MOCK.with(|m| rows.push(serde_json::json!({"case":scenario,"request":cfg,"result":result,"calls":m.borrow().calls})));
 }
 let before=v6_snapshot("base",1);
 let mut extra=before.clone();extra.ipv6_addresses.push(crate::domain::Ipv6AddressConfig{ip_address:"2001:db8::99".into(),prefix_length:64,prefix_origin:"Manual".into(),suffix_origin:"Manual".into()});extra.ipv6_gateways=vec!["fe80::bad".into()];
 rows.push(serde_json::json!({"case":"ipv6_restore_extra_address_wrong_gateway","result":verify_snapshot_restored(&before,&extra)}));
 MOCK.with(|m| *m.borrow_mut()=MockState{scenario:"v6_rollback_command_failure".into(),..Default::default()});
 let result=rollback_snapshot("fixture",&before);
 MOCK.with(|m| rows.push(serde_json::json!({"case":"ipv6_rollback_nonzero_exit_ignored","result":result,"calls":m.borrow().calls})));
 let mut dhcp_before=before.clone();dhcp_before.ipv6_dhcp_enabled=true;
 MOCK.with(|m| *m.borrow_mut()=MockState::default());
 let result=rollback_snapshot("fixture",&dhcp_before);
 MOCK.with(|m| rows.push(serde_json::json!({"case":"ipv6_dhcp_restore_commands","result":result,"calls":m.borrow().calls})));
}
