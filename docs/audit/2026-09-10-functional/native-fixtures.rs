
struct Mock { snap: AdapterSnapshot, calls: Vec<serde_json::Value>, fail_v6_dns_once: bool }
fn initial() -> AdapterSnapshot {
 serde_json::from_value(serde_json::json!({
  "adapterName":"fixture","interfaceIndex":7,"interfaceGuid":"fixture-guid","status":"Up",
  "dhcpEnabled":true,"dnsDhcpEnabled":false,"ipv6Enabled":false,"dohSupported":true,
  "addresses":[],"gateways":[],"dnsServers":["192.0.2.53"],
  "ip":"","mask":"","gateway":"","dns1":"192.0.2.53","dns2":"",
  "doh1":{"mode":"auto","template":"https://adapter.example/dns-query","allowFallback":false},
  "dohSettings":{"192.0.2.53":{"template":"https://global.example/dns-query","autoUpgrade":true,"allowFallback":true}},
  "ipv6DhcpEnabled":true,"ipv6DnsDhcpEnabled":true
 })).unwrap()
}
thread_local! { static MOCK: std::cell::RefCell<Mock> = std::cell::RefCell::new(Mock{snap:initial(),calls:vec![],fail_v6_dns_once:false}); }
pub fn audit_main() {
 let mut rows=vec![];
 for name in ["disable_with_hidden_static","disable_with_hidden_static_dns","keep_dns_turns_doh_off","static_dns_blank_keeps_previous","rollback_uses_global_not_adapter_doh","failed_ipv6_dns_does_not_restore_doh"] {
  MOCK.with(|m|*m.borrow_mut()=Mock{snap:initial(),calls:vec![],fail_v6_dns_once:false});
  let mut cfg:Ipv4Config=serde_json::from_value(serde_json::json!({"adapter":"fixture","ip":"","mask":"","gateway":"","dns1":"","dns2":"","ipMode":"keep","dnsMode":"keep","ipv6Enabled":false})).unwrap();
  if name=="disable_with_hidden_static" {cfg.ipv6_mode=Some("static".into());}
  if name=="disable_with_hidden_static_dns" {cfg.ipv6_dns_mode=Some("static".into());}
  if name=="static_dns_blank_keeps_previous" {
   cfg.dns_mode=Some("static".into());
   MOCK.with(|m|m.borrow_mut().snap.dns_dhcp_enabled=true);
  }
  if name=="failed_ipv6_dns_does_not_restore_doh" {
   cfg.ipv6_enabled=Some(true);cfg.ipv6_dns_mode=Some("dhcp".into());
   cfg.dns_mode=Some("static".into());cfg.dns1="192.0.2.53".into();cfg.doh1=initial().doh1;
   MOCK.with(|m| {
    let mut m=m.borrow_mut();m.fail_v6_dns_once=true;
    m.snap.ipv6_enabled=true;m.snap.ipv6_dns_dhcp_enabled=false;
    m.snap.ipv6_dns_servers=vec!["2001:db8::53".into()];
    m.snap.doh_settings.insert("2001:db8::53".into(),DohServerSetting{template:"https://v6.example/dns-query".into(),auto_upgrade:true,allow_fallback:false});
   });
  }
  if name.starts_with("rollback_") {
   let mut before=initial();
   before.ipv6_dns_servers=vec!["2001:db8::53".into()];
   let result=rollback_snapshot_internal("fixture",&before,&["192.0.2.53".into()],&[]);
   MOCK.with(|m|rows.push(serde_json::json!({"case":name,"before":before,"result":result,"calls":m.borrow().calls})));
  } else {
   let result=apply_adapter_ipv4_config_transactional(&cfg);
   MOCK.with(|m|rows.push(serde_json::json!({"case":name,"config":cfg,"result":result,"calls":m.borrow().calls})));
  }
 }
 println!("{}",serde_json::to_string_pretty(&rows).unwrap());
}
